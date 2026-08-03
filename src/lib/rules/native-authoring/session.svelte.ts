//! 原生规则 Flow 编辑器 session（.svelte.ts，runes）。
//!
//! 唯一页面 owner：持有 plain TS `NativeRuleAuthoringCore` state、异步 API（wire wrapper）与
//! 瞬时 UI 状态。组件只调 session 方法、读 session 投影，不直接持有 core/reduce。
//!
//! # 设计要点
//!
//! - core state（`NativeRuleAuthoringState`）存于 `$state.raw`：reassign only（大 JSON 不可变）。

import {
  cloneJson,
  createBlankDefinition,
  createInitialState,
  readLayout,
  reduce,
  type AuthoringAction,
  type NativeRuleAuthoringState,
  type Position,
} from './core';
import {
  loopRegionSelectionToken,
  portSelectionToken,
  semanticToFlow,
  type FlowHandleDirection,
  type FlowProjection,
} from './flow-adapter';
import type {
  CreateMode,
  CredentialMutationRequest,
  ExpectedDataType,
  FlowEdge,
  FlowNode,
  FlowNodeKind,
  InstallCandidate,
  InstallDiagnostic,
  NativeRuleDocumentDetail,
  NativeRuleDocumentSummary,
  RevisionConflict,
  RuleDefinition,
  SaveNativeRuleDocumentOutcome,
  StandardIntent,
  ValidateNativeRuleDocumentPreview,
} from './wire';
import {
  createNativeRuleDocument,
  getNativeRuleDocument,
  prepareNativeRuleDocument,
  saveNativeRuleDocument,
  validateNativeRuleDocument,
} from './wire';

// ---------------------------------------------------------------------------
// 详情快照辅助：旧版本 wire 可能没有 definition/layout，保留空图回退以兼容旧文档。
// ---------------------------------------------------------------------------

/** 解析后端保存的 opaque layout；非法布局不影响语义快照打开。 */
function parseLayoutJson(layoutJson: string | null | undefined): unknown {
  if (!layoutJson) return null;
  try {
    return JSON.parse(layoutJson) as unknown;
  } catch {
    return null;
  }
}

/** 从详情快照重建本地 state；definition 缺失时回退空图。 */
function stateFromDetail(
  detail: NativeRuleDocumentDetail,
  fallbackDefinition?: NativeRuleAuthoringState['definition'],
): NativeRuleAuthoringState {
  const definition =
    detail.definition ??
    fallbackDefinition ??
    createBlankDefinition(detail.summary.source_identity);
  const semanticRevision = detail.semantic_revision ?? detail.summary.semantic_revision;
  const layoutRevision = detail.layout_revision ?? detail.summary.layout_revision;
  return createInitialState({
    definition,
    layout: parseLayoutJson(detail.layout_json),
    savedSemanticRevision: semanticRevision === 0 ? null : semanticRevision,
    savedLayoutRevision: layoutRevision === 0 ? null : layoutRevision,
  });
}

// ---------------------------------------------------------------------------
// NativeRuleEditorSession
// ---------------------------------------------------------------------------

/**
 * Flow 编辑器 session：唯一页面 state owner。
 *
 * 使用方式：
 * ```ts
 * const session = new NativeRuleEditorSession();
 * await session.createTemplate({ title: 'My Rule', intent: 'Search', dataType: 'html', baseUrl: 'https://…' });
 * // 或 await session.loadDocument('doc:1');
 * ```
 *
 * 组件通过 `$derived` 绑定 session 的投影属性（flowProjection、dirty、canUndo 等），
 * 并且只通过 `dispatch` (单个 action) 或 `dispatchAll` (action 数组) 修改 state。
 */
export class NativeRuleEditorSession {
  // -----------------------------------------------------------------------
  // 内部依赖
  // -----------------------------------------------------------------------

  /** wire 事件订阅清理函数（execution-delivery 模式）。 */
  #cleanups: (() => void)[] = [];

  /** 当前文档摘要（只读 metadata；null = 未打开文档）。 */
  #summary: NativeRuleDocumentSummary | null = $state(null);

  /**
   * core state（$state.raw：不可变快照大对象，只 reassign 不 mutate）。
   */
  #core: NativeRuleAuthoringState = $state.raw(
    createInitialState({
      definition: createBlankDefinition('placeholder'),
    }),
  );

  // -----------------------------------------------------------------------
  // 暴露的响应式状态（$derived 投影）
  // -----------------------------------------------------------------------

  /** 当前文档 id（未打开时为 null）。 */
  get documentId(): string | null {
    return this.#summary?.document_id ?? null;
  }

  /** 当前文档标题（未打开时为 null）。 */
  get title(): string | null {
    return this.#summary?.title ?? null;
  }

  /** 当前语义定义（只读投影；组件不直接改）。 */
  get definition(): NativeRuleAuthoringState['definition'] {
    return this.#core.definition;
  }

  /** 分域 dirty 标记。 */
  get dirty(): { semantic: boolean; layout: boolean } {
    return this.#core.dirty;
  }

  /** 语义域脏标记。 */
  get dirtySemantic(): boolean {
    return this.#core.dirty.semantic;
  }

  /** 布局域脏标记。 */
  get dirtyLayout(): boolean {
    return this.#core.dirty.layout;
  }

  /** 能否 undo。 */
  get canUndo(): boolean {
    return this.#core.history.length > 0;
  }

  /** 能否 redo。 */
  get canRedo(): boolean {
    return this.#core.redo.length > 0;
  }

  /** 当前诊断列表。 */
  get diagnostics(): InstallDiagnostic[] {
    return this.#core.diagnostics;
  }

  /** 已暂存的安装候选（语义变更后失效为 null）。 */
  get candidate(): { id: string; expires_at_ms: number } | null {
    return this.#core.candidate;
  }

  /** 分域乐观并发冲突。 */
  get conflict(): { semantic: RevisionConflict | null; layout: RevisionConflict | null } {
    return {
      semantic: this.#core.conflict.semantic,
      layout: this.#core.conflict.layout,
    };
  }

  /** 当前意图焦点。 */
  get intentFocus(): StandardIntent | null {
    return this.#core.intentFocus;
  }

  /** 当前选中节点 id。 */
  get selection(): string | null {
    return this.#core.selection;
  }

  /** 画布/视图分域 dirty 时可用 save。 */
  get hasUnsavedChanges(): boolean {
    return this.#core.dirty.semantic || this.#core.dirty.layout;
  }

  /** 在途保存中（等待响应）。 */
  get isSaving(): boolean {
    return this.#core.inFlightSave !== null;
  }

  /** 当前 Flow 投影（$derived 只读）。 */
  get flowProjection(): FlowProjection {
    return semanticToFlow(
      this.#core.definition,
      readLayout(this.#core.layout),
      this.#core.intentFocus,
      this.#core.selection,
    );
  }

  /**
   * 无布局信息的三参投影（用于纯展示场景）。
   */
  get flowProjectionSimple(): FlowProjection {
    return semanticToFlow(this.#core.definition, this.#core.intentFocus, this.#core.selection);
  }

  /** credential 明文不进 history —— 只暴露 slot summary。 */
  get pendingCredentialMutations(): readonly CredentialMutationRequest[] {
    return this.#core.pendingCredentialMutations;
  }

  // -----------------------------------------------------------------------
  // 文档生命周期方法（异步）
  // -----------------------------------------------------------------------

  /**
   * 打开已有文档。
   *
   * wire `get_native_rule_document` 只返回 metadata（无 definition content），
   * 因此 session 用 `createBlankDefinition` 配合文档的 source_identity 构建本地 skeleton；
   * 首次 save 后与后端同步。
   */
  async loadDocument(documentId: string): Promise<void> {
    const detail = await getNativeRuleDocument({ document_id: documentId });
    if (!detail) throw new Error(`文档 ${documentId} 不存在`);

    const state = stateFromDetail(detail);
    this.#summary = detail.summary;
    this.#core = state;
  }

  /**
   * 通过模板创建文档（后端生成最小 Http→Extract→Mapper 骨架）。
   *
   * 只问来源标题、意图、数据类型与基础 URL；后端自动分配 source identity。
   * 返回文档摘要。
   */
  async createTemplate(options: {
    title: string;
    intent: StandardIntent;
    dataType: ExpectedDataType;
    baseUrl: string;
  }): Promise<NativeRuleDocumentSummary> {
    const { title, intent, dataType, baseUrl } = options;
    const mode: CreateMode = {
      kind: 'template',
      title,
      intent,
      data_type: dataType,
      base_url: baseUrl,
    };
    const summary = await createNativeRuleDocument({ mode });
    const detail = await getNativeRuleDocument({ document_id: summary.document_id });
    if (!detail) throw new Error(`文档 ${summary.document_id} 创建后无法读取`);

    // 旧后端未返回 Definition 时保留模板基础 URL，避免展示空表单。
    const definition = createBlankDefinition(summary.source_identity);
    definition.base_url = baseUrl;
    this.#summary = detail.summary;
    this.#core = stateFromDetail(detail, definition);
    return summary;
  }

  /** 导入已通过 current RuleDefinition 预检的原生规则 JSON。 */
  async createImported(options: {
    title: string;
    definition: RuleDefinition;
  }): Promise<NativeRuleDocumentSummary> {
    const { title, definition } = options;
    const mode: CreateMode = {
      kind: 'import',
      title,
      definition: cloneJson(definition),
    };
    const summary = await createNativeRuleDocument({ mode });
    const detail = await getNativeRuleDocument({ document_id: summary.document_id });
    if (!detail) throw new Error(`文档 ${summary.document_id} 创建后无法读取`);
    this.#summary = detail.summary;
    this.#core = stateFromDetail(detail, definition);
    return summary;
  }

  /**
   * 创建空白图文档（后端生成空 Flow + 自动 identity）。
   */
  async createBlank(): Promise<NativeRuleDocumentSummary> {
    const mode: CreateMode = { kind: 'blank' };
    const summary = await createNativeRuleDocument({ mode });
    const detail = await getNativeRuleDocument({ document_id: summary.document_id });
    if (!detail) throw new Error(`文档 ${summary.document_id} 创建后无法读取`);
    this.#summary = detail.summary;
    this.#core = stateFromDetail(detail);
    return summary;
  }

  /**
   * 保存当前文档。
   *
   * 分域保存：semantic（含 credential_mutations）+ layout（layout_json）。
   * 返回 outcome；若分域冲突由上游决定重试策略。
   */
  async save(): Promise<SaveNativeRuleDocumentOutcome> {
    const docId = this.#summary?.document_id;
    if (!docId) throw new Error('未打开文档，无法保存');

    // 1. saveRequest（快照 + epoch 递增 + 清空一次性 credential）
    this.#core = reduce(this.#core, { kind: 'saveRequest' });
    const inFlight = this.#core.inFlightSave;
    if (!inFlight) {
      // 无 dirty 域：幂等返回。
      return {
        document_id: docId,
        semantic: { revision: this.#core.savedSemanticRevision ?? 0, conflict: null },
        layout: { revision: this.#core.savedLayoutRevision ?? 0, conflict: null },
      };
    }

    // 2. 构造请求载荷
    const semanticPayload = inFlight.semantic
      ? {
          expected_revision: inFlight.semantic.revision,
          definition: inFlight.semantic.content as NativeRuleAuthoringState['definition'],
          credential_mutations: [...inFlight.semantic.credentialMutations],
        }
      : undefined;

    const layoutPayload = inFlight.layout
      ? {
          expected_revision: inFlight.layout.revision,
          layout_json: JSON.stringify(inFlight.layout.content),
        }
      : undefined;

    // 3. 调用 wire
    const outcome = await saveNativeRuleDocument({
      document_id: docId,
      semantic: semanticPayload ?? null,
      layout: layoutPayload ?? null,
    });

    // 4. saveResponse（按 epoch merge + increament）
    this.#core = reduce(this.#core, { kind: 'saveResponse', epoch: inFlight.epoch, outcome });

    // 5. 更新 summary revision metadata
    if (this.#summary) {
      this.#summary = {
        ...this.#summary,
        semantic_revision: outcome.semantic?.revision ?? this.#summary.semantic_revision,
        layout_revision: outcome.layout?.revision ?? this.#summary.layout_revision,
      };
    }

    return outcome;
  }

  /**
   * 校验文档（返回安全诊断/哈希/资料/能力摘要）。
   */
  async validate(): Promise<ValidateNativeRuleDocumentPreview> {
    const docId = this.#summary?.document_id;
    if (!docId) throw new Error('未打开文档，无法校验');
    const revision = this.#core.savedSemanticRevision ?? 0;
    const preview = await validateNativeRuleDocument({ document_id: docId, revision });
    this.#core = reduce(this.#core, {
      kind: 'setDiagnostics',
      diagnostics: preview.diagnostics as unknown as InstallDiagnostic[],
    });
    return preview;
  }

  /**
   * 暂存为安装候选（只接受已保存 revision）。
   * 返回 InstallCandidate（复用现有候选链）。
   */
  async prepare(): Promise<InstallCandidate> {
    const docId = this.#summary?.document_id;
    if (!docId) throw new Error('未打开文档，无法 prepare');
    // 强制保存未保存的修改
    if (this.#core.dirty.semantic || this.#core.dirty.layout) {
      await this.save();
    }
    const revision = this.#core.savedSemanticRevision ?? 0;
    const candidate = await prepareNativeRuleDocument({ document_id: docId, revision });
    return candidate;
  }

  // -----------------------------------------------------------------------
  // 状态变更方法（dispatch typed action）
  // -----------------------------------------------------------------------

  /**
   * 派发单个 typed action 到 core reducer。
   */
  dispatch(action: AuthoringAction): void {
    this.#core = reduce(this.#core, action);
  }

  /**
   * 批量派发多个 action（原子序列：前一个输出作为后一个输入）。
   */
  dispatchAll(actions: AuthoringAction[]): void {
    this.#core = actions.reduce(reduce, this.#core);
  }

  /** 撤销（semantic/layout 均可撤销）。 */
  undo(): void {
    this.dispatch({ kind: 'undo' });
  }

  /** 重做。 */
  redo(): void {
    this.dispatch({ kind: 'redo' });
  }

  // -----------------------------------------------------------------------
  // 便捷画布操作（封装 FlowUIChange → core actions）
  // -----------------------------------------------------------------------

  /** 连接两个 port。 */
  connect(edge: FlowEdge): void {
    this.dispatch({ kind: 'edgeConnect', edge, connected: true });
  }

  /** 断开边。 */
  disconnect(edge: FlowEdge): void {
    this.dispatch({ kind: 'edgeConnect', edge, connected: false });
  }

  /** 重连（断开旧边 + 连接新边）。 */
  reconnect(from: FlowEdge, to: FlowEdge): void {
    this.dispatch({ kind: 'edgeReconnect', from, to });
  }

  /** 移动节点（layout command；core 会自动 coalesce 连续同节点操作）。 */
  moveNode(nodeId: string, position: Position): void {
    this.dispatch({ kind: 'moveNode', nodeId, position });
  }

  /** 批量写入节点位置；自动布局作为一个可撤销布局操作提交。 */
  layoutNodes(positions: Record<string, Position>): void {
    this.dispatch({ kind: 'layoutNodes', positions });
  }

  /** 折叠/展开节点。 */
  collapseNode(nodeId: string, collapsed: boolean): void {
    this.dispatch({ kind: 'collapseNode', nodeId, collapsed });
  }

  /** 折叠/展开 Loop region；只写 layout，不改 Definition。 */
  collapseLoopRegion(loopNodeId: string, collapsed: boolean): void {
    this.dispatch({ kind: 'collapseLoopRegion', loopNodeId, collapsed });
  }

  /** 选中节点（不进 history）。 */
  selectNode(nodeId: string | null): void {
    this.dispatch({ kind: 'selection', nodeId });
  }

  /** 选中连线（不进 history）。selection token 仅供 UI 投影使用。 */
  selectEdge(edgeId: string): void {
    this.selectNode(`edge:${edgeId}`);
  }

  /** 选中端口（不进 history）；handle 使用 compiler 的 semantic handle。 */
  selectPort(nodeId: string, direction: FlowHandleDirection, handle: string): void {
    this.selectNode(portSelectionToken(nodeId, direction, handle));
  }

  /** 选中 Loop structured region（不进 history）。 */
  selectLoopRegion(loopNodeId: string): void {
    this.selectNode(loopRegionSelectionToken(loopNodeId));
  }

  /** 设置意图焦点（不进 history）。 */
  focusIntent(intent: StandardIntent | null): void {
    this.dispatch({ kind: 'intentFocus', intent });
  }

  /** 设置节点配置 patch。 */
  setNodeConfig(nodeId: string, patch: Record<string, unknown>): void {
    this.dispatch({ kind: 'setNodeConfig', nodeId, patch });
  }

  /** 替换 credential（一次性明文；不进 history）。 */
  credentialReplace(nodeId: string, jsonPointer: string, logicalName: string, value: string): void {
    this.dispatch({ kind: 'credentialReplace', nodeId, jsonPointer, logicalName, value });
  }

  /** 清除 credential。 */
  credentialClear(nodeId: string, jsonPointer: string, logicalName: string): void {
    this.dispatch({ kind: 'credentialClear', nodeId, jsonPointer, logicalName });
  }

  /** 设置 diagnostics（校验/编译后）。 */
  setDiagnostics(diagnostics: InstallDiagnostic[]): void {
    this.dispatch({ kind: 'setDiagnostics', diagnostics });
  }

  // -----------------------------------------------------------------------
  // 添加/删除节点
  // -----------------------------------------------------------------------

  /**
   * 添加节点到 definition.flow.nodes。
   *
   * 返回新节点的 uuid。
   */
  addNode(kind: FlowNodeKind, configValue?: Record<string, unknown>): string {
    const id = crypto.randomUUID();
    const node: FlowNode = {
      id,
      config: { kind, value: configValue ?? {} },
    };
    this.dispatch({ kind: 'nodeAdd', node });
    this.selectNode(id);
    return id;
  }

  /**
   * 删除节点及相关边。
   *
   * 引用边与节点一起作为一个 semantic history command 回放。
   */
  deleteNode(nodeId: string): void {
    this.dispatch({ kind: 'nodeDelete', nodeId });
    if (this.selection === nodeId) this.selectNode(null);
  }

  // -----------------------------------------------------------------------
  // 清理（teardown 外部订阅）
  // -----------------------------------------------------------------------

  /** 释放所有 wire 事件订阅与内部资源。 */
  cleanup(): void {
    for (const fn of this.#cleanups) {
      try {
        fn();
      } catch {
        /* 静默清理 */
      }
    }
    this.#cleanups = [];
  }
}
