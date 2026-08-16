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
  useReactFlow,
  type Connection,
  type EdgeTypes,
  type NodeTypes,
  type OnSelectionChangeParams,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
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
  semanticHandleForUi,
  type FlowHandleDirection,
  type FlowViewEdge,
  type FlowViewNode,
  type LoopRegionView,
} from './model/flow-adapter';
import { selectFlowProjection } from './model/session';
import { useRuleEditorSession, useRuleEditorSessionStore } from './use-session';
import { RuleFlowNode } from './nodes/RuleFlowNode';
import { SemanticEdge } from './edges/SemanticEdge';
import { LoopRegionOverlay, type LoopRegionBounds } from './LoopRegionOverlay';
import { FlowTopBar } from './FlowTopBar';
import { NodePalette, usePaletteDrop } from './NodePalette';
import './rules-flow.css';

/** 节点数超过该阈值才渲染缩略图，小图不占画布空间。 */
const MINIMAP_NODE_THRESHOLD = 12;

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
};

const edgeTypes: EdgeTypes = { semantic: SemanticEdge };

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
    let queued = false;
    const bump = () => {
      if (disposed || queued) return;
      queued = true;
      queueMicrotask(() => {
        queued = false;
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
  const { fitView } = useReactFlow();
  const { onDragOver, onDrop } = usePaletteDrop();

  const projection = useRuleEditorSession(selectFlowProjection);
  const definition = useRuleEditorSession((state) => state.core.definition);
  const selection = useRuleEditorSession((state) => state.core.selection);
  const intentFocus = useRuleEditorSession((state) => state.core.intentFocus);

  /** 端口选择要把可视 handle 还原成 compiler 的 semantic handle。 */
  const selectPort = useCallback(
    (nodeId: string, direction: FlowHandleDirection, uiHandle: string) => {
      const node = selectFlowProjection(store.getState()).nodes.find(
        (candidate) => candidate.id === nodeId,
      );
      if (!node) return;
      store
        .getState()
        .selectPort(
          nodeId,
          direction,
          semanticHandleForUi(node.data.kind, node.data.config, uiHandle, direction),
        );
    },
    [store],
  );

  const nodes = useMemo<FlowViewNode[]>(
    () =>
      projection.nodes.map((node) => ({
        ...node,
        data: {
          ...node.data,
          onPortSelect: (direction: FlowHandleDirection, handle: string) =>
            selectPort(node.id, direction, handle),
        },
      })),
    [projection.nodes, selectPort],
  );

  const edges = useMemo<FlowViewEdge[]>(
    () => projection.edges.map((edge) => ({ ...edge, type: 'semantic' })),
    [projection.edges],
  );

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
    (connection: Connection) =>
      semanticEdgeFromConnection(
        connection,
        gateGraph.nodes.find((node) => node.id === connection.source),
        gateGraph.nodes.find((node) => node.id === connection.target),
      ),
    [gateGraph],
  );

  const deleteSelection = useCallback(
    (nodeIds: string[], edgeIds: string[]) => {
      if (nodeIds.length === 0 && edgeIds.length === 0) return;
      store.getState().deleteSelection(nodeIds, edgeIds);
    },
    [store],
  );

  // 快捷键：撤销 / 重做 / 删除。输入类元素内不拦截。
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
      const state = store.getState();
      if (modifier && !event.altKey && event.key.toLowerCase() === 'z') {
        event.preventDefault();
        if (event.shiftKey) state.redo();
        else state.undo();
        return;
      }
      if (modifier && !event.altKey && event.key.toLowerCase() === 'y') {
        event.preventDefault();
        state.redo();
        return;
      }
      if (event.key !== 'Delete' && event.key !== 'Backspace') return;
      const current = state.core.selection;
      if (!current) return;
      const parsed = parseEditorSelection(current);
      if (parsed.kind === 'edge') {
        event.preventDefault();
        deleteSelection([], [parsed.edgeId]);
      } else if (parsed.kind === 'node') {
        event.preventDefault();
        deleteSelection([parsed.nodeId], []);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [store, deleteSelection]);

  // 意图焦点切换后把可达子图带进视野。
  useEffect(() => {
    if (!intentFocus) return;
    const timer = setTimeout(() => fitView({ padding: 0.3, duration: 240 }), 0);
    return () => clearTimeout(timer);
  }, [intentFocus, fitView]);

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
      const [selectedEdge] = selectedEdges;
      if (selectedEdge) {
        if (state.core.selection !== `edge:${selectedEdge.id}`) state.selectEdge(selectedEdge.id);
        return;
      }
      const next = selectedNodes[0]?.id ?? null;
      if (state.core.selection !== next) state.selectNode(next);
    },
    [store],
  );

  return (
    <div
      ref={rootRef}
      data-slot="rule-flow-canvas"
      data-intent-focus={intentFocus ?? 'all'}
      className={cn('relative h-full w-full overflow-hidden bg-canvas', className)}
    >
      <ReactFlow
        nodes={nodes}
        edges={edges}
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
        onNodeDragStop={(_event, node) => {
          store.getState().moveNode(node.id, { x: node.position.x, y: node.position.y });
        }}
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
        <FlowTopBar paletteOpen={paletteOpen} onTogglePalette={() => setPaletteOpen((v) => !v)} />
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
