//! 规则流程图画布：xyflow 受控容器。
//!
//! nodes/edges 全部来自 session 的 adapter 投影，画布不持有 core；
//! 连接、重连、删除都先过 connection gate，失败就不派发 —— 绝不写入无效语义。

import {
  Background,
  Controls,
  MiniMap,
  ReactFlow,
  ViewportPortal,
  applyNodeChanges,
  useReactFlow,
  type Connection,
  type EdgeTypes,
  type NodeTypes,
  type OnSelectionChangeParams,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { Position } from './model/core';
import { cn } from '@/shared/utils';
import { useMessages } from '@/shared/i18n/messages';
import { useTheme } from '@/shared/theme/use-theme';
import {
  flowEntryByIntentFromDefinition,
  semanticEdgeFromConnection,
  validateConnection,
  type ConnectionRejectReason,
  type GateGraph,
} from './model/connection-gate';
import {
  loopRegionSelectionToken,
  parseEditorSelection,
  type FlowHandleDirection,
  type FlowViewEdge,
  type FlowViewNode,
  type LoopRegionView,
} from './model/flow-adapter';
import {
  selectFlowProjection,
  subgraphFromProjection,
  type SubgraphClipboard,
} from './model/session';
import { useRuleEditorSession, useRuleEditorSessionStore } from './use-session';
import { RuleFlowNode } from './nodes/RuleFlowNode';
import { SemanticEdge } from './edges/SemanticEdge';
import { LoopRegionOverlay, type LoopRegionBounds } from './LoopRegionOverlay';
import { FlowTopBar } from './FlowTopBar';
import { NodePalette, usePaletteDrop } from './NodePalette';
import './rules-flow.css';

/** 节点数超过该阈值才渲染缩略图，小图不占画布空间。 */
const MINIMAP_NODE_THRESHOLD = 12;

/** 布局过渡时长；与 rules-flow.css 的 transition 保持一致。 */
const LAYOUT_ANIMATION_MS = 200;

/** 未实测到真实尺寸时的兜底，与 rules-flow.css 的卡片尺寸一致。 */
const NODE_FALLBACK = { width: 252, height: 142 };
const REGION_PADDING_X = 34;
const REGION_PADDING_Y = 38;

/** 拒绝提示的停留时长；不设自动消失会一直遮住画布顶部。 */
const REJECT_HINT_MS = 2600;

const nodeTypes: NodeTypes = {
  http: RuleFlowNode,
  js: RuleFlowNode,
  extract: RuleFlowNode,
  mapper: RuleFlowNode,
  merge: RuleFlowNode,
  condition: RuleFlowNode,
  loop: RuleFlowNode,
  // 未安装能力（kind 无 descriptor 声明）没有专属组件。`default` 兜底成同一个
  // `RuleFlowNode`：端口按 descriptor 解析，查不到即空端口 + unavailable 摘要，
  // 未安装能力节点因此仍可展示与选中，而不是退化成 xyflow 的空白节点。
  default: RuleFlowNode,
};

const edgeTypes: EdgeTypes = { semantic: SemanticEdge };

const NO_MARQUEE: MarqueeSelection = { nodes: [], edges: [] };

/** 框选集合；只存 id，节点数据始终来自投影。 */
type MarqueeSelection = { nodes: string[]; edges: string[] };

function sameIds(left: readonly string[], right: readonly string[]): boolean {
  return left.length === right.length && left.every((id, index) => id === right[index]);
}

function rejectText(m: ReturnType<typeof useMessages>, reason: ConnectionRejectReason): string {
  switch (reason) {
    case 'missing-handle': {
      return m.rules_connect_reject_missing_handle();
    }
    case 'unknown-node': {
      return m.rules_connect_reject_unknown_node();
    }
    case 'unknown-handle': {
      return m.rules_connect_reject_unknown_handle();
    }
    case 'self-loop': {
      return m.rules_connect_reject_self_loop();
    }
    case 'incompatible-ports': {
      return m.rules_connect_reject_incompatible_ports();
    }
    case 'duplicate-edge': {
      return m.rules_connect_reject_duplicate_edge();
    }
    case 'entry-occupied': {
      return m.rules_connect_reject_entry_occupied();
    }
    case 'back-edge': {
      return m.rules_connect_reject_back_edge();
    }
    case 'loop-yield-occupied': {
      return m.rules_connect_reject_loop_yield_occupied();
    }
    case 'intent-mismatch': {
      return m.rules_connect_reject_intent_mismatch();
    }
  }
}

/** 从已渲染的 DOM 量节点实际尺寸；循环域边界要按真实高度算才不会切边。 */
function useNodeSizes(rootRef: React.RefObject<HTMLDivElement | null>) {
  const [epoch, setEpoch] = useState(0);

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;

    let disposed = false;
    let frame: number | null = null;
    const bump = () => {
      if (disposed || frame !== null) return;
      // ResizeObserver 回调内同步 setState 会触发布局回环告警；下一帧统一量尺寸。
      frame = requestAnimationFrame(() => {
        frame = null;
        if (!disposed) setEpoch((value) => value + 1);
      });
    };

    const resizeObserver = new ResizeObserver(bump);
    const observeNodes = () => {
      resizeObserver.disconnect();
      for (const node of root.querySelectorAll<HTMLElement>('.react-flow__node')) {
        resizeObserver.observe(node);
      }
      bump();
    };
    const mutationObserver = new MutationObserver(observeNodes);
    mutationObserver.observe(root, { childList: true, subtree: true });
    observeNodes();

    return () => {
      disposed = true;
      if (frame !== null) cancelAnimationFrame(frame);
      resizeObserver.disconnect();
      mutationObserver.disconnect();
    };
  }, [rootRef]);

  return useCallback(
    (nodeId: string) => {
      const root = rootRef.current;
      if (!root || epoch === 0) return NODE_FALLBACK;
      const element = [...root.querySelectorAll<HTMLElement>('.react-flow__node')].find(
        (candidate) => candidate.dataset.id === nodeId,
      );
      return {
        width: element?.offsetWidth || NODE_FALLBACK.width,
        height: element?.offsetHeight || NODE_FALLBACK.height,
      };
    },
    [rootRef, epoch],
  );
}

export function RuleFlowCanvas({ className }: { className?: string }) {
  const m = useMessages();
  const { resolvedTheme } = useTheme();
  const store = useRuleEditorSessionStore();
  const rootRef = useRef<HTMLDivElement>(null);
  const nodeSize = useNodeSizes(rootRef);
  const [rejectReason, setRejectReason] = useState<ConnectionRejectReason | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  // 框选是画布本地状态：core.selection 只表达检查器的单点输入，
  // 把多选写回 core 会让投影重算，反过来把框选清掉。
  const [marquee, setMarquee] = useState<MarqueeSelection>(NO_MARQUEE);
  const [layoutAnimating, setLayoutAnimating] = useState(false);
  const [flowNodes, setFlowNodes] = useState<FlowViewNode[]>([]);
  const dragPositionsRef = useRef<Record<string, Position>>({});
  const clipboardRef = useRef<SubgraphClipboard | null>(null);
  const { fitView } = useReactFlow();
  const { onDragOver, onDrop } = usePaletteDrop();

  const projection = useRuleEditorSession(selectFlowProjection);
  const definition = useRuleEditorSession((state) => state.core.definition);
  const selection = useRuleEditorSession((state) => state.core.selection);
  const revealRequest = useRuleEditorSession((state) => state.revealRequest);
  const intentFocus = useRuleEditorSession((state) => state.core.intentFocus);

  /** 端口选择直接使用 descriptor 的 handle（NodeShell 渲染的就是它）。 */
  const selectPort = useCallback(
    (nodeId: string, direction: FlowHandleDirection, uiHandle: string) => {
      const node = selectFlowProjection(store.getState()).nodes.find(
        (candidate) => candidate.id === nodeId,
      );
      if (!node) return;
      store.getState().selectPort(nodeId, direction, uiHandle);
    },
    [store],
  );

  const multiSelected = marquee.nodes.length + marquee.edges.length > 1;

  const nodes = useMemo<FlowViewNode[]>(() => {
    const picked = new Set(marquee.nodes);
    return projection.nodes.map((node) => ({
      ...node,
      // 多选期间由框选说了算：受控的 selected 必须与 xyflow 内部一致，
      // 否则下一次投影会把框选覆盖掉。
      selected: multiSelected ? picked.has(node.id) : node.selected,
      data: {
        ...node.data,
        onPortSelect: (direction: FlowHandleDirection, handle: string) =>
          selectPort(node.id, direction, handle),
      },
    }));
  }, [projection.nodes, selectPort, marquee.nodes, multiSelected]);

  const edges = useMemo<FlowViewEdge[]>(() => {
    const picked = new Set(marquee.edges);
    return projection.edges.map((edge) => ({
      ...edge,
      type: 'semantic',
      selected: multiSelected ? picked.has(edge.id) : edge.selected,
    }));
  }, [projection.edges, marquee.edges, multiSelected]);

  // 受控模式需要自己消费所有变更。位置只暂存本地，避免拖动每一帧
  // 都创建 layout history 和重新投影整个语义图；停止拖动后再一次性持久化。
  useEffect(() => {
    setFlowNodes(nodes);
  }, [nodes]);

  const onNodesChange = useCallback(
    (changes: Parameters<typeof applyNodeChanges<FlowViewNode>>[0]) => {
      for (const change of changes) {
        if (change.type === 'position' && change.position) {
          dragPositionsRef.current[change.id] = { ...change.position };
        }
      }
      setFlowNodes((current) => applyNodeChanges(changes, current));
    },
    [],
  );

  const commitDraggedPositions = useCallback(() => {
    const positions = dragPositionsRef.current;
    dragPositionsRef.current = {};
    if (Object.keys(positions).length > 0) store.getState().layoutNodes(positions);
  }, [store]);

  const entryByIntent = useMemo(() => flowEntryByIntentFromDefinition(definition), [definition]);

  const gateGraph = useMemo<GateGraph>(
    () => ({
      nodes: projection.nodes.map((node) => ({
        id: node.id,
        kind: node.data.kind,
        config: node.data.config,
      })),
      edges: projection.edges.map((edge) => edge.data.edge),
      focusEntry: intentFocus ? (entryByIntent[intentFocus] ?? null) : null,
    }),
    [projection.nodes, projection.edges, intentFocus, entryByIntent],
  );

  /** 校验并记录拒绝原因；`gate` 用于 isValidConnection，不落状态。 */
  const check = useCallback(
    (connection: Connection, graph: GateGraph = gateGraph): boolean => {
      const verdict = validateConnection(connection, graph);
      setRejectReason(verdict.ok ? null : verdict.reason);
      return verdict.ok;
    },
    [gateGraph],
  );

  // 拒绝提示自动消散，避免一直压在画布上。
  useEffect(() => {
    if (!rejectReason) return;
    const timer = setTimeout(() => setRejectReason(null), REJECT_HINT_MS);
    return () => clearTimeout(timer);
  }, [rejectReason]);

  const toSemanticEdge = useCallback(
    (connection: Connection) => semanticEdgeFromConnection(connection),
    [],
  );

  /** 当前作用于批量操作的节点集合：优先框选，其次单点选中。 */
  const operableNodeIds = useCallback((): string[] => {
    if (marquee.nodes.length > 0) return marquee.nodes;
    const parsed = parseEditorSelection(store.getState().core.selection);
    return parsed.kind === 'node' ? [parsed.nodeId] : [];
  }, [marquee.nodes, store]);

  /** 把一组节点连同其内部边打包成可粘贴的子图。 */
  const snapshot = useCallback(
    (nodeIds: string[]) => subgraphFromProjection(projection, nodeIds),
    [projection],
  );

  const deleteSelection = useCallback(
    (nodeIds: string[], edgeIds: string[]) => {
      if (nodeIds.length === 0 && edgeIds.length === 0) return;
      setMarquee(NO_MARQUEE);
      store.getState().deleteSelection(nodeIds, edgeIds);
    },
    [store],
  );

  // 快捷键：撤销 / 重做 / 复制 / 粘贴 / 直接复制。
  // 删除交给 xyflow 内置的 deleteKeyCode，它本来就按整个选中集走 onDelete。
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const { target } = event;
      if (
        target instanceof HTMLElement &&
        (target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.tagName === 'SELECT' ||
          target.isContentEditable)
      ) {
        return;
      }
      const modifier = event.metaKey || event.ctrlKey;
      if (!modifier || event.altKey) return;
      const state = store.getState();
      switch (event.key.toLowerCase()) {
        case 'z': {
          event.preventDefault();
          if (event.shiftKey) state.redo();
          else state.undo();
          return;
        }
        case 'y': {
          event.preventDefault();
          state.redo();
          return;
        }
        case 'c': {
          const copied = snapshot(operableNodeIds());
          if (!copied) return;
          event.preventDefault();
          clipboardRef.current = copied;
          return;
        }
        case 'v': {
          const pending = clipboardRef.current;
          if (!pending) return;
          event.preventDefault();
          setMarquee(NO_MARQUEE);
          state.pasteSubgraph(pending);
          return;
        }
        case 'd': {
          const ids = operableNodeIds();
          if (ids.length === 0) return;
          event.preventDefault();
          setMarquee(NO_MARQUEE);
          state.duplicateNodes(ids);
        }
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [store, snapshot, operableNodeIds]);

  // 意图焦点切换后把可达子图带进视野。
  useEffect(() => {
    if (!intentFocus) return;
    const timer = setTimeout(() => fitView({ padding: 0.3, duration: 240 }), 0);
    return () => clearTimeout(timer);
  }, [intentFocus, fitView]);

  // 诊断点击消费一次 reveal 请求：选中节点并让它进入可视区。
  useEffect(() => {
    if (!revealRequest) return;
    const frame = requestAnimationFrame(() => {
      void fitView({ nodes: [{ id: revealRequest.nodeId }], padding: 0.4, duration: 240 });
    });
    return () => cancelAnimationFrame(frame);
  }, [fitView, revealRequest]);

  const regionBounds = useCallback(
    (region: LoopRegionView): LoopRegionBounds => {
      const ids = new Set([region.loopNodeId, ...region.bodyNodes]);
      const members = nodes.filter((node) => ids.has(node.id));
      const source =
        members.length > 0 ? members : nodes.filter((node) => node.id === region.loopNodeId);
      if (source.length === 0) return { left: 0, top: 0, width: 0, height: 0 };
      const boxes = source.map((node) => {
        const size = nodeSize(node.id);
        return {
          left: node.position.x,
          top: node.position.y,
          right: node.position.x + size.width,
          bottom: node.position.y + size.height,
        };
      });
      const left = Math.min(...boxes.map((box) => box.left));
      const top = Math.min(...boxes.map((box) => box.top));
      const right = Math.max(...boxes.map((box) => box.right));
      const bottom = Math.max(...boxes.map((box) => box.bottom));
      return {
        left: left - REGION_PADDING_X,
        top: top - REGION_PADDING_Y,
        width: right - left + REGION_PADDING_X * 2,
        height: bottom - top + REGION_PADDING_Y * 2,
      };
    },
    [nodes, nodeSize],
  );

  const onSelectionChange = useCallback(
    ({ nodes: selectedNodes, edges: selectedEdges }: OnSelectionChangeParams) => {
      const state = store.getState();
      const nodeIds = selectedNodes.map((node) => node.id);
      const edgeIds = selectedEdges.map((edge) => edge.id);
      setMarquee((prev) =>
        sameIds(prev.nodes, nodeIds) && sameIds(prev.edges, edgeIds)
          ? prev
          : { nodes: nodeIds, edges: edgeIds },
      );
      // 多选时检查器留空：它只处理单个对象，选了一堆再显示其中一个会误导。
      if (nodeIds.length + edgeIds.length > 1) {
        if (state.core.selection !== null) state.selectNode(null);
        return;
      }
      const [selectedEdge] = edgeIds;
      if (selectedEdge) {
        if (state.core.selection !== `edge:${selectedEdge}`) state.selectEdge(selectedEdge);
        return;
      }
      const next = nodeIds[0] ?? null;
      if (state.core.selection !== next) state.selectNode(next);
    },
    [store],
  );

  /** ELK 布局落下后给一段位移过渡；跳变会让人丢失空间定位。 */
  const onLayoutApplied = useCallback(() => {
    setLayoutAnimating(true);
  }, []);

  useEffect(() => {
    if (!layoutAnimating) return;
    const timer = setTimeout(() => setLayoutAnimating(false), LAYOUT_ANIMATION_MS + 40);
    return () => clearTimeout(timer);
  }, [layoutAnimating]);

  return (
    <div
      ref={rootRef}
      data-slot="rule-flow-canvas"
      data-intent-focus={intentFocus ?? 'all'}
      data-layout-animating={layoutAnimating ? '' : undefined}
      className={cn('relative h-full w-full overflow-hidden bg-canvas', className)}
    >
      <ReactFlow
        nodes={flowNodes}
        edges={edges}
        onNodesChange={onNodesChange}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        isValidConnection={(connection) =>
          validateConnection(connection as Connection, gateGraph).ok
        }
        onConnect={(connection) => {
          if (!check(connection)) return;
          store.getState().connect(toSemanticEdge(connection));
        }}
        onReconnect={(oldEdge, connection) => {
          // 重连要排除正在替换的旧边，否则原位置会被误判成 duplicate。
          if (!check(connection, { ...gateGraph, excludeEdgeId: oldEdge.id })) return;
          const { edge: previous } = (oldEdge as FlowViewEdge).data;
          store.getState().reconnect(previous, toSemanticEdge(connection));
        }}
        onNodeDragStop={commitDraggedPositions}
        onNodeClick={(_event, node) => store.getState().selectNode(node.id)}
        onEdgeClick={(_event, edge) => store.getState().selectEdge(edge.id)}
        onPaneClick={() => store.getState().selectNode(null)}
        onSelectionChange={onSelectionChange}
        onDelete={({ nodes: removedNodes, edges: removedEdges }) => {
          deleteSelection(
            removedNodes.map((node) => node.id),
            removedEdges.map((edge) => edge.id),
          );
        }}
        elevateEdgesOnSelect={false}
        snapGrid={[16, 16]}
        snapToGrid
        minZoom={0.2}
        maxZoom={2}
        fitView
        fitViewOptions={{ padding: 0.3 }}
        defaultEdgeOptions={{ type: 'smoothstep' }}
        connectionLineType={'smoothstep' as never}
        colorMode={resolvedTheme}
        proOptions={{ hideAttribution: true }}
        onDragOver={onDragOver}
        onDrop={onDrop}
        aria-label={m.rules_canvas_aria()}
      >
        <FlowTopBar
          paletteOpen={paletteOpen}
          onTogglePalette={() => setPaletteOpen((v) => !v)}
          onLayoutApplied={onLayoutApplied}
        />
        {paletteOpen ? <NodePalette onClose={() => setPaletteOpen(false)} /> : null}
        <Background gap={20} size={1.5} />
        {/* React 版 ViewportPortal 只有一个容器，层级靠 rules-flow.css 的 z-index。 */}
        {projection.loopRegions.map((region) => (
          <ViewportPortal key={region.loopNodeId}>
            <LoopRegionOverlay
              region={region}
              bounds={regionBounds(region)}
              selected={selection === loopRegionSelectionToken(region.loopNodeId)}
              onSelect={() => store.getState().selectLoopRegion(region.loopNodeId)}
              onToggle={() =>
                store.getState().collapseLoopRegion(region.loopNodeId, !region.collapsed)
              }
            />
          </ViewportPortal>
        ))}
        {nodes.length > MINIMAP_NODE_THRESHOLD ? (
          <MiniMap pannable zoomable nodeColor={() => 'var(--lantern-strong)'} />
        ) : null}
        <Controls showInteractive={false} />
      </ReactFlow>

      {rejectReason ? (
        <div className="pointer-events-none absolute inset-x-3 top-3 z-(--layer-popover) flex justify-center">
          <p
            role="status"
            aria-live="polite"
            className="max-w-full border border-danger/40 bg-surface-1 px-3 py-1.5 text-ui-sm text-danger"
          >
            {rejectText(m, rejectReason)}
            <span className="ml-1 font-mono text-[10px] opacity-70">{rejectReason}</span>
          </p>
        </div>
      ) : null}
    </div>
  );
}
