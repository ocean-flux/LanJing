//! 原生规则 Flow 投影 adapter（纯 TS，无 runes）。
//!
//! 只做两件事：
//! 1. `semanticToFlow`：把 canonical `RuleDefinition` 投影为 Svelte Flow 的 node/edge 形状。
//!    - 七类节点 kind → Svelte Flow custom node type（同名，画布按 kind 注册组件）。
//!    - node position 优先来自 layout（`NativeDocumentLayout`），未保存的节点按 kind
//!      确定性默认网格排布；绝不写回 definition hash。
//!    - edge id 由 semantic identity（`from.node_id:from.handle->to.node_id:to.handle`）
//!      确定性派生，与 core 的边去重 identity 完全一致；不产生每意图副本。
//!    - intentFocus 只影响 focused/dimmed 标记（可达性投影），不复制节点/边/配置。
//! 2. `flowToSemantic`：把画布 UI 变更映射为 core.ts 的 typed `AuthoringAction`。
//!
//! 本模块不持有任何状态；全部是纯函数，可直接在 Vitest 中测试。

import type { AuthoringAction, NativeDocumentLayout, Position } from './core';
import type {
  FlowEdge,
  FlowNodeKind,
  FlowPortRef,
  InstallDiagnostic,
  RuleDefinition,
  StandardIntent,
} from './wire';
import { getNodePorts, type PortRole } from '$lib/views/rules/nodes/ports';

// ---------------------------------------------------------------------------
// 投影类型
// ---------------------------------------------------------------------------

/** 画布布局状态（未保存时可为 null，节点回退默认网格）。 */
export type LayoutState = NativeDocumentLayout | null;

/** Svelte Flow custom node 的 data 载荷（携带语义身份与诊断/聚焦标记）。 */
export interface FlowNodeData {
  [key: string]: unknown;
  /** 语义节点 ID（与 definition.flow.nodes[].id 一致）。 */
  nodeId: string;
  /** 节点类型（七类闭集）。 */
  kind: FlowNodeKind;
  /** 节点配置最小镜像（只读消费）。 */
  config: Record<string, unknown>;
  /** 当前生效的诊断（severity/element 定位）。 */
  diagnostics: InstallDiagnostic[];
  /** 是否处于意图聚焦可达子图中。 */
  focused: boolean;
  /** 是否被意图聚焦淡化（非可达子图）。 */
  dimmed: boolean;
  /** 以该节点为 flow_entry 的标准意图。 */
  entryIntents?: StandardIntent[];
  /** 节点折叠状态（来自 layout）。 */
  collapsed: boolean;
  /** 当前选中的可视端口；端口选择不把节点标记为 selected。 */
  selectedPort?: { direction: FlowHandleDirection; id: string };
  /** 端口行的键盘/点击选择回调；由画布注入，不进入语义定义。 */
  onPortSelect?: (direction: FlowHandleDirection, id: string) => void;
}

/** Svelte Flow custom edge 的 data 载荷（画布据此重建 semantic 边）。 */
export interface FlowEdgeData {
  [key: string]: unknown;
  /** 完整 semantic 边（from/to port 引用）。 */
  edge: FlowEdge;
  /** 是否被意图聚焦淡化。 */
  dimmed: boolean;
  selected: boolean;
  role: PortRole;
  /** 边路由车道；不改变 semantic edge，只影响可读性。 */
  lane: 'main' | 'branch' | 'auxiliary' | 'loop';
  valueKind: string;
  label?: string;
  route: 'normal' | 'loop-back';
}

/** Svelte Flow node 形状（与 @xyflow/svelte `Node<FlowNodeData>` 结构兼容）。 */
export interface FlowViewNode {
  id: string;
  /** custom node type = 节点 kind（画布注册同名组件）。 */
  type: FlowNodeKind;
  position: Position;
  data: FlowNodeData;
  /** 受控选中标记（对应 core selection）。 */
  selected?: boolean;
  hidden?: boolean;
}

/** Svelte Flow edge 形状（与 @xyflow/svelte `Edge<FlowEdgeData>` 结构兼容）。 */
export interface FlowViewEdge {
  /** 由 semantic identity 确定性派生的展示 id（不写回 definition）。 */
  id: string;
  source: string;
  target: string;
  sourceHandle?: string | null;
  targetHandle?: string | null;
  selected?: boolean;
  label?: string;
  labelShowBg?: boolean;
  data: FlowEdgeData;
  hidden?: boolean;
}

/** semanticToFlow 的投影结果。 */
export interface FlowProjection {
  nodes: FlowViewNode[];
  edges: FlowViewEdge[];
  /** 从 flat semantic graph 即时推导的 Loop region；不写入 Definition。 */
  loopRegions: LoopRegionView[];
}

/** Loop region 的整体状态；diagnostics 负责说明 invalid 的具体原因。 */
export type LoopRegionStatus = 'valid' | 'invalid';

/** Loop region 诊断码与 compiler stable code 对齐，UI 负责本地化展示。 */
export type LoopRegionDiagnosticCode =
  | 'LOOP_NOT_FOUND'
  | 'LOOP_ENTRY_INVALID'
  | 'LOOP_BODY_INVALID'
  | 'LOOP_YIELD_INVALID'
  | 'LOOP_DONE_INVALID'
  | 'LOOP_YIELD_UNREACHABLE'
  | 'LOOP_BODY_BYPASS'
  | 'LOOP_CROSS_REGION_EDGE'
  | 'LOOP_NESTING_UNSUPPORTED'
  | 'LOOP_REGION_OVERLAP';

/** Loop region 的纯诊断；不复制 compiler message 或 Definition 内容。 */
export interface LoopRegionDiagnostic {
  code: LoopRegionDiagnosticCode;
  edgeIds: string[];
}

/**
 * flat FlowGraph 的 Loop structured region 投影。
 *
 * `bodyEntry` / `yieldSource` 与 compiler 的 `LoopControlRegion` 对齐；
 * 所有 edge id 都由 semantic identity 派生，数组排序后可稳定消费。
 */
export interface LoopRegionView {
  loopNodeId: string;
  status: LoopRegionStatus;
  collapsed: boolean;
  bodyEntry: FlowPortRef | null;
  yieldSource: FlowPortRef | null;
  bodyNodes: string[];
  collectionEdgeIds: string[];
  bodyEdgeIds: string[];
  yieldEdgeIds: string[];
  doneEdgeIds: string[];
  crossRegionEdgeIds: string[];
  /** collection/body/yield/done 与异常跨边界边的去重并集。 */
  boundaryEdgeIds: string[];
  diagnostics: LoopRegionDiagnostic[];
}

export type FlowHandleDirection = 'source' | 'target';

export type EditorSelection =
  | { kind: 'none' }
  | { kind: 'node'; nodeId: string }
  | { kind: 'edge'; edgeId: string }
  | { kind: 'port'; nodeId: string; direction: FlowHandleDirection; handle: string }
  | { kind: 'loopRegion'; loopNodeId: string };

export function portSelectionToken(
  nodeId: string,
  direction: FlowHandleDirection,
  handle: string,
): string {
  return `port:${encodeURIComponent(nodeId)}:${direction}:${encodeURIComponent(handle)}`;
}

export function loopRegionSelectionToken(loopNodeId: string): string {
  return `loop-region:${encodeURIComponent(loopNodeId)}`;
}

function decodeSelectionPart(value: string): string | null {
  try {
    return decodeURIComponent(value);
  } catch {
    return null;
  }
}

export function parseEditorSelection(selection: string | null): EditorSelection {
  if (!selection) return { kind: 'none' };
  if (selection.startsWith('edge:')) return { kind: 'edge', edgeId: selection.slice(5) };
  if (selection.startsWith('loop-region:')) {
    const loopNodeId = decodeSelectionPart(selection.slice('loop-region:'.length));
    return loopNodeId === null ? { kind: 'none' } : { kind: 'loopRegion', loopNodeId };
  }
  if (selection.startsWith('port:')) {
    const parts = selection.split(':');
    if (parts.length === 4 && (parts[2] === 'source' || parts[2] === 'target')) {
      const nodeId = decodeSelectionPart(parts[1]);
      const handle = decodeSelectionPart(parts[3]);
      if (nodeId !== null && handle !== null) {
        return { kind: 'port', nodeId, direction: parts[2], handle };
      }
    }
    return { kind: 'none' };
  }
  return { kind: 'node', nodeId: selection };
}

// ---------------------------------------------------------------------------
// 常量与纯函数辅助
// ---------------------------------------------------------------------------

/** 七类节点的展示顺序（默认网格的列次序）。 */
export const KIND_ORDER: readonly FlowNodeKind[] = [
  'http',
  'js',
  'extract',
  'mapper',
  'merge',
  'condition',
  'loop',
] as const;

/** 默认网格的列间距（px）。 */
const GRID_COLUMN_GAP = 260;
/** 默认网格的行间距（px）。 */
const GRID_ROW_GAP = 140;
/** 默认网格的起始偏移（px）。 */
const GRID_ORIGIN = 24;

/** 由 semantic identity 确定性派生 edge id（与 core.edgeIdentity 完全一致）。 */
export function edgeId(edge: FlowEdge): string {
  return `${edge.from.node_id}:${edge.from.handle}->${edge.to.node_id}:${edge.to.handle}`;
}

const LOOP_COLLECTION_HANDLE = 'collection';
const LOOP_BODY_HANDLE = 'body';
const LOOP_YIELD_HANDLE = 'yield';
const LOOP_DONE_HANDLE = 'done';

type LoopDiagnosticBuckets = Map<LoopRegionDiagnosticCode, Set<string>>;

function sortedEdgeIds(edges: readonly FlowEdge[]): string[] {
  return [...new Set(edges.map(edgeId))].sort();
}

function addLoopDiagnostic(
  buckets: LoopDiagnosticBuckets,
  code: LoopRegionDiagnosticCode,
  edges: readonly FlowEdge[] = [],
): void {
  const ids = buckets.get(code) ?? new Set<string>();
  for (const edge of edges) ids.add(edgeId(edge));
  buckets.set(code, ids);
}

function loopDiagnostics(buckets: LoopDiagnosticBuckets): LoopRegionDiagnostic[] {
  return [...buckets.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([code, ids]) => ({ code, edgeIds: [...ids].sort() }));
}

function loopReachableNodes(
  start: string,
  loopNodeId: string,
  excludedEdgeId: string,
  edges: readonly FlowEdge[],
): Set<string> {
  const seen = new Set<string>();
  const queue = [start];
  while (queue.length > 0) {
    const current = queue.shift();
    if (current === undefined || current === loopNodeId || seen.has(current)) continue;
    seen.add(current);
    for (const edge of edges) {
      if (
        edgeId(edge) === excludedEdgeId ||
        edge.from.node_id !== current ||
        edge.to.node_id === loopNodeId
      ) {
        continue;
      }
      queue.push(edge.to.node_id);
    }
  }
  return seen;
}

function loopNodesReaching(
  target: string,
  loopNodeId: string,
  excludedEdgeId: string,
  edges: readonly FlowEdge[],
): Set<string> {
  const seen = new Set<string>();
  const queue = [target];
  while (queue.length > 0) {
    const current = queue.shift();
    if (current === undefined || current === loopNodeId || seen.has(current)) continue;
    seen.add(current);
    for (const edge of edges) {
      if (
        edgeId(edge) === excludedEdgeId ||
        edge.to.node_id !== current ||
        edge.from.node_id === loopNodeId
      ) {
        continue;
      }
      queue.push(edge.from.node_id);
    }
  }
  return seen;
}

function regionBoundaryEdgeIds(
  collectionEdges: readonly FlowEdge[],
  bodyEdges: readonly FlowEdge[],
  yieldEdges: readonly FlowEdge[],
  doneEdges: readonly FlowEdge[],
  crossRegionEdges: readonly FlowEdge[],
): string[] {
  return [
    ...new Set(
      [collectionEdges, bodyEdges, yieldEdges, doneEdges, crossRegionEdges].flatMap((edges) =>
        edges.map(edgeId),
      ),
    ),
  ].sort();
}

function emptyLoopRegion(loopNodeId: string, code: LoopRegionDiagnosticCode): LoopRegionView {
  return {
    loopNodeId,
    status: 'invalid',
    collapsed: false,
    bodyEntry: null,
    yieldSource: null,
    bodyNodes: [],
    collectionEdgeIds: [],
    bodyEdgeIds: [],
    yieldEdgeIds: [],
    doneEdgeIds: [],
    crossRegionEdgeIds: [],
    boundaryEdgeIds: [],
    diagnostics: [{ code, edgeIds: [] }],
  };
}

function withLoopDiagnostic(
  region: LoopRegionView,
  code: LoopRegionDiagnosticCode,
  edgeIds: readonly string[] = [],
): LoopRegionView {
  const diagnostics = region.diagnostics.some((diagnostic) => diagnostic.code === code)
    ? region.diagnostics
    : [...region.diagnostics, { code, edgeIds: [...edgeIds].sort() }].sort((left, right) =>
        left.code.localeCompare(right.code),
      );
  return {
    ...region,
    status: 'invalid',
    diagnostics,
  };
}

/**
 * 从单个 Loop 的 flat semantic graph 推导 structured region。
 *
 * bodyNodes 只保留“body entry 可达且能到达唯一 yield source”的节点；
 * yield backedge 不参与遍历，故不会制造第二份嵌套 body graph。
 */
export function projectLoopRegion(
  definition: RuleDefinition,
  loopNodeId: string,
  layout: LayoutState = null,
): LoopRegionView {
  const loopNode = definition.flow.nodes.find((node) => node.id === loopNodeId);
  if (!loopNode || loopNode.config.kind !== 'loop') {
    return emptyLoopRegion(loopNodeId, 'LOOP_NOT_FOUND');
  }

  const edges = definition.flow.edges;
  const collectionEdges = edges.filter(
    (edge) => edge.to.node_id === loopNodeId && edge.to.handle === LOOP_COLLECTION_HANDLE,
  );
  const bodyEdges = edges.filter(
    (edge) => edge.from.node_id === loopNodeId && edge.from.handle === LOOP_BODY_HANDLE,
  );
  const yieldEdges = edges.filter(
    (edge) => edge.to.node_id === loopNodeId && edge.to.handle === LOOP_YIELD_HANDLE,
  );
  const doneEdges = edges.filter(
    (edge) => edge.from.node_id === loopNodeId && edge.from.handle === LOOP_DONE_HANDLE,
  );

  const diagnostics: LoopDiagnosticBuckets = new Map();
  if (collectionEdges.length !== 1)
    addLoopDiagnostic(diagnostics, 'LOOP_ENTRY_INVALID', collectionEdges);
  if (bodyEdges.length !== 1 || bodyEdges[0]?.to.node_id === loopNodeId) {
    addLoopDiagnostic(diagnostics, 'LOOP_BODY_INVALID', bodyEdges);
  }
  if (yieldEdges.length !== 1 || yieldEdges[0]?.from.node_id === loopNodeId) {
    addLoopDiagnostic(diagnostics, 'LOOP_YIELD_INVALID', yieldEdges);
  }
  if (doneEdges.length === 0) addLoopDiagnostic(diagnostics, 'LOOP_DONE_INVALID');

  const bodyEntry =
    bodyEdges.length === 1 && bodyEdges[0].to.node_id !== loopNodeId
      ? { ...bodyEdges[0].to }
      : null;
  const yieldSource =
    yieldEdges.length === 1 && yieldEdges[0].from.node_id !== loopNodeId
      ? { ...yieldEdges[0].from }
      : null;
  const bodyNodeIds = new Set(definition.flow.nodes.map((node) => node.id));
  let bodyNodes: string[] = [];
  let crossRegionEdges: FlowEdge[] = [];

  if (bodyEntry && yieldSource && yieldEdges.length === 1 && bodyEdges.length === 1) {
    const yieldEdgeId = edgeId(yieldEdges[0]);
    const forward = loopReachableNodes(bodyEntry.node_id, loopNodeId, yieldEdgeId, edges);
    const reverse = loopNodesReaching(yieldSource.node_id, loopNodeId, yieldEdgeId, edges);

    if (!forward.has(yieldSource.node_id)) {
      addLoopDiagnostic(diagnostics, 'LOOP_YIELD_UNREACHABLE', yieldEdges);
    } else {
      bodyNodes = [...forward]
        .filter((nodeId) => reverse.has(nodeId) && bodyNodeIds.has(nodeId))
        .sort();
      if ([...forward].some((nodeId) => !reverse.has(nodeId))) {
        addLoopDiagnostic(diagnostics, 'LOOP_BODY_BYPASS');
      }

      const bodyNodeSet = new Set(bodyNodes);
      const bodyEdgeId = edgeId(bodyEdges[0]);
      crossRegionEdges = edges.filter((edge) => {
        const fromInside = bodyNodeSet.has(edge.from.node_id);
        const toInside = bodyNodeSet.has(edge.to.node_id);
        const validEntry = edgeId(edge) === bodyEdgeId;
        const validYield = edgeId(edge) === yieldEdgeId;
        return (
          (!fromInside && toInside && !validEntry) ||
          (fromInside && !toInside && !validYield) ||
          (collectionEdges.some((collectionEdge) => edgeId(collectionEdge) === edgeId(edge)) &&
            fromInside) ||
          (doneEdges.some((doneEdge) => edgeId(doneEdge) === edgeId(edge)) && toInside)
        );
      });
      if (crossRegionEdges.length > 0) {
        addLoopDiagnostic(diagnostics, 'LOOP_CROSS_REGION_EDGE', crossRegionEdges);
      }

      if (
        bodyNodes.some(
          (nodeId) =>
            definition.flow.nodes.find((node) => node.id === nodeId)?.config.kind === 'loop',
        )
      ) {
        addLoopDiagnostic(diagnostics, 'LOOP_NESTING_UNSUPPORTED');
      }
    }
  }

  const diagnosticList = loopDiagnostics(diagnostics);
  const boundaryEdgeIds = regionBoundaryEdgeIds(
    collectionEdges,
    bodyEdges,
    yieldEdges,
    doneEdges,
    crossRegionEdges,
  );
  return {
    loopNodeId,
    status: diagnosticList.length === 0 ? 'valid' : 'invalid',
    collapsed: layout?.loopRegions?.[loopNodeId]?.collapsed ?? false,
    bodyEntry,
    yieldSource,
    bodyNodes,
    collectionEdgeIds: sortedEdgeIds(collectionEdges),
    bodyEdgeIds: sortedEdgeIds(bodyEdges),
    yieldEdgeIds: sortedEdgeIds(yieldEdges),
    doneEdgeIds: sortedEdgeIds(doneEdges),
    crossRegionEdgeIds: sortedEdgeIds(crossRegionEdges),
    boundaryEdgeIds,
    diagnostics: diagnosticList,
  };
}

/** 投影所有 Loop，并把重叠 body 标记到涉及的两个 region。 */
export function projectLoopRegions(
  definition: RuleDefinition,
  layout: LayoutState = null,
): LoopRegionView[] {
  const loopNodeIds = definition.flow.nodes
    .filter((node) => node.config.kind === 'loop')
    .map((node) => node.id)
    .sort();
  const regions = loopNodeIds.map((loopNodeId) =>
    projectLoopRegion(definition, loopNodeId, layout),
  );

  for (let leftIndex = 0; leftIndex < regions.length; leftIndex += 1) {
    for (let rightIndex = leftIndex + 1; rightIndex < regions.length; rightIndex += 1) {
      const left = regions[leftIndex];
      const right = regions[rightIndex];
      const overlap = left.bodyNodes.filter((nodeId) => right.bodyNodes.includes(nodeId));
      if (overlap.length === 0) continue;
      regions[leftIndex] = withLoopDiagnostic(left, 'LOOP_REGION_OVERLAP');
      regions[rightIndex] = withLoopDiagnostic(right, 'LOOP_REGION_OVERLAP');
    }
  }
  return regions;
}

/** 七类 kind 的确定性默认网格位置（列=kind 次序，行=同类节点序号）。 */
export function defaultNodePosition(kind: FlowNodeKind, ordinal: number): Position {
  const column = KIND_ORDER.indexOf(kind);
  return {
    x: GRID_ORIGIN + Math.max(0, column) * GRID_COLUMN_GAP,
    y: GRID_ORIGIN + ordinal * GRID_ROW_GAP,
  };
}

/**
 * 从入口节点出发沿语义边做可达性遍历（BFS）。
 *
 * 返回可达节点 id 集合；`edges` 为 definition.flow.edges（from → to 方向）。
 * 供意图聚焦投影与画布 connection gate 共用，保证 gate 与投影一致。
 */
export function reachableNodes(entryId: string, edges: readonly FlowEdge[]): Set<string> {
  const reachable = new Set<string>([entryId]);
  const queue = [entryId];
  while (queue.length > 0) {
    const current = queue.shift();
    if (current === undefined) break;
    for (const edge of edges) {
      if (edge.from.node_id !== current) continue;
      const next = edge.to.node_id;
      if (!reachable.has(next)) {
        reachable.add(next);
        queue.push(next);
      }
    }
  }
  return reachable;
}

/** 读取配置中的字符串数组（Merge/Condition 使用）。 */
function stringArray(config: Record<string, unknown>, key: string): string[] {
  const value = config[key];
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : [];
}

/** 把 semantic handle 映射为 custom node 使用的可视 handle。 */
export function uiHandleForSemantic(
  kind: FlowNodeKind,
  config: Record<string, unknown>,
  handle: string,
  direction: FlowHandleDirection,
): string {
  if (direction === 'target') {
    if (kind === 'http' || kind === 'js' || kind === 'mapper') {
      return handle === 'input' ? 'in' : handle;
    }
    if (kind === 'extract') return handle === 'input' ? 'source' : handle;
    if (kind === 'loop') {
      if (handle === 'collection') return 'in';
      return handle;
    }
    if (kind === 'condition' && handle === 'input') return 'in';
    return handle;
  }

  if (kind === 'http') return handle === 'output' ? 'http_response' : handle;
  if (kind === 'js' && handle === 'output') {
    return String(config.output).toLowerCase() === 'raw' ? 'raw' : 'json';
  }
  if (kind === 'extract') return handle === 'output' ? 'json' : handle;
  if (kind === 'mapper' || kind === 'merge')
    return handle === 'output' ? (kind === 'mapper' ? 'delta' : 'json') : handle;
  if (kind === 'condition') {
    const index = stringArray(config, 'branches').indexOf(handle);
    return index >= 0 ? `branch:${index}` : handle;
  }
  return handle;
}

/** 把 custom node 的可视 handle还原为 compiler 使用的 semantic handle。 */
export function semanticHandleForUi(
  kind: FlowNodeKind,
  config: Record<string, unknown>,
  handle: string,
  direction: FlowHandleDirection,
): string {
  if (direction === 'target') {
    if (kind === 'http' || kind === 'js' || kind === 'mapper') {
      return handle === 'in' ? 'input' : handle;
    }
    if (kind === 'extract') return handle === 'source' ? 'input' : handle;
    if (kind === 'loop') {
      if (handle === 'in') return 'collection';
      return handle;
    }
    if (kind === 'condition' && handle === 'in') return 'input';
    return handle;
  }

  if (kind === 'http') return handle === 'http_response' ? 'output' : handle;
  if (kind === 'js' && (handle === 'json' || handle === 'raw')) return 'output';
  if (kind === 'extract') return handle === 'json' ? 'output' : handle;
  if (kind === 'mapper' && handle === 'delta') return 'output';
  if (kind === 'merge' && handle === 'json') return 'output';
  if (kind === 'condition' && handle.startsWith('branch:')) {
    const index = Number(handle.slice('branch:'.length));
    const branch = stringArray(config, 'branches')[index];
    return branch ?? handle;
  }
  return handle;
}

/** 读取布局中的节点位置/折叠；未保存的节点回退默认网格。 */
function layoutEntry(
  layout: LayoutState,
  nodeId: string,
  kind: FlowNodeKind,
  ordinal: number,
): { position: Position; collapsed: boolean } {
  const entry = layout?.nodes?.[nodeId];
  if (entry) {
    return { position: { ...entry.position }, collapsed: entry.collapsed };
  }
  return { position: defaultNodePosition(kind, ordinal), collapsed: false };
}

/** 计算 intentFocus 的 focused/dimmed 标记。 */
function focusFlags(
  definition: RuleDefinition,
  intentFocus: StandardIntent | null,
  nodeId: string,
): { focused: boolean; dimmed: boolean } {
  if (intentFocus === null) return { focused: false, dimmed: false };
  const flowEntry = definition.intent_exports[intentFocus]?.flow_entry;
  if (!flowEntry) return { focused: false, dimmed: true };
  const reachable = reachableNodes(flowEntry, definition.flow.edges);
  return { focused: reachable.has(nodeId), dimmed: !reachable.has(nodeId) };
}

function edgePresentation(
  edge: FlowEdge,
  sourceNode: { config: { kind: FlowNodeKind; value: Record<string, unknown> } } | undefined,
  targetNode: { config: { kind: FlowNodeKind; value: Record<string, unknown> } } | undefined,
): {
  role: PortRole;
  valueKind: string;
  label?: string;
  route: 'normal' | 'loop-back';
  lane: 'main' | 'branch' | 'auxiliary' | 'loop';
} {
  const sourceHandle = sourceNode
    ? uiHandleForSemantic(
        sourceNode.config.kind,
        sourceNode.config.value,
        edge.from.handle,
        'source',
      )
    : edge.from.handle;
  const targetHandle = targetNode
    ? uiHandleForSemantic(targetNode.config.kind, targetNode.config.value, edge.to.handle, 'target')
    : edge.to.handle;
  const sourcePort = sourceNode
    ? getNodePorts(sourceNode.config.kind, sourceNode.config.value).outputs.find(
        (port) => port.id === sourceHandle,
      )
    : undefined;
  const targetPort = targetNode
    ? getNodePorts(targetNode.config.kind, targetNode.config.value).inputs.find(
        (port) => port.id === targetHandle,
      )
    : undefined;
  const role =
    sourcePort?.role && sourcePort.role !== 'data'
      ? sourcePort.role
      : targetPort?.role && targetPort.role !== 'data'
        ? targetPort.role
        : 'data';
  const valueKind = sourcePort?.emits ?? targetPort?.accepts[0] ?? 'unknown';
  let label: string | undefined;
  if (sourceNode?.config.kind === 'condition' && sourcePort) label = `branch: ${sourcePort.label}`;
  else if (sourceNode?.config.kind === 'loop' && sourcePort) label = sourcePort.label;
  else if (targetNode?.config.kind === 'loop' && targetHandle === 'in') label = 'collection';
  else if (targetNode?.config.kind === 'loop' && edge.to.handle === LOOP_YIELD_HANDLE && targetPort)
    label = targetPort.label;
  const route =
    targetNode?.config.kind === 'loop' && targetHandle === 'yield' ? 'loop-back' : 'normal';
  const lane =
    route === 'loop-back'
      ? 'loop'
      : sourceNode?.config.kind === 'condition'
        ? 'branch'
        : role === 'control' || role === 'binding'
          ? 'auxiliary'
          : 'main';
  return { role, valueKind, label, route, lane };
}

// ---------------------------------------------------------------------------
// semanticToFlow：语义 → Svelte Flow 投影
// ---------------------------------------------------------------------------

/**
 * 把 canonical `RuleDefinition` 投影为 Svelte Flow nodes/edges。
 *
 * - 节点：`type = kind`（custom node type），`position` 来自 layout 或 kind 默认网格，
 *   `data` 携带语义身份/配置/诊断/聚焦标记，`selected` 反映 selection。
 * - 边：id 由 semantic identity 确定性派生；`source/target` 为语义 node id，
 *   `sourceHandle/targetHandle` 为语义 handle。
 * - intentFocus：只计算 focused/dimmed 标记，不复制节点/边/配置/undo history。
 *
 * 支持两种调用形态（合同 3 参 + 实现 4 参）：
 * - `semanticToFlow(definition, intentFocus, selection)`
 * - `semanticToFlow(definition, layout, intentFocus, selection)`
 */
export function semanticToFlow(
  definition: RuleDefinition,
  intentFocus: StandardIntent | null,
  selection: string | null,
): FlowProjection;
export function semanticToFlow(
  definition: RuleDefinition,
  layout: LayoutState,
  intentFocus: StandardIntent | null,
  selection: string | null,
): FlowProjection;
export function semanticToFlow(
  definition: RuleDefinition,
  layoutOrFocus: LayoutState | StandardIntent | null,
  focusOrSelection: StandardIntent | string | null,
  maybeSelection?: string | null,
): FlowProjection {
  const layout: LayoutState =
    maybeSelection === undefined ? null : (layoutOrFocus as LayoutState | null);
  const intentFocus: StandardIntent | null =
    maybeSelection === undefined
      ? (layoutOrFocus as StandardIntent | null)
      : (focusOrSelection as StandardIntent | null);
  const selection: string | null =
    maybeSelection === undefined ? (focusOrSelection as string | null) : (maybeSelection ?? null);
  const selectionView = parseEditorSelection(selection);
  const selectedEdge = selectionView.kind === 'edge' ? selectionView.edgeId : null;
  const loopRegions = projectLoopRegions(definition, layout);
  const collapsedBodyNodes = new Set(
    loopRegions.filter((region) => region.collapsed).flatMap((region) => region.bodyNodes),
  );

  // 同类节点计数（决定未保存节点的默认网格行）。
  const ordinalByKind = new Map<FlowNodeKind, number>();

  const nodes: FlowViewNode[] = definition.flow.nodes.map((node) => {
    const ordinal = ordinalByKind.get(node.config.kind) ?? 0;
    ordinalByKind.set(node.config.kind, ordinal + 1);
    const { position, collapsed } = layoutEntry(layout, node.id, node.config.kind, ordinal);
    const { focused, dimmed } = focusFlags(definition, intentFocus, node.id);
    const selectedPort =
      selectionView.kind === 'port' && selectionView.nodeId === node.id
        ? {
            direction: selectionView.direction,
            id: uiHandleForSemantic(
              node.config.kind,
              node.config.value,
              selectionView.handle,
              selectionView.direction,
            ),
          }
        : undefined;
    return {
      id: node.id,
      type: node.config.kind,
      position,
      selected: selectionView.kind === 'node' && node.id === selectionView.nodeId,
      hidden: collapsedBodyNodes.has(node.id),
      data: {
        nodeId: node.id,
        kind: node.config.kind,
        config: node.config.value,
        diagnostics: [],
        focused,
        dimmed,
        entryIntents: Object.entries(definition.intent_exports ?? {})
          .filter(([, entry]) => entry?.flow_entry === node.id)
          .map(([intent]) => intent as StandardIntent),
        collapsed,
        selectedPort,
      },
    };
  });

  const edges: FlowViewEdge[] = definition.flow.edges.map((edge) => {
    const sourceNode = definition.flow.nodes.find((node) => node.id === edge.from.node_id);
    const targetNode = definition.flow.nodes.find((node) => node.id === edge.to.node_id);
    const sourceDimmed =
      intentFocus !== null &&
      !reachableNodes(
        definition.intent_exports[intentFocus]?.flow_entry ?? '',
        definition.flow.edges,
      ).has(edge.from.node_id);
    const targetDimmed =
      intentFocus !== null &&
      !reachableNodes(
        definition.intent_exports[intentFocus]?.flow_entry ?? '',
        definition.flow.edges,
      ).has(edge.to.node_id);
    const presentation = edgePresentation(edge, sourceNode, targetNode);
    const selected = selectedEdge === edgeId(edge);
    return {
      id: edgeId(edge),
      source: edge.from.node_id,
      target: edge.to.node_id,
      sourceHandle: sourceNode
        ? uiHandleForSemantic(
            sourceNode.config.kind,
            sourceNode.config.value,
            edge.from.handle,
            'source',
          )
        : edge.from.handle,
      targetHandle: targetNode
        ? uiHandleForSemantic(
            targetNode.config.kind,
            targetNode.config.value,
            edge.to.handle,
            'target',
          )
        : edge.to.handle,
      selected,
      hidden: collapsedBodyNodes.has(edge.from.node_id) && collapsedBodyNodes.has(edge.to.node_id),
      label: presentation.label,
      labelShowBg: Boolean(presentation.label),
      data: {
        edge,
        dimmed: sourceDimmed || targetDimmed,
        selected,
        role: presentation.role,
        lane: presentation.lane,
        valueKind: presentation.valueKind,
        label: presentation.label,
        route: presentation.route,
      },
    };
  });

  return { nodes, edges, loopRegions };
}

// ---------------------------------------------------------------------------
// flowToSemantic：画布 UI 变更 → core typed action
// ---------------------------------------------------------------------------

/** 画布 UI 变更（唯一写入口；语义身份由画布从 data 重建）。 */
export type FlowUIChange =
  | { kind: 'connect'; edge: FlowEdge }
  | { kind: 'disconnect'; edge: FlowEdge }
  | { kind: 'reconnect'; from: FlowEdge; to: FlowEdge }
  | { kind: 'move'; nodeId: string; position: Position }
  | { kind: 'collapse'; nodeId: string; collapsed: boolean }
  | { kind: 'select'; nodeId: string | null };

/**
 * 把一次画布 UI 变更映射为 core.ts 的 typed action（或动作序列）。
 *
 * - connect → `edgeConnect(edge, connected: true)`
 * - disconnect（删除边）→ `edgeConnect(edge, connected: false)`
 * - reconnect → 先断开旧边再连接新边（两个 semantic action）
 * - move → `moveNode`（layout domain；core 会对连续拖动 coalesce）
 * - collapse → `collapseNode`（layout domain）
 * - select → `selection`（transient，不进 history）
 */
export function flowToSemantic(change: FlowUIChange): AuthoringAction | AuthoringAction[] {
  switch (change.kind) {
    case 'connect':
      return { kind: 'edgeConnect', edge: change.edge, connected: true };
    case 'disconnect':
      return { kind: 'edgeConnect', edge: change.edge, connected: false };
    case 'reconnect':
      return [
        { kind: 'edgeConnect', edge: change.from, connected: false },
        { kind: 'edgeConnect', edge: change.to, connected: true },
      ];
    case 'move':
      return { kind: 'moveNode', nodeId: change.nodeId, position: change.position };
    case 'collapse':
      return { kind: 'collapseNode', nodeId: change.nodeId, collapsed: change.collapsed };
    case 'select':
      return { kind: 'selection', nodeId: change.nodeId };
  }
}
