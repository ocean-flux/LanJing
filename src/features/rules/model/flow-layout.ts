import type { ElkNode, LayoutOptions } from 'elkjs/lib/elk-api';
import type { Position } from './core';
import type { FlowNode, RuleDefinition } from '@/shared/tauri/rules';

export const FLOW_NODE_WIDTH = 252;
export const FLOW_LAYOUT_OPTIONS: LayoutOptions = {
  'elk.algorithm': 'layered',
  'elk.direction': 'RIGHT',
  'elk.edgeRouting': 'ORTHOGONAL',
  'elk.padding': '[top=48,left=48,bottom=48,right=48]',
  'elk.spacing.nodeNode': '72',
  'elk.layered.spacing.nodeNodeBetweenLayers': '168',
  'elk.layered.spacing.edgeNodeBetweenLayers': '72',
  'elk.layered.spacing.edgeEdgeBetweenLayers': '28',
  'elk.layered.feedbackEdges': 'true',
  'elk.layered.crossingMinimization.strategy': 'LAYER_SWEEP',
  'elk.layered.nodePlacement.strategy': 'NETWORK_SIMPLEX',
  'elk.layered.nodePlacement.favorStraightEdges': 'true',
};

const FLOW_ORIGIN = 48;
const FLOW_ROW_GAP = 72;
const FLOW_LAYER_GAP = 168;
const NODE_HEADER_HEIGHT = 48;
const NODE_SUMMARY_HEIGHT = 36;
const NODE_LOOP_EXTRA_HEIGHT = 72;
const NODE_ENTRY_HEIGHT = 25;
const NODE_MIN_HEIGHT = 142;

type LayoutPositionMap = Record<string, Position>;

export type FlowNodeDimensions = Record<
  string,
  {
    width?: number;
    height?: number;
  }
>;

function nodeHeight(
  node: FlowNode,
  definition: RuleDefinition,
  dimensions: FlowNodeDimensions = {},
): number {
  const measuredHeight = dimensions[node.id]?.height;
  if (typeof measuredHeight === 'number' && Number.isFinite(measuredHeight) && measuredHeight > 0) {
    return measuredHeight;
  }
  const entryHeight = Object.values(definition.intent_exports ?? {}).some(
    (entry) => entry?.flow_entry === node.id,
  )
    ? NODE_ENTRY_HEIGHT
    : 0;
  const loopHeight = node.config.kind === 'loop' ? NODE_LOOP_EXTRA_HEIGHT : 0;
  return Math.max(
    NODE_MIN_HEIGHT,
    entryHeight + NODE_HEADER_HEIGHT + NODE_SUMMARY_HEIGHT + loopHeight,
  );
}

function nodeWidth(node: FlowNode, dimensions: FlowNodeDimensions): number {
  const measuredWidth = dimensions[node.id]?.width;
  return typeof measuredWidth === 'number' && Number.isFinite(measuredWidth) && measuredWidth > 0
    ? measuredWidth
    : FLOW_NODE_WIDTH;
}

function createElkNode(
  node: FlowNode,
  definition: RuleDefinition,
  dimensions: FlowNodeDimensions,
): ElkNode {
  return {
    id: node.id,
    width: nodeWidth(node, dimensions),
    height: nodeHeight(node, definition, dimensions),
  };
}

function createElkGraph(definition: RuleDefinition, dimensions: FlowNodeDimensions): ElkNode {
  const { nodes } = definition.flow;
  const nodesById = new Map(nodes.map((node) => [node.id, node]));
  return {
    id: 'native-rule-flow',
    children: nodes.map((node) => createElkNode(node, definition, dimensions)),
    edges: definition.flow.edges
      .filter((edge) => !isLoopBackEdge(definition, edge))
      .flatMap((edge, index) => {
        const sourceNode = nodesById.get(edge.from.node_id);
        const targetNode = nodesById.get(edge.to.node_id);
        if (!sourceNode || !targetNode) return [];
        return [{ id: `edge:${index}`, sources: [sourceNode.id], targets: [targetNode.id] }];
      }),
    layoutOptions: FLOW_LAYOUT_OPTIONS,
  };
}

function normalizeElkPositions(result: ElkNode): LayoutPositionMap {
  const children = (result.children ?? []).filter(
    (child): child is ElkNode =>
      typeof child.id === 'string' && Number.isFinite(child.x) && Number.isFinite(child.y),
  );
  if (children.length === 0) return {};
  const minX = Math.min(...children.map((child) => child.x ?? 0));
  const minY = Math.min(...children.map((child) => child.y ?? 0));
  return Object.fromEntries(
    children.map((child) => [
      child.id,
      {
        x: Math.round((child.x ?? 0) - minX + FLOW_ORIGIN),
        y: Math.round((child.y ?? 0) - minY + FLOW_ORIGIN),
      },
    ]),
  );
}

function isLoopBackEdge(
  definition: RuleDefinition,
  edge: RuleDefinition['flow']['edges'][number],
): boolean {
  return (
    edge.to.handle === 'yield' &&
    definition.flow.nodes.some((node) => node.id === edge.to.node_id && node.config.kind === 'loop')
  );
}

function fallbackFlowLayout(
  definition: RuleDefinition,
  dimensions: FlowNodeDimensions = {},
): LayoutPositionMap {
  const { nodes } = definition.flow;
  const nodeIds = new Set(nodes.map((node) => node.id));
  const incoming = new Map(nodes.map((node) => [node.id, 0]));
  const outgoing = new Map<string, string[]>();
  for (const edge of definition.flow.edges) {
    if (
      !nodeIds.has(edge.from.node_id) ||
      !nodeIds.has(edge.to.node_id) ||
      isLoopBackEdge(definition, edge)
    ) {
      continue;
    }
    incoming.set(edge.to.node_id, (incoming.get(edge.to.node_id) ?? 0) + 1);
    outgoing.set(edge.from.node_id, [...(outgoing.get(edge.from.node_id) ?? []), edge.to.node_id]);
  }

  const rank = new Map<string, number>();
  const queue = nodes.filter((node) => (incoming.get(node.id) ?? 0) === 0).map((node) => node.id);
  if (queue.length === 0 && nodes[0]) queue.push(nodes[0].id);
  for (const nodeId of queue) rank.set(nodeId, 0);
  let cursor = 0;
  while (cursor < queue.length) {
    const nodeId = queue[cursor++];
    const nextRank = (rank.get(nodeId) ?? 0) + 1;
    for (const targetId of outgoing.get(nodeId) ?? []) {
      rank.set(targetId, Math.max(rank.get(targetId) ?? 0, nextRank));
      incoming.set(targetId, (incoming.get(targetId) ?? 1) - 1);
      if (incoming.get(targetId) === 0) queue.push(targetId);
    }
  }

  let orphanRank = Math.max(...rank.values(), 0) + 1;
  for (const node of nodes) {
    if (!rank.has(node.id)) rank.set(node.id, orphanRank++);
  }

  const lanes = new Map<number, FlowNode[]>();
  for (const node of nodes) {
    const lane = rank.get(node.id) ?? 0;
    lanes.set(lane, [...(lanes.get(lane) ?? []), node]);
  }
  const laneHeights = new Map(
    [...lanes].map(([lane, laneNodes]) => [
      lane,
      laneNodes.reduce((height, node) => height + nodeHeight(node, definition, dimensions), 0) +
        Math.max(0, laneNodes.length - 1) * FLOW_ROW_GAP,
    ]),
  );
  const canvasHeight = Math.max(...laneHeights.values(), NODE_MIN_HEIGHT);
  const canvasCenter = FLOW_ORIGIN + canvasHeight / 2;
  const positions: LayoutPositionMap = {};
  for (const [lane, laneNodes] of lanes) {
    const laneHeight = laneHeights.get(lane) ?? NODE_MIN_HEIGHT;
    let y = canvasCenter - laneHeight / 2;
    for (const node of laneNodes) {
      positions[node.id] = { x: FLOW_ORIGIN + lane * (FLOW_NODE_WIDTH + FLOW_LAYER_GAP), y };
      y += nodeHeight(node, definition, dimensions) + FLOW_ROW_GAP;
    }
  }
  return positions;
}

export async function layoutNativeRuleFlow(
  definition: RuleDefinition,
  dimensions: FlowNodeDimensions = {},
): Promise<LayoutPositionMap> {
  if (definition.flow.nodes.length === 0) return {};
  try {
    const { default: Elk } = await import('elkjs/lib/elk.bundled.js');
    const elk = new Elk();
    const result = await elk.layout(createElkGraph(definition, dimensions));
    const positions = normalizeElkPositions(result);
    if (Object.keys(positions).length === definition.flow.nodes.length) return positions;
  } catch {
    return fallbackFlowLayout(definition, dimensions);
  }
  return fallbackFlowLayout(definition, dimensions);
}

export { fallbackFlowLayout, nodeHeight };
