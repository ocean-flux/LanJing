<script lang="ts">
  //! 规则流程图画布：Svelte Flow 受控容器。
  //!
  //! nodes/edges 全部来自 session.flowProjection（adapter 投影），画布不持有 core；
  //! onconnect/onreconnect/onnodeschange 经连接 gate 校验后发 typed action 到 session。
  //! 校验失败返回 false / 不派发，绝不写入无效 semantic state。

  import {
    Background,
    Controls,
    ViewportPortal,
    SvelteFlow,
    type Connection,
    type IsValidConnection,
    type ConnectionLineType,
  } from '@xyflow/svelte';
  import { onMount } from 'svelte';
  import '@xyflow/svelte/dist/style.css';
  import { m } from '$lib/i18n';
  import { cn } from '$lib/utils.js';
  import type { NativeRuleEditorSession } from '$lib/rules/native-authoring/session.svelte';
  import type { StandardIntent } from '$lib/rules/native-authoring/wire';
  import {
    layoutNativeRuleFlow,
    type FlowNodeDimensions,
  } from '$lib/rules/native-authoring/flow-layout';
  import {
    parseEditorSelection,
    semanticHandleForUi,
    type FlowHandleDirection,
  } from '$lib/rules/native-authoring/flow-adapter';
  import {
    semanticEdgeFromConnection,
    validateConnection,
    flowEntryByIntentFromDefinition,
  } from './connection-gate';
  import FlowTopBar from './FlowTopBar.svelte';
  import NodePalette from './NodePalette.svelte';
  import LoopRegionOverlay from './LoopRegionOverlay.svelte';
  import ViewportSync from './ViewportSync.svelte';
  import SemanticEdge from './SemanticEdge.svelte';
  import { nodeTypes } from './nodes';
  import type { FlowViewEdge, FlowViewNode } from './nodes/types';

  type Props = {
    session: NativeRuleEditorSession;
    class?: string;
  };

  let { session, class: className }: Props = $props();

  const SMOOTH_STEP_CONNECTION_LINE = 'smoothstep' as ConnectionLineType;
  const CANVAS_ARIA_LABELS = {
    'controls.ariaLabel': m.rules_canvas_controls(),
    'controls.zoomIn.ariaLabel': m.rules_canvas_zoom_in(),
    'controls.zoomOut.ariaLabel': m.rules_canvas_zoom_out(),
    'controls.fitView.ariaLabel': m.rules_canvas_fit_view(),
    'controls.interactive.ariaLabel': m.rules_canvas_interactive(),
  };

  // ---- 投影（adapter 派生，响应式） ----
  const viewNodes = $derived.by(() =>
    session.flowProjection.nodes.map((node) => ({
      ...node,
      data: {
        ...node.data,
        onPortSelect: (direction: FlowHandleDirection, handle: string) =>
          handlePortSelect(node.id, direction, handle),
      },
    })),
  );
  const nodeIds = $derived(viewNodes.map((node) => node.id));
  const viewEdges = $derived(
    session.flowProjection.edges.map((edge) => ({
      ...edge,
      type: 'semantic',
      class: cn('flow-edge', `flow-edge-${edge.data.role}`, edge.data.dimmed && 'flow-edge-dimmed'),
    })),
  );
  const loopRegions = $derived(session.flowProjection.loopRegions ?? []);
  const loopRegionViews = $derived.by(() =>
    loopRegions.map((region) => ({ region, bounds: regionBounds(region) })),
  );
  const intentFocus = $derived(session.intentFocus);
  const semanticEdges = $derived(viewEdges.map((edge) => edge.data.edge));
  const entryByIntent = $derived(flowEntryByIntentFromDefinition(session.definition));
  const focusEntry = $derived(intentFocus ? (entryByIntent[intentFocus] ?? null) : null);

  // ---- 连接 gate：随投影变化重建 ----
  const gateGraph = $derived({
    nodes: viewNodes.map((node) => ({
      id: node.id,
      kind: node.data.kind,
      config: node.data.config,
    })),
    edges: semanticEdges,
    focusEntry,
  });

  const gate = $derived.by(
    () => (connection: Connection) => validateConnection(connection, gateGraph).ok,
  );

  /** onconnect：gate 通过才派发 typed action。 */
  function handleConnect(connection: Connection) {
    if (!gate(connection)) return;
    session.connect(toSemanticEdge(connection));
  }

  async function handleAutoLayout(dimensions: FlowNodeDimensions): Promise<void> {
    const positions = await layoutNativeRuleFlow(session.definition, dimensions);
    session.layoutNodes(positions);
  }

  function handlePortSelect(
    nodeId: string,
    direction: FlowHandleDirection,
    uiHandle: string,
  ): void {
    const node = session.flowProjection.nodes.find((candidate) => candidate.id === nodeId);
    if (!node) return;
    session.selectPort(
      nodeId,
      direction,
      semanticHandleForUi(node.data.kind, node.data.config, uiHandle, direction),
    );
  }

  /** onbeforeconnect：不兼容返回 false，不写无效 semantic state。 */
  function handleBeforeConnect(connection: Connection): Connection | false {
    return gate(connection) ? connection : false;
  }

  /** onreconnect：重连目标同样过 gate。 */
  function handleReconnect(oldEdge: FlowViewEdge, connection: Connection) {
    if (!gate(connection)) return;
    session.reconnect(oldEdge.data.edge, toSemanticEdge(connection));
  }

  function toSemanticEdge(connection: Connection) {
    const sourceNode = gateGraph.nodes.find((node) => node.id === connection.source);
    const targetNode = gateGraph.nodes.find((node) => node.id === connection.target);
    return semanticEdgeFromConnection(connection, sourceNode, targetNode);
  }

  /** onnodedragstop：拖动结束 → 发 moveNode。 */
  function handleNodeDragStop(event: {
    targetNode: FlowViewNode | null;
    nodes: FlowViewNode[];
    event: MouseEvent | TouchEvent;
  }) {
    const node = event.targetNode ?? event.nodes[0];
    if (!node) return;
    session.moveNode(node.id, {
      x: node.position.x,
      y: node.position.y,
    });
  }

  /** onnodeclick：选中节点。 */
  function handleNodeClick(event: { node: FlowViewNode; event: MouseEvent | TouchEvent }) {
    const target = event.event.target;
    if (target instanceof Element) {
      const portRow = target.closest<HTMLElement>('.flow-node-port-row');
      if (portRow) {
        const side = portRow.dataset.portSide;
        const handle = portRow.dataset.portId;
        if ((side === 'input' || side === 'output') && handle) {
          handlePortSelect(event.node.id, side === 'input' ? 'target' : 'source', handle);
          return;
        }
      }
    }
    session.selectNode(event.node.id);
  }

  /** onedgeclick：选中连线，Inspector 读取同一条 semantic edge。 */
  function handleEdgeClick(event: { edge: FlowViewEdge; event: MouseEvent }) {
    session.selectEdge(event.edge.id);
  }

  function handleSelectionChange(event: { nodes: FlowViewNode[]; edges: FlowViewEdge[] }) {
    const selectedEdge = event.edges[0];
    if (selectedEdge) {
      if (session.selection !== `edge:${selectedEdge.id}`) {
        session.selectEdge(selectedEdge.id);
      }
      return;
    }

    const selectedNode = event.nodes[0];
    const nextSelection = selectedNode?.id ?? null;
    if (session.selection !== nextSelection) {
      session.selectNode(nextSelection);
    }
  }

  /** onpaneclick：取消选中。 */
  function handlePaneClick() {
    session.selectNode(null);
  }

  /** ondelete：删除已选节点与边。 */
  function handleDelete(event: { nodes: FlowViewNode[]; edges: FlowViewEdge[] }) {
    for (const edge of event.edges) {
      session.disconnect(edge.data.edge);
    }
    for (const node of event.nodes) {
      session.deleteNode(node.id);
    }
  }

  // ---- 键盘删除：Backspace/Delete 删除选中节点（incident 边由 session 组合） ----
  // 外部监听挂载即订阅、卸载即 teardown。
  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.tagName === 'SELECT' ||
          target.isContentEditable)
      ) {
        return;
      }
      if ((event.key === 'Delete' || event.key === 'Backspace') && session.selection) {
        event.preventDefault();
        const selection = parseEditorSelection(session.selection);
        if (selection.kind === 'edge') {
          const edgeId = selection.edgeId;
          const edge = session.flowProjection.edges.find((candidate) => candidate.id === edgeId);
          if (edge) session.disconnect(edge.data.edge);
        } else if (selection.kind === 'node') {
          session.deleteNode(selection.nodeId);
        }
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  // ---- palette 开关 ----
  let paletteOpen = $state(false);
  let flowNodeMeasurementEpoch = $state(0);
  let flowRoot: HTMLElement | null = null;

  const NODE_WIDTH = 252;
  const NODE_HEIGHT = 142;
  const REGION_PADDING_X = 34;
  const REGION_PADDING_Y = 38;

  function observeFlowNodeDimensions(root: HTMLElement) {
    flowRoot = root;
    let disposed = false;
    let refreshQueued = false;
    const refresh = () => {
      if (disposed || refreshQueued) return;
      refreshQueued = true;
      queueMicrotask(() => {
        refreshQueued = false;
        if (!disposed) flowNodeMeasurementEpoch += 1;
      });
    };
    const resizeObserver =
      typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(refresh);
    const observeNodes = () => {
      resizeObserver?.disconnect();
      root.querySelectorAll<HTMLElement>('.svelte-flow__node').forEach((node) => {
        resizeObserver?.observe(node);
      });
      refresh();
    };
    const mutationObserver =
      typeof MutationObserver === 'undefined' ? undefined : new MutationObserver(observeNodes);

    mutationObserver?.observe(root, { childList: true, subtree: true });
    observeNodes();

    return () => {
      disposed = true;
      refreshQueued = false;
      resizeObserver?.disconnect();
      mutationObserver?.disconnect();
      if (flowRoot === root) flowRoot = null;
    };
  }

  function nodeSize(nodeId: string): { width: number; height: number } {
    const fallback = { width: NODE_WIDTH, height: NODE_HEIGHT };
    if (!flowRoot || flowNodeMeasurementEpoch === 0) return fallback;
    const element = [...flowRoot.querySelectorAll<HTMLElement>('.svelte-flow__node')].find(
      (candidate) => candidate.dataset.id === nodeId,
    );
    return {
      width: element?.offsetWidth || fallback.width,
      height: element?.offsetHeight || fallback.height,
    };
  }

  function regionBounds(region: (typeof loopRegions)[number]): {
    left: number;
    top: number;
    width: number;
    height: number;
  } {
    const ids = [region.loopNodeId, ...region.bodyNodes];
    const members = viewNodes.filter((node) => ids.includes(node.id));
    const source =
      members.length > 0 ? members : viewNodes.filter((node) => node.id === region.loopNodeId);
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
  }

  function selectLoopRegion(loopNodeId: string): void {
    session.selectLoopRegion(loopNodeId);
  }

  function toggleLoopRegion(region: (typeof loopRegions)[number]): void {
    session.collapseLoopRegion(region.loopNodeId, !region.collapsed);
  }
</script>

<div
  {@attach observeFlowNodeDimensions}
  data-slot="rule-flow-canvas"
  data-intent-focus={intentFocus ?? 'all'}
  class={cn(
    'relative h-full w-full overflow-hidden rounded-[var(--radius-panel)] border border-hairline bg-canvas',
    className,
  )}
>
  <SvelteFlow
    nodes={viewNodes}
    edges={viewEdges}
    {nodeTypes}
    edgeTypes={{ semantic: SemanticEdge }}
    isValidConnection={gate as unknown as IsValidConnection}
    onbeforeconnect={handleBeforeConnect}
    onconnect={handleConnect}
    onreconnect={handleReconnect}
    onnodedragstop={handleNodeDragStop}
    onnodeclick={handleNodeClick}
    onedgeclick={handleEdgeClick}
    onpaneclick={handlePaneClick}
    onselectionchange={handleSelectionChange}
    ondelete={handleDelete}
    elevateEdgesOnSelect={false}
    snapGrid={[16, 16]}
    minZoom={0.2}
    maxZoom={2}
    fitView
    fitViewOptions={{ padding: 0.3 }}
    defaultEdgeOptions={{ type: 'smoothstep' }}
    connectionLineType={SMOOTH_STEP_CONNECTION_LINE}
    ariaLabelConfig={CANVAS_ARIA_LABELS}
    proOptions={{ hideAttribution: true }}
    colorMode="system"
    class="flow-canvas"
  >
    <FlowTopBar
      {intentFocus}
      {nodeIds}
      flowEntryByIntent={entryByIntent}
      edges={semanticEdges}
      onFocusIntent={(intent: StandardIntent | null) => session.focusIntent(intent)}
      onAutoLayout={handleAutoLayout}
      {paletteOpen}
      onTogglePalette={() => (paletteOpen = !paletteOpen)}
    />
    {#key viewNodes.length}
      <ViewportSync />
    {/key}
    {#if paletteOpen}
      <NodePalette {session} onClose={() => (paletteOpen = false)} />
    {/if}
    <Background bgColor="var(--canvas)" patternColor="var(--hairline-strong)" gap={20} size={1.5} />
    {#each loopRegionViews as loopRegion (loopRegion.region.loopNodeId)}
      <ViewportPortal target="back">
        <LoopRegionOverlay
          region={loopRegion.region}
          {...loopRegion.bounds}
          selected={session.selection ===
            `loop-region:${encodeURIComponent(loopRegion.region.loopNodeId)}`}
          onSelect={() => selectLoopRegion(loopRegion.region.loopNodeId)}
          onToggle={() => toggleLoopRegion(loopRegion.region)}
        />
      </ViewportPortal>
    {/each}
    <Controls class="flow-controls" />
  </SvelteFlow>
</div>

<style>
  /* Svelte Flow 画布主题：使用既有 Ethereal token，不引入第二套样式。 */
  :global(.flow-canvas) {
    background: var(--canvas);
    font-family: var(--font-ui);
  }

  /* Svelte Flow 的 viewport 子层按 DOM 顺序渲染；固定语义层，避免 overlay 盖住图形。 */
  :global(.flow-canvas .svelte-flow__viewport-back) {
    z-index: 0;
    pointer-events: none;
  }

  :global(.flow-canvas .svelte-flow__edges),
  :global(.flow-canvas .svelte-flow__edge-labels) {
    z-index: 1;
  }

  :global(.flow-canvas .svelte-flow__nodes) {
    z-index: 2;
  }

  :global(.flow-canvas .svelte-flow__node) {
    z-index: 2 !important;
  }

  :global(.flow-canvas .svelte-flow__node[data-selected='true']) {
    z-index: 3 !important;
  }

  :global(.flow-canvas .svelte-flow__nodesselection) {
    z-index: 3;
  }

  :global(.flow-canvas .svelte-flow__edge-path),
  :global(.flow-canvas .svelte-flow__connection-path) {
    stroke: var(--lantern-strong);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
    fill: none;
    transition:
      opacity var(--motion-fast) var(--motion-standard),
      stroke var(--motion-fast) var(--motion-standard),
      stroke-width var(--motion-fast) var(--motion-standard);
  }

  :global(.flow-canvas .flow-edge-dimmed) {
    opacity: 0.2;
  }

  :global(.flow-canvas .flow-edge-dimmed .svelte-flow__edge-path) {
    stroke: var(--ink-subtle);
  }

  :global(.flow-canvas .svelte-flow__edge:hover .svelte-flow__edge-path),
  :global(.flow-canvas .svelte-flow__edge.selected .svelte-flow__edge-path) {
    stroke: var(--lantern);
    stroke-width: 2.5;
  }

  :global(.flow-canvas .semantic-edge-control) {
    stroke: var(--lantern-strong);
    stroke-dasharray: 7 3;
  }

  :global(.flow-canvas .semantic-edge-binding) {
    stroke: var(--ink-muted);
    stroke-dasharray: 3 3;
  }

  :global(.flow-canvas .semantic-edge-branch) {
    stroke: var(--lantern-strong);
  }

  :global(.flow-canvas .semantic-edge-auxiliary) {
    opacity: 0.86;
  }

  :global(.flow-canvas .semantic-edge-lane-loop) {
    stroke: var(--lantern-strong);
  }

  :global(.flow-canvas .semantic-edge-loop-back) {
    stroke: var(--lantern-strong);
    stroke-dasharray: 8 4;
  }

  :global(.flow-canvas .semantic-edge-selected) {
    stroke: var(--lantern);
    stroke-width: 2.5;
  }

  :global(.flow-canvas .semantic-edge-label) {
    display: inline-flex;
    min-height: 18px;
    align-items: center;
    border: 1px solid var(--hairline-strong);
    border-radius: 4px;
    padding: 2px 5px;
    color: var(--ink-muted);
    background: var(--surface-panel);
    font-family: var(--font-mono);
    font-size: 9px;
    line-height: 1;
    box-shadow: var(--surface-control-shadow);
  }

  :global(.flow-canvas .semantic-edge-label-control) {
    border-color: color-mix(in oklab, var(--lantern-strong) 55%, var(--hairline-strong));
    color: var(--lantern-strong);
  }

  :global(.flow-canvas .semantic-edge-label-binding) {
    color: var(--ink-muted);
  }

  :global(.flow-canvas .svelte-flow__connection-path) {
    stroke: var(--lantern);
    stroke-dasharray: 5 4;
  }

  :global(.flow-canvas .svelte-flow__minimap) {
    background: var(--surface-panel);
    border: 1px solid var(--hairline);
    border-radius: var(--radius-md);
  }

  :global(.flow-canvas .svelte-flow__controls) {
    border: 1px solid var(--hairline);
    border-radius: var(--radius-md);
    overflow: hidden;
    box-shadow: var(--surface-control-shadow);
  }

  :global(.flow-canvas .svelte-flow__controls-button) {
    width: var(--density-control-md);
    height: var(--density-control-md);
    background: var(--surface-control);
    border-bottom: 1px solid var(--hairline);
    color: var(--ink-muted);
  }

  :global(.flow-canvas .svelte-flow__controls-button:hover) {
    background: var(--surface-2);
    color: var(--ink);
  }

  :global(.flow-canvas .svelte-flow__controls-button svg) {
    width: 15px;
    height: 15px;
    fill: currentColor;
  }

  @media (pointer: coarse) {
    :global(.flow-canvas .svelte-flow__controls-button) {
      width: var(--density-touch-target);
      height: var(--density-touch-target);
    }
  }
</style>
