//! 原生规则作者核心：immutable state + reducer。
//!
//! 纯 TS，无 runes、无 .svelte.ts。所有变更通过 `reduce(state, action)` 返回新 state；
//! 历史（history/redo）只记录可回放的 semantic/layout 命令，selection/viewport/
//! intentFocus 等 transient 状态与 credential 值绝不进入历史。
//!
//! 不变量（实现于本模块并在 core.test.ts 覆盖）：
//! - credential 值不进 action/history 持久化；credentialReplace/Clear 携带一次性明文，
//!   入队后在 saveRequest 快照进 inFlightSave 并立即清空，不跨 save 驻留。
//! - selection/viewport/intentFocus 不进 history。
//! - semantic 命令标记 domain；undo/redo 只回放 semantic/layout 命令。
//! - epoch 在 saveRequest 时递增并快照；saveResponse 校验快照仍为当前 epoch 才接受
//!   （stale response 拒绝），接受后再递增一次（同一响应重复投递同样被拒）。
//! - semantic action 清 candidate；layout action 不清。
//! - saveRequest 按当前 dirty 域拍快照；saveResponse 按快照分域 merge
//!   （编辑中的域保持 dirty，未编辑的域收敛为已保存）。

import type {
  CapabilityManifest,
  CredentialMutationRequest,
  FlowEdge,
  FlowNode,
  FlowNodeKind,
  InstallDiagnostic,
  IntentExport,
  RevisionConflict,
  RuleDefinition,
  SaveNativeRuleDocumentOutcome,
  SourceIdentity,
  StandardIntent,
} from './wire';

/** 节点画布坐标。 */
export interface Position {
  x: number;
  y: number;
}

/**
 * 布局文档最小结构（core 内部解释）。
 *
 * wire 侧 layout 为 `layout_json` 字符串；core 的 `layout` 字段按合同为 `unknown`，
 * 非 null 时解释为本结构。保存时由上层 `JSON.stringify` 回传。
 */
export interface NativeDocumentLayout {
  nodes: Record<string, { position: Position; collapsed: boolean }>;
  loopRegions?: Record<string, { collapsed: boolean }>;
}

/** 定义顶层可编辑字段（set-field 语义命令的目标）。 */
export type DefinitionField =
  'base_url' | 'source_identity' | 'source_id_rules' | 'capability_manifest';

export type DefinitionFieldValue = string | SourceIdentity | string[] | CapabilityManifest;

/** 校验结果身份，避免旧诊断被误显示为当前 Definition 结果。 */
export type ValidationState = {
  status: 'unknown' | 'pending' | 'valid' | 'invalid' | 'stale' | 'error';
  revision: number | null;
  definitionHash: string | null;
  planHash: string | null;
  diagnostics: InstallDiagnostic[];
};

function unknownValidation(): ValidationState {
  return {
    status: 'unknown',
    revision: null,
    definitionHash: null,
    planHash: null,
    diagnostics: [],
  };
}

/**
 * 可回放命令（history / redo 栈条目）。
 *
 * 只包含 semantic/layout 域；selection/viewport/intentFocus/credential 不产生条目。
 */
export type TypedCommand =
  | {
      domain: 'semantic';
      kind: 'setField';
      field: DefinitionField;
      value: DefinitionFieldValue;
      prev: DefinitionFieldValue;
    }
  | {
      domain: 'semantic';
      kind: 'setIntentExport';
      intent: StandardIntent;
      value: IntentExport | null;
      prev: IntentExport | null;
    }
  | {
      domain: 'semantic';
      kind: 'setNodeConfig';
      nodeId: string;
      value: Record<string, unknown>;
      prev: Record<string, unknown>;
      edges: FlowEdge[];
      prevEdges: FlowEdge[];
    }
  | {
      domain: 'semantic';
      kind: 'nodeAdd';
      node: FlowNode;
    }
  | {
      domain: 'semantic';
      kind: 'nodeDelete';
      node: FlowNode;
      edges: FlowEdge[];
    }
  | {
      domain: 'semantic';
      kind: 'nodeDeleteMany';
      nodes: FlowNode[];
      edges: FlowEdge[];
    }
  | {
      domain: 'semantic';
      kind: 'deleteSelection';
      nodes: FlowNode[];
      edges: FlowEdge[];
    }
  | {
      domain: 'semantic';
      kind: 'edgeConnect';
      edge: FlowEdge;
      connected: boolean;
    }
  | {
      domain: 'semantic';
      kind: 'edgeReconnect';
      from: FlowEdge;
      to: FlowEdge;
    }
  | {
      domain: 'layout';
      kind: 'moveNode';
      nodeId: string;
      position: Position;
      prev: Position;
    }
  | {
      domain: 'layout';
      kind: 'layoutNodes';
      positions: Record<string, Position>;
      prev: Record<string, Position>;
    }
  | {
      domain: 'layout';
      kind: 'collapseNode';
      nodeId: string;
      collapsed: boolean;
      prev: boolean;
    }
  | {
      domain: 'layout';
      kind: 'collapseLoopRegion';
      loopNodeId: string;
      collapsed: boolean;
      prev: boolean;
    };

/** 保存中的域快照（edit-during-save 分域 merge 的依据）。 */
export interface SaveDomainSnapshot {
  /** 快照时已保存的 revision。 */
  revision: number;
  /** 快照时的域内容（definition 或 layout 的深拷贝）。 */
  content: unknown;
  /** 快照时已排队的 credential 变更（仅 semantic 域；成功保存后从队列移除）。 */
  credentialMutations: readonly CredentialMutationRequest[];
}

/** 在途保存记录。 */
export interface InFlightSave {
  epoch: number;
  semantic: SaveDomainSnapshot | null;
  layout: SaveDomainSnapshot | null;
}

/** 作者核心不可变状态。 */
export interface NativeRuleAuthoringState {
  /** 当前语义定义（TS 镜像；回传保存）。 */
  definition: RuleDefinition;
  /** 当前布局（不透明；core 只做快照比较与回传）。 */
  layout: unknown;
  /** 已保存的语义 revision；从未保存为 null。 */
  savedSemanticRevision: number | null;
  /** 已保存的布局 revision；从未保存为 null。 */
  savedLayoutRevision: number | null;
  /** 分域脏标记（semantic/layout 独立）。 */
  dirty: { semantic: boolean; layout: boolean };
  /** 保存代际；stale response 拒绝依据。 */
  epoch: number;
  /** 当前选中的节点 id；不进 history。 */
  selection: string | null;
  /** 当前意图焦点；不进 history。 */
  intentFocus: StandardIntent | null;
  /** 最近一次校验/编译结果及其 semantic revision/hash 身份。 */
  validation: ValidationState;
  /** 已暂存的安装候选；语义变更后失效。 */
  candidate: { id: string; expires_at_ms: number } | null;
  /** 分域乐观并发冲突。 */
  conflict: { semantic: RevisionConflict | null; layout: RevisionConflict | null };
  /** 可回放命令历史（undo 栈；仅 semantic/layout）。 */
  history: TypedCommand[];
  /** 已撤销命令栈（redo；仅 semantic/layout）。 */
  redo: TypedCommand[];
  /** 排队的 credential 变更（slot summary + 一次性明文；不进 history，saveRequest 后清空）。 */
  pendingCredentialMutations: CredentialMutationRequest[];
  /** 在途保存快照；编辑中继续可编辑。 */
  inFlightSave: InFlightSave | null;
}

/** 作者核心 action（不可变 reducer 输入）。 */
export type AuthoringAction =
  | { kind: 'setField'; field: DefinitionField; value: DefinitionFieldValue }
  | { kind: 'setIntentExport'; intent: StandardIntent; value: IntentExport | null }
  | { kind: 'setNodeConfig'; nodeId: string; patch: Record<string, unknown> }
  | { kind: 'nodeAdd'; node: FlowNode }
  | { kind: 'nodeDelete'; nodeId: string }
  | { kind: 'nodesDelete'; nodeIds: string[] }
  | { kind: 'deleteSelection'; nodeIds: string[]; edgeIds: string[] }
  | { kind: 'edgeConnect'; edge: FlowEdge; connected: boolean }
  | { kind: 'edgeReconnect'; from: FlowEdge; to: FlowEdge }
  | { kind: 'moveNode'; nodeId: string; position: Position }
  | { kind: 'layoutNodes'; positions: Record<string, Position> }
  | { kind: 'collapseNode'; nodeId: string; collapsed: boolean }
  | { kind: 'collapseLoopRegion'; loopNodeId: string; collapsed: boolean }
  | { kind: 'viewport'; viewport: unknown }
  | { kind: 'selection'; nodeId: string | null }
  | { kind: 'intentFocus'; intent: StandardIntent | null }
  | { kind: 'setValidation'; validation: ValidationState }
  | { kind: 'setCandidate'; candidate: NativeRuleAuthoringState['candidate'] }
  | { kind: 'undo' }
  | { kind: 'redo' }
  | {
      kind: 'credentialReplace';
      nodeId: string;
      jsonPointer: string;
      logicalName: string;
      value: string;
    }
  | { kind: 'credentialClear'; nodeId: string; jsonPointer: string; logicalName: string }
  | { kind: 'saveRequest' }
  | { kind: 'saveResponse'; epoch: number; outcome: SaveNativeRuleDocumentOutcome }
  | { kind: 'saveFailure'; epoch: number }
  | { kind: 'reset'; state: NativeRuleAuthoringState };

/** 空能力清单（对应 Rust `CapabilityManifest::default()`）。 */
export function defaultCapabilityManifest(): CapabilityManifest {
  return {
    required: { network: false, system: { fs: false, env: false, process: false } },
  };
}

/** 构造 blank 定义（对应 RuleSystem create blank：空图、无导出、默认清单）。 */
export function createBlankDefinition(sourceIdentity: string): RuleDefinition {
  return {
    contract: 'rule_definition',
    schema_version: 1,
    source_identity: sourceIdentity,
    base_url: '',
    intent_exports: {},
    flow: { nodes: [], edges: [] },
    capability_manifest: defaultCapabilityManifest(),
    source_id_rules: [],
  };
}

/** 初始状态。 */
export function createInitialState(options: {
  definition: RuleDefinition;
  layout?: unknown;
  savedSemanticRevision?: number | null;
  savedLayoutRevision?: number | null;
}): NativeRuleAuthoringState {
  const savedSemanticRevision = options.savedSemanticRevision ?? null;
  const savedLayoutRevision = options.savedLayoutRevision ?? null;
  return {
    definition: cloneJson(options.definition),
    layout: options.layout === undefined ? null : cloneJson(options.layout),
    savedSemanticRevision,
    savedLayoutRevision,
    dirty: { semantic: false, layout: false },
    epoch: 0,
    selection: null,
    intentFocus: null,
    validation: unknownValidation(),
    candidate: null,
    conflict: { semantic: null, layout: null },
    history: [],
    redo: [],
    pendingCredentialMutations: [],
    inFlightSave: null,
  };
}

/** 深拷贝 JSON 结构（definition/layout 均为纯 JSON）。 */
export function cloneJson<T>(value: T): T {
  if (value === null || value === undefined) return value;
  try {
    return structuredClone(value);
  } catch {
    throw new Error('无法复制 JSON 状态');
  }
}

/** 递归 JSON 深比较（对象键序无关）。 */
export function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== typeof b) return false;
  if (a === null || b === null) return a === b;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (Array.isArray(a)) {
    const left = a as unknown[];
    const right = b as unknown[];
    if (left.length !== right.length) return false;
    return left.every((item, index) => deepEqual(item, right[index]));
  }
  if (typeof a === 'object') {
    const left = a as Record<string, unknown>;
    const right = b as Record<string, unknown>;
    const leftKeys = Object.keys(left);
    const rightKeys = Object.keys(right);
    if (leftKeys.length !== rightKeys.length) return false;
    return leftKeys.every((key) => key in right && deepEqual(left[key], right[key]));
  }
  return false;
}

/** 读取布局；非 NativeDocumentLayout 结构时返回 null（core 不解释未知布局）。 */
export function readLayout(layout: unknown): NativeDocumentLayout | null {
  if (layout === null || layout === undefined) return null;
  if (typeof layout !== 'object') return null;
  const candidate = layout as { nodes?: unknown };
  if (candidate.nodes === null || typeof candidate.nodes !== 'object') return null;
  return layout as NativeDocumentLayout;
}

/** 布局节点的位置与折叠状态读取；节点缺失时回退默认。 */
function layoutNode(
  layout: NativeDocumentLayout | null,
  nodeId: string,
): { position: Position; collapsed: boolean } {
  const entry = layout?.nodes[nodeId];
  if (!entry) return { position: { x: 0, y: 0 }, collapsed: false };
  return { position: { ...entry.position }, collapsed: entry.collapsed };
}

function loopRegionLayout(
  layout: NativeDocumentLayout | null,
  loopNodeId: string,
): { collapsed: boolean } {
  return layout?.loopRegions?.[loopNodeId] ?? { collapsed: false };
}

/** 应用命令的正向效果（dispatch 与 redo 共用）。 */
function applyCommand(
  state: NativeRuleAuthoringState,
  command: TypedCommand,
): NativeRuleAuthoringState {
  switch (command.kind) {
    case 'setField':
      return {
        ...state,
        definition: setDefinitionField(state.definition, command.field, command.value),
      };
    case 'setIntentExport':
      return {
        ...state,
        definition: setIntentExport(state.definition, command.intent, command.value),
      };
    case 'setNodeConfig': {
      const definition = setNodeConfigValue(state.definition, command.nodeId, command.value);
      return {
        ...state,
        definition: {
          ...definition,
          flow: { ...definition.flow, edges: cloneJson(command.edges) },
        },
      };
    }
    case 'nodeAdd': {
      if (state.definition.flow.nodes.some((node) => node.id === command.node.id)) return state;
      return {
        ...state,
        definition: {
          ...state.definition,
          flow: {
            ...state.definition.flow,
            nodes: [...state.definition.flow.nodes, cloneJson(command.node)],
          },
        },
      };
    }
    case 'nodeDelete': {
      const nodes = state.definition.flow.nodes.filter((node) => node.id !== command.node.id);
      return {
        ...state,
        definition: {
          ...state.definition,
          flow: {
            ...state.definition.flow,
            nodes,
            edges: state.definition.flow.edges.filter(
              (edge) =>
                edge.from.node_id !== command.node.id && edge.to.node_id !== command.node.id,
            ),
          },
        },
      };
    }
    case 'nodeDeleteMany': {
      const nodeIds = new Set(command.nodes.map((node) => node.id));
      return {
        ...state,
        definition: {
          ...state.definition,
          flow: {
            ...state.definition.flow,
            nodes: state.definition.flow.nodes.filter((node) => !nodeIds.has(node.id)),
            edges: state.definition.flow.edges.filter(
              (edge) => !nodeIds.has(edge.from.node_id) && !nodeIds.has(edge.to.node_id),
            ),
          },
        },
      };
    }
    case 'deleteSelection': {
      const nodeIds = new Set(command.nodes.map((node) => node.id));
      const edgeIds = new Set(command.edges.map(edgeIdentity));
      return {
        ...state,
        definition: {
          ...state.definition,
          flow: {
            ...state.definition.flow,
            nodes: state.definition.flow.nodes.filter((node) => !nodeIds.has(node.id)),
            edges: state.definition.flow.edges.filter(
              (edge) =>
                !nodeIds.has(edge.from.node_id) &&
                !nodeIds.has(edge.to.node_id) &&
                !edgeIds.has(edgeIdentity(edge)),
            ),
          },
        },
      };
    }
    case 'edgeConnect': {
      const edges = command.connected
        ? addEdge(state.definition.flow.edges, command.edge)
        : removeEdge(state.definition.flow.edges, command.edge);
      return {
        ...state,
        definition: { ...state.definition, flow: { ...state.definition.flow, edges } },
      };
    }
    case 'edgeReconnect': {
      const edges = addEdge(removeEdge(state.definition.flow.edges, command.from), command.to);
      return {
        ...state,
        definition: { ...state.definition, flow: { ...state.definition.flow, edges } },
      };
    }
    case 'moveNode': {
      const layout = readLayout(state.layout) ?? { nodes: {} };
      const current = layoutNode(layout, command.nodeId);
      const nodes = {
        ...layout.nodes,
        [command.nodeId]: { ...current, position: command.position },
      };
      return { ...state, layout: { ...layout, nodes } };
    }
    case 'layoutNodes': {
      const layout = readLayout(state.layout) ?? { nodes: {} };
      const nodes = { ...layout.nodes };
      for (const [nodeId, position] of Object.entries(command.positions)) {
        const current = layoutNode(layout, nodeId);
        nodes[nodeId] = { ...current, position: { ...position } };
      }
      return { ...state, layout: { ...layout, nodes } };
    }
    case 'collapseNode': {
      const layout = readLayout(state.layout) ?? { nodes: {} };
      const current = layoutNode(layout, command.nodeId);
      const nodes = {
        ...layout.nodes,
        [command.nodeId]: { ...current, collapsed: command.collapsed },
      };
      return { ...state, layout: { ...layout, nodes } };
    }
    case 'collapseLoopRegion': {
      const layout = readLayout(state.layout) ?? { nodes: {} };
      const loopRegions = {
        ...layout.loopRegions,
        [command.loopNodeId]: { collapsed: command.collapsed },
      };
      return { ...state, layout: { ...layout, loopRegions } };
    }
  }
}

/** 应用命令的逆向效果（undo 用）。 */
function revertCommand(
  state: NativeRuleAuthoringState,
  command: TypedCommand,
): NativeRuleAuthoringState {
  switch (command.kind) {
    case 'setField':
      return {
        ...state,
        definition: setDefinitionField(state.definition, command.field, command.prev),
      };
    case 'setIntentExport':
      return {
        ...state,
        definition: setIntentExport(state.definition, command.intent, command.prev),
      };
    case 'setNodeConfig': {
      const definition = setNodeConfigValue(state.definition, command.nodeId, command.prev);
      return {
        ...state,
        definition: {
          ...definition,
          flow: { ...definition.flow, edges: cloneJson(command.prevEdges) },
        },
      };
    }
    case 'nodeAdd': {
      const nodes = state.definition.flow.nodes.filter((node) => node.id !== command.node.id);
      return {
        ...state,
        definition: {
          ...state.definition,
          flow: {
            ...state.definition.flow,
            nodes,
            edges: state.definition.flow.edges.filter(
              (edge) =>
                edge.from.node_id !== command.node.id && edge.to.node_id !== command.node.id,
            ),
          },
        },
      };
    }
    case 'nodeDelete': {
      const nodes = state.definition.flow.nodes.some((node) => node.id === command.node.id)
        ? state.definition.flow.nodes
        : [...state.definition.flow.nodes, cloneJson(command.node)];
      return {
        ...state,
        definition: {
          ...state.definition,
          flow: {
            ...state.definition.flow,
            nodes,
            edges: command.edges.reduce(addEdge, state.definition.flow.edges),
          },
        },
      };
    }
    case 'nodeDeleteMany': {
      const nodes = [...state.definition.flow.nodes];
      for (const node of command.nodes) {
        if (!nodes.some((current) => current.id === node.id)) nodes.push(cloneJson(node));
      }
      return {
        ...state,
        definition: {
          ...state.definition,
          flow: {
            ...state.definition.flow,
            nodes,
            edges: command.edges.reduce(addEdge, state.definition.flow.edges),
          },
        },
      };
    }
    case 'deleteSelection': {
      const nodes = [...state.definition.flow.nodes];
      for (const node of command.nodes) {
        if (!nodes.some((current) => current.id === node.id)) nodes.push(cloneJson(node));
      }
      return {
        ...state,
        definition: {
          ...state.definition,
          flow: {
            ...state.definition.flow,
            nodes,
            edges: command.edges.reduce(addEdge, state.definition.flow.edges),
          },
        },
      };
    }
    case 'edgeConnect': {
      const edges = command.connected
        ? removeEdge(state.definition.flow.edges, command.edge)
        : addEdge(state.definition.flow.edges, command.edge);
      return {
        ...state,
        definition: { ...state.definition, flow: { ...state.definition.flow, edges } },
      };
    }
    case 'edgeReconnect': {
      const edges = addEdge(removeEdge(state.definition.flow.edges, command.to), command.from);
      return {
        ...state,
        definition: { ...state.definition, flow: { ...state.definition.flow, edges } },
      };
    }
    case 'moveNode': {
      const layout = readLayout(state.layout) ?? { nodes: {} };
      const current = layoutNode(layout, command.nodeId);
      const nodes = { ...layout.nodes, [command.nodeId]: { ...current, position: command.prev } };
      return { ...state, layout: { ...layout, nodes } };
    }
    case 'layoutNodes': {
      const layout = readLayout(state.layout) ?? { nodes: {} };
      const nodes = { ...layout.nodes };
      for (const [nodeId, position] of Object.entries(command.prev)) {
        const current = layoutNode(layout, nodeId);
        nodes[nodeId] = { ...current, position: { ...position } };
      }
      return { ...state, layout: { ...layout, nodes } };
    }
    case 'collapseNode': {
      const layout = readLayout(state.layout) ?? { nodes: {} };
      const current = layoutNode(layout, command.nodeId);
      const nodes = { ...layout.nodes, [command.nodeId]: { ...current, collapsed: command.prev } };
      return { ...state, layout: { ...layout, nodes } };
    }
    case 'collapseLoopRegion': {
      const layout = readLayout(state.layout) ?? { nodes: {} };
      const loopRegions = { ...layout.loopRegions };
      if (command.prev) {
        loopRegions[command.loopNodeId] = { collapsed: true };
      } else {
        delete loopRegions[command.loopNodeId];
      }
      if (Object.keys(loopRegions).length === 0) {
        const layoutWithoutRegions = { ...layout };
        delete layoutWithoutRegions.loopRegions;
        return { ...state, layout: layoutWithoutRegions };
      }
      return {
        ...state,
        layout: { ...layout, loopRegions },
      };
    }
  }
}

/** 顶层定义字段写入（set-field 前向/回退共用）。 */
function setDefinitionField(
  definition: RuleDefinition,
  field: DefinitionField,
  value: DefinitionFieldValue,
): RuleDefinition {
  switch (field) {
    case 'base_url':
      return { ...definition, base_url: value as string };
    case 'source_identity':
      return { ...definition, source_identity: value as SourceIdentity | string };
    case 'source_id_rules':
      return { ...definition, source_id_rules: cloneJson(value as string[]) };
    case 'capability_manifest':
      return { ...definition, capability_manifest: cloneJson(value as CapabilityManifest) };
  }
}

/** 意图导出写入（null = 移除）。 */
function setIntentExport(
  definition: RuleDefinition,
  intent: StandardIntent,
  value: IntentExport | null,
): RuleDefinition {
  const intentExports = { ...definition.intent_exports };
  if (value === null) {
    delete intentExports[intent];
  } else {
    intentExports[intent] = cloneJson(value);
  }
  return { ...definition, intent_exports: intentExports };
}

/** 节点 config.value 整体写入（前向/回退共用；patch 合并由 dispatch 层完成）。 */
function setNodeConfigValue(
  definition: RuleDefinition,
  nodeId: string,
  value: Record<string, unknown>,
): RuleDefinition {
  const nodes = definition.flow.nodes.map((node) =>
    node.id === nodeId ? { ...node, config: { ...node.config, value: cloneJson(value) } } : node,
  );
  return { ...definition, flow: { ...definition.flow, nodes } };
}

/** 按 from+to 语义 identity 去重添加边。 */
function addEdge(edges: FlowEdge[], edge: FlowEdge): FlowEdge[] {
  if (edges.some((existing) => edgeIdentity(existing) === edgeIdentity(edge))) return edges;
  return [...edges, cloneJson(edge)];
}

/** 按 from+to 语义 identity 移除边。 */
function removeEdge(edges: FlowEdge[], edge: FlowEdge): FlowEdge[] {
  const identity = edgeIdentity(edge);
  return edges.filter((existing) => edgeIdentity(existing) !== identity);
}

function edgeIdentity(edge: FlowEdge): string {
  return `${edge.from.node_id}:${edge.from.handle}->${edge.to.node_id}:${edge.to.handle}`;
}

/** 读取会生成动态 semantic handle 的闭集配置。 */
function dynamicHandles(kind: FlowNodeKind, config: Record<string, unknown>): string[] {
  if (kind === 'condition') {
    return Array.isArray(config.branches)
      ? config.branches.filter(
          (value): value is string => typeof value === 'string' && value.length > 0,
        )
      : [];
  }
  if (kind === 'merge') {
    return Array.isArray(config.inputs)
      ? [...config.inputs]
          .filter(
            (value): value is Record<string, unknown> =>
              typeof value === 'object' && value !== null,
          )
          .sort((left, right) => {
            const leftOrder = typeof left.order === 'number' ? left.order : 0;
            const rightOrder = typeof right.order === 'number' ? right.order : 0;
            return leftOrder - rightOrder;
          })
          .map((value) => (typeof value.handle === 'string' ? value.handle : ''))
          .filter((value) => value.length > 0)
      : [];
  }
  return [];
}

/**
 * 动态端口变更时迁移同序重命名的 handle，并移除已不存在的 handle 边。
 * 同一命令保存 config 与 edges，保证一次 undo 恢复完整 canonical 状态。
 */
function reconcileDynamicEdges(
  nodeId: string,
  kind: FlowNodeKind,
  previousConfig: Record<string, unknown>,
  nextConfig: Record<string, unknown>,
  edges: FlowEdge[],
): FlowEdge[] {
  if (kind !== 'condition' && kind !== 'merge') return cloneJson(edges);
  const previousHandles = dynamicHandles(kind, previousConfig);
  const nextHandles = dynamicHandles(kind, nextConfig);
  const nextSet = new Set(nextHandles);
  const previousSet = new Set(previousHandles);
  const migrations = new Map<string, string>();
  previousHandles.forEach((handle, index) => {
    if (nextSet.has(handle)) return;
    const replacement = nextHandles[index];
    if (replacement && !previousSet.has(replacement)) migrations.set(handle, replacement);
  });
  const output: FlowEdge[] = [];
  for (const edge of edges) {
    let nextEdge = edge;
    if (kind === 'condition' && edge.from.node_id === nodeId) {
      const handle = migrations.get(edge.from.handle) ?? edge.from.handle;
      if (!nextSet.has(handle)) continue;
      nextEdge = { ...edge, from: { ...edge.from, handle } };
    } else if (kind === 'merge' && edge.to.node_id === nodeId) {
      const handle = migrations.get(edge.to.handle) ?? edge.to.handle;
      if (!nextSet.has(handle)) continue;
      nextEdge = { ...edge, to: { ...edge.to, handle } };
    }
    if (!output.some((current) => edgeIdentity(current) === edgeIdentity(nextEdge))) {
      output.push(cloneJson(nextEdge));
    }
  }
  return output;
}

/** 语义 action 统一副作用：清 candidate、置 semantic dirty、清 redo。 */
function markSemanticDirty(state: NativeRuleAuthoringState): NativeRuleAuthoringState {
  return {
    ...state,
    dirty: { ...state.dirty, semantic: true },
    validation: {
      ...state.validation,
      status: state.validation.revision === null ? 'unknown' : 'stale',
    },
    candidate: null,
    redo: [],
  };
}

/** 布局 action 统一副作用：置 layout dirty、清 redo；candidate 保持。 */
function markLayoutDirty(state: NativeRuleAuthoringState): NativeRuleAuthoringState {
  return {
    ...state,
    dirty: { ...state.dirty, layout: true },
    redo: [],
  };
}

/** semantic/layout 命令的窄化类型。 */
type SemanticCommand = Extract<TypedCommand, { domain: 'semantic' }>;
type LayoutCommand = Extract<TypedCommand, { domain: 'layout' }>;

/**
 * layout 命令入 history：连续同节点 move/collapse 合并为一条历史。
 *
 * 合并时保留首次操作前的 `prev`（undo 一次回到整个拖动序列之前）。
 */
function coalesceLayout(history: TypedCommand[], command: LayoutCommand): TypedCommand[] {
  const last = history[history.length - 1];
  if (!last || last.domain !== 'layout' || last.kind !== command.kind) {
    return [...history, command];
  }
  if (last.kind === 'moveNode' && command.kind === 'moveNode' && last.nodeId === command.nodeId) {
    return [...history.slice(0, -1), { ...command, prev: last.prev }];
  }
  if (
    last.kind === 'collapseNode' &&
    command.kind === 'collapseNode' &&
    last.nodeId === command.nodeId
  ) {
    return [...history.slice(0, -1), { ...command, prev: last.prev }];
  }
  if (last.kind === 'collapseLoopRegion' && command.kind === 'collapseLoopRegion') {
    if (last.loopNodeId === command.loopNodeId) {
      return [...history.slice(0, -1), { ...command, prev: last.prev }];
    }
  }
  return [...history, command];
}

function sanitizeSelection(
  state: NativeRuleAuthoringState,
  command: SemanticCommand,
): NativeRuleAuthoringState {
  let selection = state.selection;
  if (selection?.startsWith('edge:')) {
    const edgeId = selection.slice('edge:'.length);
    if (!state.definition.flow.edges.some((edge) => edgeIdentity(edge) === edgeId))
      selection = null;
  }
  if (selection && !selection.startsWith('edge:') && !selection.startsWith('port:')) {
    if (!state.definition.flow.nodes.some((node) => node.id === selection)) selection = null;
  }
  if (selection?.startsWith('port:')) {
    const encodedNodeId = selection.slice('port:'.length).split(':', 1)[0];
    try {
      const nodeId = decodeURIComponent(encodedNodeId);
      if (!state.definition.flow.nodes.some((node) => node.id === nodeId)) selection = null;
    } catch {
      selection = null;
    }
  }
  if (
    command.kind === 'setNodeConfig' &&
    selection?.startsWith(`port:${encodeURIComponent(command.nodeId)}:`)
  ) {
    selection = null;
  }
  if (command.kind === 'edgeReconnect' && selection === `edge:${edgeIdentity(command.from)}`) {
    selection = `edge:${edgeIdentity(command.to)}`;
  }
  return selection === state.selection ? state : { ...state, selection };
}

/** 语义命令执行入口：应用 + 入 history（语义命令不合并）。 */
function pushSemanticCommand(
  state: NativeRuleAuthoringState,
  command: SemanticCommand,
): NativeRuleAuthoringState {
  const applied = applyCommand(state, command);
  const dirty = markSemanticDirty(applied);
  const sanitized = sanitizeSelection(dirty, command);
  return { ...sanitized, history: [...sanitized.history, command] };
}

/** 布局命令执行入口：应用 + 入 history（layout 连续操作合并为一条历史）。 */
function pushLayoutCommand(
  state: NativeRuleAuthoringState,
  command: LayoutCommand,
): NativeRuleAuthoringState {
  const applied = applyCommand(state, command);
  const dirty = markLayoutDirty(applied);
  return { ...dirty, history: coalesceLayout(dirty.history, command) };
}

/** undo：弹出最后一条 semantic/layout 命令并回退；transient/credential 不受影响。 */
function undo(state: NativeRuleAuthoringState): NativeRuleAuthoringState {
  const command = state.history[state.history.length - 1];
  if (!command) return state;
  const reverted = revertCommand(state, command);
  const dirty =
    command.domain === 'semantic' ? markSemanticDirty(reverted) : markLayoutDirty(reverted);
  return {
    ...dirty,
    history: state.history.slice(0, -1),
    redo: [...state.redo, command],
  };
}

/** redo：重放最近一条撤销的命令。 */
function redo(state: NativeRuleAuthoringState): NativeRuleAuthoringState {
  const command = state.redo[state.redo.length - 1];
  if (!command) return state;
  const applied = applyCommand(state, command);
  const dirty =
    command.domain === 'semantic' ? markSemanticDirty(applied) : markLayoutDirty(applied);
  return {
    ...dirty,
    history: [...state.history, command],
    redo: state.redo.slice(0, -1),
  };
}

/** credential 变更入队：携带一次性明文；不进 history；saveRequest 快照后立即清空。 */
function pushCredentialMutation(
  state: NativeRuleAuthoringState,
  mutation: CredentialMutationRequest,
): NativeRuleAuthoringState {
  return markSemanticDirty({
    ...state,
    pendingCredentialMutations: [...state.pendingCredentialMutations, mutation],
  });
}

/** saveRequest：对每个 dirty 域拍快照，递增并快照 epoch；credential 明文快照后立即清空。 */
function saveRequest(state: NativeRuleAuthoringState): NativeRuleAuthoringState {
  if (!state.dirty.semantic && !state.dirty.layout) return state;
  const epoch = state.epoch + 1;
  return {
    ...state,
    epoch,
    inFlightSave: {
      epoch,
      semantic: state.dirty.semantic
        ? {
            revision: state.savedSemanticRevision ?? 0,
            content: cloneJson(state.definition),
            credentialMutations: [...state.pendingCredentialMutations],
          }
        : null,
      layout: state.dirty.layout
        ? {
            revision: state.savedLayoutRevision ?? 0,
            content: cloneJson(state.layout),
            credentialMutations: [],
          }
        : null,
    },
    // 一次性：明文只存在于 inFlightSave 快照；响应后不保留，冲突时由 UI 重新输入。
    pendingCredentialMutations: [],
  };
}

/** saveResponse：校验 epoch；按快照分域 merge（编辑中的域保持 dirty）。 */
function saveResponse(
  state: NativeRuleAuthoringState,
  epoch: number,
  outcome: SaveNativeRuleDocumentOutcome,
): NativeRuleAuthoringState {
  const inFlight = state.inFlightSave;
  if (!inFlight || epoch !== inFlight.epoch || epoch !== state.epoch) return state;

  let next = state;

  if (outcome.semantic && inFlight.semantic) {
    const snapshot = inFlight.semantic;
    const result = outcome.semantic;
    if (result.conflict) {
      // 乐观并发冲突：revision 不推进、pending 已在 saveRequest 清空（明文一次性），
      // 冲突时由 UI 重新输入；candidate 失效。
      next = {
        ...next,
        conflict: { ...next.conflict, semantic: result.conflict },
        candidate: null,
      };
    } else {
      // 成功：pending 已在 saveRequest 清空；当前 pending 均为响应期间新输入，保留。
      const drift = !deepEqual(next.definition, snapshot.content);
      next = {
        ...next,
        savedSemanticRevision: result.revision,
        conflict: { ...next.conflict, semantic: null },
        dirty: {
          ...next.dirty,
          semantic: drift || next.pendingCredentialMutations.length > 0,
        },
        candidate: null,
      };
    }
  }

  if (outcome.layout && inFlight.layout) {
    const snapshot = inFlight.layout;
    const result = outcome.layout;
    if (result.conflict) {
      next = { ...next, conflict: { ...next.conflict, layout: result.conflict } };
    } else {
      const drift = !deepEqual(next.layout, snapshot.content);
      next = {
        ...next,
        savedLayoutRevision: result.revision,
        conflict: { ...next.conflict, layout: null },
        dirty: { ...next.dirty, layout: drift },
      };
    }
  }

  // 响应已接受：epoch 再递增（同一响应重复投递同样被拒），在途快照清空。
  return { ...next, epoch: next.epoch + 1, inFlightSave: null };
}

/** 保存调用失败：清理 in-flight，保留 dirty，且不恢复一次性 credential 明文。 */
function saveFailure(state: NativeRuleAuthoringState, epoch: number): NativeRuleAuthoringState {
  const inFlight = state.inFlightSave;
  if (!inFlight || inFlight.epoch !== epoch || state.epoch !== epoch) return state;
  return { ...state, epoch: state.epoch + 1, inFlightSave: null };
}

/** 不可变 reducer。 */
export function reduce(
  state: NativeRuleAuthoringState,
  action: AuthoringAction,
): NativeRuleAuthoringState {
  switch (action.kind) {
    case 'setField': {
      const prev = definitionFieldValue(state.definition, action.field);
      if (deepEqual(prev, action.value)) return state;
      return pushSemanticCommand(state, {
        domain: 'semantic',
        kind: 'setField',
        field: action.field,
        value: cloneJson(action.value),
        prev,
      });
    }
    case 'setIntentExport': {
      const prev = state.definition.intent_exports[action.intent] ?? null;
      if (deepEqual(prev, action.value)) return state;
      return pushSemanticCommand(state, {
        domain: 'semantic',
        kind: 'setIntentExport',
        intent: action.intent,
        value: action.value === null ? null : cloneJson(action.value),
        prev: prev === null ? null : cloneJson(prev),
      });
    }
    case 'setNodeConfig': {
      const node = state.definition.flow.nodes.find((candidate) => candidate.id === action.nodeId);
      if (!node) return state;
      const merged = { ...node.config.value, ...action.patch };
      const edges = reconcileDynamicEdges(
        node.id,
        node.config.kind,
        node.config.value,
        merged,
        state.definition.flow.edges,
      );
      if (deepEqual(node.config.value, merged) && deepEqual(state.definition.flow.edges, edges)) {
        return state;
      }
      return pushSemanticCommand(state, {
        domain: 'semantic',
        kind: 'setNodeConfig',
        nodeId: action.nodeId,
        value: merged,
        prev: cloneJson(node.config.value),
        edges: cloneJson(edges),
        prevEdges: cloneJson(state.definition.flow.edges),
      });
    }
    case 'nodeAdd': {
      if (state.definition.flow.nodes.some((node) => node.id === action.node.id)) return state;
      return pushSemanticCommand(state, {
        domain: 'semantic',
        kind: 'nodeAdd',
        node: cloneJson(action.node),
      });
    }
    case 'nodeDelete': {
      const node = state.definition.flow.nodes.find((candidate) => candidate.id === action.nodeId);
      if (!node) return state;
      return pushSemanticCommand(state, {
        domain: 'semantic',
        kind: 'nodeDelete',
        node: cloneJson(node),
        edges: cloneJson(
          state.definition.flow.edges.filter(
            (edge) => edge.from.node_id === node.id || edge.to.node_id === node.id,
          ),
        ),
      });
    }
    case 'nodesDelete': {
      const nodeIds = new Set(action.nodeIds);
      const nodes = state.definition.flow.nodes.filter((node) => nodeIds.has(node.id));
      if (nodes.length === 0) return state;
      return pushSemanticCommand(state, {
        domain: 'semantic',
        kind: 'nodeDeleteMany',
        nodes: cloneJson(nodes),
        edges: cloneJson(
          state.definition.flow.edges.filter(
            (edge) => nodeIds.has(edge.from.node_id) || nodeIds.has(edge.to.node_id),
          ),
        ),
      });
    }
    case 'deleteSelection': {
      const nodeIds = new Set(action.nodeIds);
      const edgeIds = new Set(action.edgeIds);
      const nodes = state.definition.flow.nodes.filter((node) => nodeIds.has(node.id));
      const edges = state.definition.flow.edges.filter(
        (edge) =>
          nodeIds.has(edge.from.node_id) ||
          nodeIds.has(edge.to.node_id) ||
          edgeIds.has(edgeIdentity(edge)),
      );
      if (nodes.length === 0 && edges.length === 0) return state;
      return pushSemanticCommand(state, {
        domain: 'semantic',
        kind: 'deleteSelection',
        nodes: cloneJson(nodes),
        edges: cloneJson(edges),
      });
    }
    case 'edgeConnect': {
      const edges = state.definition.flow.edges;
      const identity = edgeIdentity(action.edge);
      const exists = edges.some((existing) => edgeIdentity(existing) === identity);
      if (exists === action.connected) return state;
      return pushSemanticCommand(state, {
        domain: 'semantic',
        kind: 'edgeConnect',
        edge: cloneJson(action.edge),
        connected: action.connected,
      });
    }
    case 'edgeReconnect': {
      const fromIdentity = edgeIdentity(action.from);
      const toIdentity = edgeIdentity(action.to);
      if (fromIdentity === toIdentity) return state;
      const fromExists = state.definition.flow.edges.some(
        (edge) => edgeIdentity(edge) === fromIdentity,
      );
      if (!fromExists) return state;
      return pushSemanticCommand(state, {
        domain: 'semantic',
        kind: 'edgeReconnect',
        from: cloneJson(action.from),
        to: cloneJson(action.to),
      });
    }
    case 'moveNode': {
      const layout = readLayout(state.layout) ?? { nodes: {} };
      const current = layoutNode(layout, action.nodeId);
      if (deepEqual(current.position, action.position)) return state;
      return pushLayoutCommand(state, {
        domain: 'layout',
        kind: 'moveNode',
        nodeId: action.nodeId,
        position: { ...action.position },
        prev: { ...current.position },
      });
    }
    case 'layoutNodes': {
      const nodeIds = new Set(state.definition.flow.nodes.map((node) => node.id));
      const positions = Object.fromEntries(
        Object.entries(action.positions).filter(
          ([nodeId, position]) =>
            nodeIds.has(nodeId) && Number.isFinite(position.x) && Number.isFinite(position.y),
        ),
      );
      const layout = readLayout(state.layout) ?? { nodes: {} };
      const prev = Object.fromEntries(
        Object.keys(positions).map((nodeId) => [nodeId, layoutNode(layout, nodeId).position]),
      );
      if (Object.keys(positions).length === 0 || deepEqual(positions, prev)) return state;
      return pushLayoutCommand(state, {
        domain: 'layout',
        kind: 'layoutNodes',
        positions: cloneJson(positions),
        prev: cloneJson(prev),
      });
    }
    case 'collapseNode': {
      const layout = readLayout(state.layout) ?? { nodes: {} };
      const current = layoutNode(layout, action.nodeId);
      if (current.collapsed === action.collapsed) return state;
      return pushLayoutCommand(state, {
        domain: 'layout',
        kind: 'collapseNode',
        nodeId: action.nodeId,
        collapsed: action.collapsed,
        prev: current.collapsed,
      });
    }
    case 'collapseLoopRegion': {
      const layout = readLayout(state.layout) ?? { nodes: {} };
      const current = loopRegionLayout(layout, action.loopNodeId);
      if (current.collapsed === action.collapsed) return state;
      return pushLayoutCommand(state, {
        domain: 'layout',
        kind: 'collapseLoopRegion',
        loopNodeId: action.loopNodeId,
        collapsed: action.collapsed,
        prev: current.collapsed,
      });
    }
    case 'viewport':
      // viewport 只改 transient 画布状态；core 不持有，返回原 state。
      return state;
    case 'selection':
      if (state.selection === action.nodeId) return state;
      return { ...state, selection: action.nodeId };
    case 'intentFocus':
      if (state.intentFocus === action.intent) return state;
      return { ...state, intentFocus: action.intent };
    case 'setValidation':
      return { ...state, validation: cloneJson(action.validation) };
    case 'setCandidate':
      return { ...state, candidate: action.candidate ? cloneJson(action.candidate) : null };
    case 'undo':
      return undo(state);
    case 'redo':
      return redo(state);
    case 'credentialReplace':
      return pushCredentialMutation(state, {
        node_id: action.nodeId,
        json_pointer: action.jsonPointer,
        logical_name: action.logicalName,
        action: 'replace',
        value: action.value,
      });
    case 'credentialClear':
      return pushCredentialMutation(state, {
        node_id: action.nodeId,
        json_pointer: action.jsonPointer,
        logical_name: action.logicalName,
        action: 'clear',
      });
    case 'saveRequest':
      return saveRequest(state);
    case 'saveResponse':
      return saveResponse(state, action.epoch, action.outcome);
    case 'saveFailure':
      return saveFailure(state, action.epoch);
    case 'reset':
      return cloneJson(action.state);
  }
}

/** 读取定义顶层字段（set-field 前向/回退共用）。 */
function definitionFieldValue(
  definition: RuleDefinition,
  field: DefinitionField,
): DefinitionFieldValue {
  switch (field) {
    case 'base_url':
      return definition.base_url;
    case 'source_identity':
      return definition.source_identity;
    case 'source_id_rules':
      return definition.source_id_rules;
    case 'capability_manifest':
      return definition.capability_manifest;
  }
}
