//! 连接 gate：连接前校验（端口类型 / 意图兼容 / 回边拒绝）+ palette 推荐与可达性。
//!
//! 纯 TS，无 runes。组件层在 dispatch typed action 前调用 `validateConnection`；
//! 校验失败不产生任何 semantic 变更。`reachableNodes` 亦被 adapter 投影
//! （focused/dimmed）复用，保证 gate 与投影对“意图子图”的判定一致。

import type { FlowEdge, FlowNodeKind, RuleDefinition, StandardIntent } from '@/shared/tauri/rules';
import { semanticHandleForUi } from './flow-adapter';
import { findInputPort, findOutputPort, getNodePorts, portCompatible } from './ports';
import { NODE_KINDS } from './meta';

/** Svelte Flow Connection 的轻量镜像（结构化兼容，避免纯逻辑依赖 UI 库）。 */
export type FlowConnection = {
  source: string;
  target: string;
  sourceHandle: string | null;
  targetHandle: string | null;
};

/** Gate 图节点（只取校验所需字段）。 */
export type GateNode = {
  id: string;
  kind: FlowNodeKind;
  config?: Record<string, unknown>;
};

/** Gate 输入图。 */
export type GateGraph = {
  nodes: GateNode[];
  edges: FlowEdge[];
  /** 当前焦点意图的 flow_entry node id；null/缺省 = 全部视图（不校验意图）。 */
  focusEntry?: string | null;
  /** 重连时排除正在替换的旧边 identity。 */
  excludeEdgeId?: string;
};

/** 连接被拒原因（闭集）。 */
export type ConnectionRejectReason =
  /** 缺少 handle 引用。 */
  | 'missing-handle'
  /** 端点节点不在图中。 */
  | 'unknown-node'
  /** 端点 handle 不属于该节点类型的端口合同。 */
  | 'unknown-handle'
  /** 源目标为同一节点。 */
  | 'self-loop'
  /** 端口类型不兼容（见 PORT_CONTRACT 矩阵）。 */
  | 'incompatible-ports'
  /** 同一条语义边已存在。 */
  | 'duplicate-edge'
  /** HTTP 入口输入已占用（单入边）。 */
  | 'entry-occupied'
  /** 将形成回路。 */
  | 'back-edge'
  /** Loop yield 只能有一条 structured return。 */
  | 'loop-yield-occupied'
  /** 当前意图焦点下，源节点不在焦点子图内。 */
  | 'intent-mismatch';

export type ConnectionVerdict = { ok: true } | { ok: false; reason: ConnectionRejectReason };

/** 语义边 identity（与 core 的 edgeIdentity 一致）。 */
function edgeIdentity(edge: FlowEdge): string {
  return `${edge.from.node_id}:${edge.from.handle}->${edge.to.node_id}:${edge.to.handle}`;
}

/** 从 Svelte Flow Connection 构造语义边（handle 已校验非空）。 */
export function semanticEdgeFromConnection(
  connection: FlowConnection,
  sourceNode?: GateNode,
  targetNode?: GateNode,
): FlowEdge {
  return {
    from: {
      node_id: connection.source,
      handle:
        sourceNode?.config === undefined
          ? (connection.sourceHandle ?? '')
          : semanticHandleForUi(
              sourceNode.kind,
              sourceNode.config,
              connection.sourceHandle ?? '',
              'source',
            ),
    },
    to: {
      node_id: connection.target,
      handle:
        targetNode?.config === undefined
          ? (connection.targetHandle ?? '')
          : semanticHandleForUi(
              targetNode.kind,
              targetNode.config,
              connection.targetHandle ?? '',
              'target',
            ),
    },
  };
}

/** 从入口节点出发，沿有向边 BFS 可达的全部节点 id（含入口自身）。 */
export function reachableNodes(entryId: string, edges: FlowEdge[]): Set<string> {
  const visited = new Set<string>([entryId]);
  const queue = [entryId];
  while (queue.length > 0) {
    const current = queue.shift() as string;
    for (const edge of edges) {
      if (edge.from.node_id === current && !visited.has(edge.to.node_id)) {
        visited.add(edge.to.node_id);
        queue.push(edge.to.node_id);
      }
    }
  }
  return visited;
}

/** 从定义读取每个意图的 flow_entry 映射（无导出的意图缺席）。 */
export function flowEntryByIntentFromDefinition(
  definition: RuleDefinition,
): Partial<Record<StandardIntent, string>> {
  const result: Partial<Record<StandardIntent, string>> = {};
  for (const [intent, entry] of Object.entries(definition.intent_exports ?? {})) {
    const flowEntry = entry?.flow_entry;
    if (typeof flowEntry === 'string' && flowEntry.length > 0) {
      result[intent as StandardIntent] = flowEntry;
    }
  }
  return result;
}

/** 校验连接是否可建立。
 *
 * 校验顺序：节点存在 → handle 存在 → 非自环 → 端口类型 → 重复边 →
 * 入口单入边 → 回边 → 意图焦点。任一失败即返回原因，不产生语义变更。
 */
export function validateConnection(
  connection: FlowConnection,
  graph: GateGraph,
): ConnectionVerdict {
  const { source, target, sourceHandle, targetHandle } = connection;
  if (!sourceHandle || !targetHandle) return { ok: false, reason: 'missing-handle' };

  const sourceNode = graph.nodes.find((node) => node.id === source);
  const targetNode = graph.nodes.find((node) => node.id === target);
  if (!sourceNode || !targetNode) return { ok: false, reason: 'unknown-node' };

  if (source === target) return { ok: false, reason: 'self-loop' };

  const outPort = findOutputPort(sourceNode.kind, sourceHandle, sourceNode.config);
  if (!outPort) return { ok: false, reason: 'unknown-handle' };
  const inPort = findInputPort(targetNode.kind, targetHandle, targetNode.config);
  if (!inPort) return { ok: false, reason: 'unknown-handle' };

  if (!portCompatible(outPort, inPort)) return { ok: false, reason: 'incompatible-ports' };

  const nextEdge = semanticEdgeFromConnection(connection, sourceNode, targetNode);
  const activeEdges = graph.excludeEdgeId
    ? graph.edges.filter((edge) => edgeIdentity(edge) !== graph.excludeEdgeId)
    : graph.edges;
  const nextIdentity = edgeIdentity(nextEdge);
  if (activeEdges.some((edge) => edgeIdentity(edge) === nextIdentity)) {
    return { ok: false, reason: 'duplicate-edge' };
  }

  if (
    targetNode.kind === 'loop' &&
    targetHandle === 'yield' &&
    activeEdges.some(
      (edge) =>
        edge.to.node_id === target &&
        edge.to.handle ===
          semanticHandleForUi(targetNode.kind, targetNode.config ?? {}, targetHandle, 'target'),
    )
  ) {
    return { ok: false, reason: 'loop-yield-occupied' };
  }

  // HTTP 入口输入单入边（意图入口语义）。
  if (
    targetNode.kind === 'http' &&
    targetHandle === 'in' &&
    activeEdges.some(
      (edge) =>
        edge.to.node_id === target &&
        edge.to.handle ===
          (targetNode.config === undefined
            ? targetHandle
            : semanticHandleForUi(targetNode.kind, targetNode.config, targetHandle, 'target')),
    )
  ) {
    return { ok: false, reason: 'entry-occupied' };
  }

  // 只有 Loop 的唯一 yield edge 可以形成 structured region 回边。
  const structuredYield = isStructuredYieldBackedge(connection, source, targetNode, activeEdges);
  if (reachableNodes(target, [...activeEdges, nextEdge]).has(source) && !structuredYield) {
    return { ok: false, reason: 'back-edge' };
  }

  // 意图兼容：焦点激活时，源节点必须在焦点子图内（构建方向为“焦点内 → 外扩”）。
  if (graph.focusEntry) {
    const focused = reachableNodes(graph.focusEntry, activeEdges);
    if (!focused.has(source)) return { ok: false, reason: 'intent-mismatch' };
  }

  return { ok: true };
}

/** 便捷布尔版（isValidConnection / onbeforeconnect 用）。 */
export function canConnect(connection: FlowConnection, graph: GateGraph): boolean {
  return validateConnection(connection, graph).ok;
}

/**
 * 单条不兼容原因。
 *
 * 只出稳定码与定位数据，展示文案由视图按 locale 组装 —— 与
 * `validateConnection` 的 reason 一致，模型层不持有任何面向用户的字符串。
 */
export type RecommendBlocker =
  | { code: 'incompatible-ports'; sourceKind: FlowNodeKind; targetKind: FlowNodeKind }
  | { code: 'outside-intent-focus' };

/** Palette 单类节点推荐结果。 */
export type KindRecommendation = {
  kind: FlowNodeKind;
  compatible: boolean;
  /** 不兼容原因；compatible 为 true 时为空数组。 */
  blockers: RecommendBlocker[];
};

/** 推荐上下文。 */
export type RecommendContext = {
  nodes: GateNode[];
  edges: FlowEdge[];
  /** 当前选中节点（作为连接的源）。 */
  selection?: string | null;
  /** 当前焦点意图的 flow_entry（可空）。 */
  focusEntry?: string | null;
};

/**
 * 计算 palette 推荐：在“全部视图”下所有节点均可添加；
 * 选中节点存在时，按所选节点输出与目标类型输入的类型矩阵判定兼容性，
 * 并叠加意图焦点（所选节点须在焦点子图内）。不兼容项给出中文原因。
 */
export function recommendKinds(context: RecommendContext): KindRecommendation[] {
  const { nodes, edges, selection, focusEntry } = context;
  const selected = selection ? nodes.find((node) => node.id === selection) : undefined;
  const focused = focusEntry ? reachableNodes(focusEntry, edges) : null;

  return NODE_KINDS.map((kind) => {
    const blockers: RecommendBlocker[] = [];

    if (selected) {
      if (!anyOutputFeeds(selected, kind)) {
        blockers.push({
          code: 'incompatible-ports',
          sourceKind: selected.kind,
          targetKind: kind,
        });
      }
      if (focused && !focused.has(selected.id)) {
        blockers.push({ code: 'outside-intent-focus' });
      }
    }

    return { kind, compatible: blockers.length === 0, blockers };
  });
}

/** 所选节点的任一输出端口能否接入目标类型的任一输入端口。 */
function anyOutputFeeds(selected: GateNode, targetKind: FlowNodeKind): boolean {
  const { outputs } = getNodePorts(selected.kind, selected.config);
  const { inputs } = getNodePorts(targetKind);
  return outputs.some((out) => inputs.some((input) => portCompatible(out, input)));
}

function isStructuredYieldBackedge(
  connection: FlowConnection,
  sourceId: string,
  targetNode: GateNode,
  edges: FlowEdge[],
): boolean {
  if (targetNode.kind !== 'loop' || connection.targetHandle !== 'yield') return false;

  const bodyEdge = edges.find(
    (edge) => edge.from.node_id === targetNode.id && edge.from.handle === 'body',
  );
  if (!bodyEdge || bodyEdge.to.node_id === targetNode.id || sourceId === targetNode.id) {
    return false;
  }

  const reachable = new Set<string>();
  const queue = [bodyEdge.to.node_id];
  while (queue.length > 0) {
    const current = queue.shift();
    if (!current || current === targetNode.id || reachable.has(current)) continue;
    reachable.add(current);
    for (const edge of edges) {
      if (edge.from.node_id === current && edge.to.node_id !== targetNode.id) {
        queue.push(edge.to.node_id);
      }
    }
  }
  return reachable.has(sourceId);
}
