//! 原生规则文档生命周期 wire：TS 类型镜像 + invoke wrapper。
//!
//! 只镜像 RuleSystem 的安全 DTO（snake_case，`request` wrapper 保持现有模式）；
//! credential 明文只作为 replace 的一次性请求值短暂存在（同 Rust `CredentialMutationRequest.value`）；
//! 不进入 history/undo/snapshot 持久化，saveRequest 后立即从 state 清空。
//! 纯 TS，无 runes、无 .svelte.ts。

import { invoke, isTauri } from '@tauri-apps/api/core';
import type {
  InstallDiagnostic,
  SourceProfile,
  SourceSpan,
  StandardIntent,
} from '@/shared/tauri/sources';

export type {
  InstallDiagnostic,
  SourceProfile,
  SourceSpan,
  StandardIntent,
} from '@/shared/tauri/sources';

// ---------------------------------------------------------------------------
// 领域镜像（lj-rule-model / lj-capability serde current shape）
// ---------------------------------------------------------------------------

/** RuleDefinition wire 合同 schema 版本（RULE_CONTRACT_SCHEMA_VERSION = 1）。 */
export const RULE_CONTRACT_SCHEMA_VERSION = 1 as const;

/** 系统级沙箱能力（fs/env/process）。 */
export interface SystemCapabilities {
  fs: boolean;
  env: boolean;
  process: boolean;
}

/** 策略能力配置（PolicyCapabilities serde 镜像）。 */
export interface PolicyCapabilities {
  network: boolean;
  system: SystemCapabilities;
}

/** 能力清单（CapabilityManifest）。 */
export interface CapabilityManifest {
  required: PolicyCapabilities;
}

/** 标准意图导出（IntentExport：Flow 入口 + Mapper 输出节点）。 */
export interface IntentExport {
  flow_entry: string;
  mapper_output: string;
}

/**
 * Flow 节点 kind：wire 上是开放字符串，不封闭。
 *
 * 已安装能力是 `http` / `js` / `extract` / `mapper` / `merge` / `condition` / `loop`；
 * 未安装能力保留作者写入的原文（Rust `FlowNodeConfig::Unavailable`），可展示、保存与
 * round-trip，因此这里不能收成闭集。字段、端口与默认值一律由 descriptor 声明提供。
 */
export type FlowNodeKind = string;

/** Flow 节点配置（FlowNodeConfig：`{ kind, value }`；value 为最小镜像，只读消费）。 */
export interface FlowNodeConfig {
  kind: FlowNodeKind;
  value: Record<string, unknown>;
}

/** Flow 节点（FlowNode）。 */
export interface FlowNode {
  id: string;
  config: FlowNodeConfig;
  span?: SourceSpan | null;
}

/** 节点语义 port 引用（FlowPortRef）。 */
export interface FlowPortRef {
  node_id: string;
  handle: string;
}

/** Flow 语义边（FlowEdge）；identity 恰为 `from node/handle + to node/handle`。 */
export interface FlowEdge {
  from: FlowPortRef;
  to: FlowPortRef;
}

/** 类型化 Flow 图（FlowGraph）。 */
export interface FlowGraph {
  nodes: FlowNode[];
  edges: FlowEdge[];
}

/** 规则定义作者合同（RuleDefinition serde current shape，含 contract tag）。 */
export interface SourceIdentity {
  id: string;
}

export interface RuleDefinition {
  contract: 'rule_definition';
  schema_version: typeof RULE_CONTRACT_SCHEMA_VERSION;
  /** Rust current wire 是 `{ id }`；string 仅保留旧前端 skeleton 兼容。 */
  source_identity: SourceIdentity | string;
  base_url: string;
  intent_exports: Partial<Record<StandardIntent, IntentExport>>;
  flow: FlowGraph;
  capability_manifest: CapabilityManifest;
  source_id_rules: string[];
}

// ---------------------------------------------------------------------------
// 请求 DTO（snake_case；command 层统一 `request` wrapper）
// ---------------------------------------------------------------------------

/** 模板意图期望的输入数据类型（ExpectedDataType 闭集）。 */
export type ExpectedDataType = 'html' | 'xml' | 'json';

/** 新建文档的创建模式（CreateMode，tagged）。 */
export type CreateMode =
  | { kind: 'blank' }
  | {
      kind: 'template';
      title: string;
      intent: StandardIntent;
      data_type: ExpectedDataType;
      base_url: string;
    }
  | {
      kind: 'import';
      title: string;
      definition: RuleDefinition;
    };

export interface CreateNativeRuleDocumentRequest {
  mode: CreateMode;
}

/** Credential 一次性变更请求（CredentialMutationRequest）；value 仅 replace 时存在。 */
export interface CredentialMutationRequest {
  node_id: string;
  json_pointer: string;
  logical_name: string;
  action: 'replace' | 'clear';
  /** 一次性明文（replace）；clear 必须省略。值只存在于请求栈，不进入 history/snapshot。 */
  value?: string;
}

/** 语义域保存输入（SemanticSave）。 */
export interface SemanticSave {
  expected_revision: number;
  definition: RuleDefinition;
  credential_mutations: CredentialMutationRequest[];
}

/** 布局域保存输入（LayoutSave）。 */
export interface LayoutSave {
  expected_revision: number;
  layout_json: string;
}

export interface SaveNativeRuleDocumentRequest {
  document_id: string;
  semantic?: SemanticSave | null;
  layout?: LayoutSave | null;
}

/** 语义保存后的生效状态。 */
export type SemanticActivation = 'draft' | 'effective';

/** 域保存结果（DomainOutcome）。 */
export interface DomainOutcome {
  revision: number;
  conflict?: RevisionConflict | null;
  activation?: SemanticActivation | null;
}

/** 乐观并发冲突（RevisionConflict）。 */
export interface RevisionConflict {
  expected: number;
  current: number;
}

export interface SaveNativeRuleDocumentOutcome {
  document_id: string;
  semantic?: DomainOutcome | null;
  layout?: DomainOutcome | null;
}

export interface ValidateNativeRuleDocumentRequest {
  document_id: string;
  revision: number;
}

/** 校验安全预览；不含 Plan/Graph JSON。 */
export interface ValidateNativeRuleDocumentPreview {
  revision: number;
  definition_hash: string;
  valid: boolean;
  plan_hash: string | null;
  diagnostics: InstallDiagnostic[];
  profile: SourceProfile | null;
  capability: PolicyCapabilities;
}

export interface GetNativeRuleDocumentRequest {
  document_id: string;
}

export interface GetNativeRuleProvenanceRequest {
  document_id: string;
}

export interface RenameNativeRuleDocumentRequest {
  document_id: string;
  title: string;
  expected_revision: number;
  trace_id: string;
  occurred_at_ms: number;
}

export interface DeleteNativeRuleDocumentRequest {
  document_id: string;
  confirm_linked: boolean;
  trace_id: string;
  occurred_at_ms: number;
}

/** 从 Effective 历史创建 Draft 的请求。 */
export interface RestoreNativeRuleRevisionRequest {
  document_id: string;
  revision: number;
  expected_revision: number;
}

// ---------------------------------------------------------------------------
// 响应 DTO
// ---------------------------------------------------------------------------

/** 文档生命周期状态。 */
export type NativeRuleDocumentState = 'draft' | 'linked';

/** 文档摘要（镜像 storage DocumentSummary）。 */
export interface NativeRuleDocumentSummary {
  document_id: string;
  format: string;
  title: string;
  source_identity: string;
  state: NativeRuleDocumentState;
  semantic_revision: number;
  layout_revision: number;
  link_revision: number;
  created_at_ms: number;
  updated_at_ms: number;
}

/** Effective Rule Revision 的只读摘要；不含 Definition、Plan 或凭证引用。 */
export interface NativeRuleRevisionSummary {
  revision: number;
  definition_hash: string;
  effective_at_ms: number;
}

/** 历史恢复结果；成功时只创建新的 Draft Revision。 */
export interface RestoreNativeRuleRevisionOutcome {
  document_id: string;
  revision: number;
  conflict?: RevisionConflict | null;
}

/** 溯源摘要（ProvenanceSummaryView；不含 secret_id）。 */
export interface ProvenanceSummaryView {
  format: string;
  adapter_version: string;
  input_hash: string;
  diagnostics: InstallDiagnostic[];
  imported_at_ms: number;
}

export interface NativeRuleDocumentDetail {
  summary: NativeRuleDocumentSummary;
  semantic_revision: number;
  effective_semantic_revision: number | null;
  effective_summary?: NativeRuleRevisionSummary | null;
  layout_revision: number;
  /** 后端当前保存的 Definition；缺失时 session 必须返回 document_semantic_missing。 */
  definition: RuleDefinition | null;
  /** 后端当前保存的作者布局 JSON；布局不进入语义 hash。 */
  layout_json: string | null;
  provenance?: ProvenanceSummaryView | null;
}

/** 只读溯源视图（NativeRuleProvenanceView；原文已脱敏）。 */
export interface NativeRuleProvenanceView {
  format: string;
  adapter_version: string;
  input_hash: string;
  diagnostics: InstallDiagnostic[];
  imported_at_ms: number;
  masked_text: string;
}

// ---------------------------------------------------------------------------
// 错误合同（lj-rule-system::RuleError serde 镜像）
// ---------------------------------------------------------------------------

/** 规则生命周期失败所属阶段（RuleErrorStage）。 */
export type RuleErrorStage =
  | 'import'
  | 'validation'
  | 'compile'
  | 'candidate'
  | 'install'
  | 'capability'
  | 'execution'
  | 'effect'
  | 'persistence'
  | 'replay'
  | 'cancelled'
  | 'internal';

/**
 * 规则生命周期的安全错误（RuleError）。
 *
 * Tauri command 的 reject 值就是这个形状而不是 `Error` 实例；`diagnostics` 是
 * compiler 已本地化的稳定 code + message，展示层直接消费，不重新解释。
 */
export interface RuleErrorWire {
  stage: RuleErrorStage;
  code: string;
  message: string;
  trace_id: string;
  retryable: boolean;
  diagnostics: InstallDiagnostic[];
}

/**
 * 判断被 reject 的值是否满足 RuleError 合同。
 *
 * 只做形状校验：非 IPC 错误（网络异常、JS 异常）必须返回 false，让调用方
 * 合成自己的稳定 code，而不是把任意对象当成后端错误解释。
 */
export function isRuleErrorWire(value: unknown): value is RuleErrorWire {
  if (typeof value !== 'object' || value === null) return false;
  const candidate = value as Partial<RuleErrorWire>;
  return (
    typeof candidate.stage === 'string' &&
    typeof candidate.code === 'string' &&
    typeof candidate.message === 'string' &&
    typeof candidate.retryable === 'boolean' &&
    Array.isArray(candidate.diagnostics)
  );
}

// ---------------------------------------------------------------------------
// Invoke wrapper（11 个，与 Tauri command 名一致；统一 `{ request }` wrapper）
// ---------------------------------------------------------------------------

/** 新建原生规则文档（blank 或 template 模式）。 */
export function createNativeRuleDocument(
  request: CreateNativeRuleDocumentRequest,
): Promise<NativeRuleDocumentSummary> {
  return invoke<NativeRuleDocumentSummary>('create_native_rule_document', { request });
}

/** 分域保存语义/布局；返回分域 outcome（revision 或 conflict）。 */
export function saveNativeRuleDocument(
  request: SaveNativeRuleDocumentRequest,
): Promise<SaveNativeRuleDocumentOutcome> {
  return invoke<SaveNativeRuleDocumentOutcome>('save_native_rule_document', { request });
}

/** 校验文档并返回安全预览（hash/diagnostics/profile/capability）。 */
export function validateNativeRuleDocument(
  request: ValidateNativeRuleDocumentRequest,
): Promise<ValidateNativeRuleDocumentPreview> {
  return invoke<ValidateNativeRuleDocumentPreview>('validate_native_rule_document', { request });
}

/** 列出全部原生规则文档摘要。 */
export function listNativeRuleDocuments(): Promise<NativeRuleDocumentSummary[]> {
  // 浏览器里跑 vite dev 时没有 IPC，读路径退化为空集，页面走空状态而不是报错。
  if (!isTauri()) return Promise.resolve([]);
  return invoke<NativeRuleDocumentSummary[]>('list_native_rule_documents', { request: {} });
}

/** 读取单个文档详情（summary + revision + provenance 摘要）。 */
export function getNativeRuleDocument(
  request: GetNativeRuleDocumentRequest,
): Promise<NativeRuleDocumentDetail | null> {
  if (!isTauri()) return Promise.resolve(null);
  return invoke<NativeRuleDocumentDetail | null>('get_native_rule_document', { request });
}

/** 列出文档的 Effective Rule Revision 历史摘要。 */
export function listNativeRuleRevisionHistory(
  request: GetNativeRuleDocumentRequest,
): Promise<NativeRuleRevisionSummary[]> {
  if (!isTauri()) return Promise.resolve([]);
  return invoke<NativeRuleRevisionSummary[]>('list_native_rule_revision_history', { request });
}

/** 从指定 Effective 历史创建新的 Draft Rule Revision。 */
export function restoreNativeRuleRevision(
  request: RestoreNativeRuleRevisionRequest,
): Promise<RestoreNativeRuleRevisionOutcome> {
  return invoke<RestoreNativeRuleRevisionOutcome>('restore_native_rule_revision', { request });
}

/** 重命名文档（展示 metadata；不推进语义 revision、不改 hash）。 */
export function renameNativeRuleDocument(
  request: RenameNativeRuleDocumentRequest,
): Promise<NativeRuleDocumentSummary> {
  return invoke<NativeRuleDocumentSummary>('rename_native_rule_document', { request });
}

/** 删除文档；state='linked' 且未确认时后端拒绝。 */
export function deleteNativeRuleDocument(request: DeleteNativeRuleDocumentRequest): Promise<void> {
  return invoke<void>('delete_native_rule_document', { request });
}

/** 读取文档溯源（masked 只读原文 + 诊断）。 */
export function getNativeRuleProvenance(
  request: GetNativeRuleProvenanceRequest,
): Promise<NativeRuleProvenanceView | null> {
  return invoke<NativeRuleProvenanceView | null>('get_native_rule_provenance', { request });
}

// ---------------------------------------------------------------------------
// 节点能力 descriptor 镜像（lj-rule-model::descriptor）
//
// 编辑器渲染、port 与默认值的唯一来源。前端不再自带 kind → 字段/端口/默认值表；
// `kind` 是 wire 字符串而不是闭集，未安装能力因此不需要新的前端分支。
// ---------------------------------------------------------------------------

/** Port 在规则图中的结构角色（PortRole）。 */
export type NodePortRole = 'data' | 'control' | 'binding';

/** Port 在节点边界上的语义侧位（PortSide）。 */
export type NodePortSide = 'left' | 'right' | 'top' | 'bottom';

/** Port 展示标签声明（PortLabelDescriptor；key 为 messages 里的稳定 key）。 */
export interface NodePortLabelDescriptor {
  key: string;
  literal: boolean;
  numbered: boolean;
}

/** Port 可引用的闭集 value kind（PortValueKind）。 */
export type PortValueKind =
  | 'intent_input'
  | 'raw'
  | 'http_response'
  | 'json'
  | 'delta'
  | 'loop_binding';

/** 单一 kind 或显式 closed union（PortValueType）。 */
export type PortValueTypeWire =
  | { type: 'kind'; kind: PortValueKind }
  | { type: 'union'; kinds: PortValueKind[] };

/** Port handle 来源（PortHandleSource）。 */
export type NodePortHandleSource =
  | { source: 'fixed'; handle: string }
  | { source: 'items'; field: string }
  | { source: 'item_field'; field: string; handle_field: string };

/** Config 枚举值 → value kind 映射（VariantKind）。 */
export interface VariantKindWire {
  value: string;
  kind: PortValueKind;
  label_key: string;
}

/** Port value type 来源（PortValueSource）。 */
export type NodePortValueSource =
  | { source: 'kind'; kind: PortValueKind }
  | { source: 'union'; kinds: PortValueKind[] }
  | { source: 'field_variant'; field: string; variants: VariantKindWire[] };

/** 节点上的一个声明式 port（PortDescriptor）。 */
export interface NodePortDescriptor {
  handle: NodePortHandleSource;
  value: NodePortValueSource;
  role: NodePortRole;
  side: NodePortSide;
  label: NodePortLabelDescriptor;
}

/** 下拉选项（SelectOptionDescriptor）。 */
export interface SelectOptionDescriptor {
  value: string;
  label: string;
}

/** 字段编辑器种类（FieldEditor）。 */
export type NodeFieldEditor =
  | { editor: 'text' }
  | { editor: 'code' }
  | { editor: 'number'; min: number | null; max: number | null }
  | { editor: 'select'; options: SelectOptionDescriptor[] }
  | {
      editor: 'string_list';
      item_label_key: string;
      add_label_key: string;
      remove_label_key: string;
      min_items: number;
    }
  | {
      editor: 'pair_list';
      key_label_key: string;
      value_label_key: string;
      add_label_key: string;
      remove_label_key: string;
    }
  | { editor: 'specialized'; editor_kind: string };

/** 节点上的一个声明式配置字段（FieldDescriptor）。 */
export interface NodeFieldDescriptor {
  name: string;
  label_key: string;
  editor: NodeFieldEditor;
  required: boolean;
}

/** 一个规则能力的完整声明（NodeDescriptor）。 */
export interface NodeDescriptor {
  kind: string;
  label_key: string;
  description_key: string;
  icon: string;
  inputs: NodePortDescriptor[];
  outputs: NodePortDescriptor[];
  fields: NodeFieldDescriptor[];
  default_config: Record<string, unknown>;
}

/** Descriptor 声明集合（NodeDescriptorSet）。 */
export interface NodeDescriptorSet {
  descriptors: NodeDescriptor[];
}

/** 读取全部节点能力声明；非 Tauri 环境退化为声明表 fixture（同源生成物）。 */
export function listRuleNodeDescriptors(): Promise<NodeDescriptorSet> {
  if (!isTauri()) return Promise.resolve({ descriptors: [] });
  return invoke<NodeDescriptorSet>('list_rule_node_descriptors', {});
}
