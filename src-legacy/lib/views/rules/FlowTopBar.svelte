<script lang="ts">
  //! 画布顶栏：意图聚焦切换（全部 + 六意图）+ 添加节点入口。
  //! 渲染在 Svelte Flow Panel 内（provider 子树），可调用 useSvelteFlow 做 fit-view。
  //! 聚焦只改变 adapter 投影的 dim + 视口对齐，绝不复制节点。

  import { Panel, useSvelteFlow } from '@xyflow/svelte';
  import { onMount, tick } from 'svelte';
  import Icon from '$lib/components/Icon.svelte';
  import { Button } from '$lib/components/ui/button/index.js';
  import { ToggleGroup, ToggleGroupItem } from '$lib/components/ui/toggle-group/index.js';
  import { m } from '$lib/i18n';
  import { cn } from '$lib/utils.js';
  import type { FlowNodeDimensions } from '$lib/rules/native-authoring/flow-layout';
  import type { FlowEdge, StandardIntent } from '$lib/rules/native-authoring/wire';
  import { INTENT_LABELS, STANDARD_INTENTS } from './intents';
  import { reachableNodes } from './connection-gate';

  type Props = {
    intentFocus: StandardIntent | null;
    nodeIds: string[];
    /** 意图 flow_entry 映射（点击切换时计算可达集用）。 */
    flowEntryByIntent: Partial<Record<StandardIntent, string>>;
    /** 语义边列表（fit-view 可达集用）。 */
    edges: FlowEdge[];
    onFocusIntent: (intent: StandardIntent | null) => void;
    onAutoLayout: (dimensions: FlowNodeDimensions) => Promise<void>;
    paletteOpen: boolean;
    onTogglePalette: () => void;
  };

  let {
    intentFocus,
    nodeIds,
    flowEntryByIntent,
    edges,
    onFocusIntent,
    onAutoLayout,
    paletteOpen,
    onTogglePalette,
  }: Props = $props();

  const flow = useSvelteFlow();
  let autoFitDone = false;
  let autoLayoutPending = $state(false);

  // 文档通过 IPC 异步进入画布；节点层出现后补 fit-view，避免初始视口落在空区域。
  onMount(() => {
    let observer: MutationObserver | undefined;
    let retryTimer: ReturnType<typeof setTimeout> | undefined;
    let retries = 0;

    const fitWhenReady = () => {
      if (autoFitDone || nodeIds.length === 0) return;
      autoFitDone = true;
      observer?.disconnect();
      flow.fitView({ padding: 0.3, duration: 0 });
    };

    const observeNodes = () => {
      const nodeLayer = document.querySelector('.svelte-flow__nodes');
      if (nodeLayer) {
        observer = new MutationObserver(fitWhenReady);
        observer.observe(nodeLayer, { childList: true });
        fitWhenReady();
        return;
      }
      if (retries < 12) {
        retries += 1;
        retryTimer = setTimeout(observeNodes, 0);
      }
    };

    observeNodes();
    return () => {
      observer?.disconnect();
      if (retryTimer) clearTimeout(retryTimer);
    };
  });

  /** 聚焦切换：只派发 focusIntent + 视口对齐（不改动节点集合）。 */
  function focusOn(intent: StandardIntent | null) {
    onFocusIntent(intent);
    const entry = intent ? (flowEntryByIntent[intent] ?? null) : null;
    if (entry) {
      const nodeIds = reachableNodes(entry, edges);
      flow.fitView({
        nodes: [...nodeIds].map((id) => ({ id })),
        padding: 0.3,
        duration: 240,
      });
    } else {
      flow.fitView({ padding: 0.3, duration: 240 });
    }
  }

  function focusFromValue(value: string) {
    if (value === 'all' || value === '') {
      focusOn(null);
      return;
    }
    if (STANDARD_INTENTS.includes(value as StandardIntent)) {
      focusOn(value as StandardIntent);
    }
  }

  async function handleAutoLayout() {
    if (autoLayoutPending || nodeIds.length === 0) return;
    autoLayoutPending = true;
    try {
      const dimensions = Object.fromEntries(
        nodeIds.flatMap((nodeId) => {
          const internalNode = flow.getInternalNode(nodeId);
          if (!internalNode) return [];
          const width = internalNode.measured.width ?? internalNode.width;
          const height = internalNode.measured.height ?? internalNode.height;
          if (!width && !height) return [];
          return [[nodeId, { width, height }]];
        }),
      );
      await onAutoLayout(dimensions);
      await tick();
      flow.fitView({ padding: 0.24, duration: 240 });
    } finally {
      autoLayoutPending = false;
    }
  }
</script>

<Panel position="top-left" class="flow-topbar">
  <div class="flex max-w-full items-center gap-1.5 p-1.5">
    <ToggleGroup
      type="single"
      value={intentFocus ?? 'all'}
      onValueChange={focusFromValue}
      size="sm"
      class="flex max-w-full items-center gap-1 overflow-x-auto rounded-lg border border-hairline bg-surface-panel/85 p-1 shadow-[var(--surface-panel-shadow)] backdrop-blur-[var(--material-blur)]"
      aria-label="意图聚焦"
    >
      <ToggleGroupItem
        value="all"
        class="shrink-0 border border-hairline bg-surface-control text-ink-muted hover:bg-surface-2 hover:text-ink data-[state=on]:border-lantern-strong/50 data-[state=on]:bg-lantern-soft data-[state=on]:text-ink"
        >全部</ToggleGroupItem
      >
      {#each STANDARD_INTENTS as intent (intent)}
        <ToggleGroupItem
          value={intent}
          class="shrink-0 border border-hairline bg-surface-control text-ink-muted hover:bg-surface-2 hover:text-ink data-[state=on]:border-lantern-strong/50 data-[state=on]:bg-lantern-soft data-[state=on]:text-ink"
          >{INTENT_LABELS[intent]}</ToggleGroupItem
        >
      {/each}
    </ToggleGroup>
    <Button
      type="button"
      variant="outline"
      size="sm"
      aria-busy={autoLayoutPending}
      disabled={autoLayoutPending || nodeIds.length === 0}
      aria-label={m.rules_canvas_auto_layout()}
      title={m.rules_canvas_auto_layout()}
      onclick={handleAutoLayout}
    >
      <Icon name={autoLayoutPending ? 'arrow-clockwise' : 'tree-structure'} class="size-3.5" />
      <span>{m.rules_canvas_auto_layout()}</span>
    </Button>
    <Button
      type="button"
      variant={paletteOpen ? 'secondary' : 'outline'}
      size="xs"
      aria-expanded={paletteOpen}
      aria-haspopup="dialog"
      class={cn('shrink-0', paletteOpen && 'border-lantern-strong/50 bg-lantern-soft')}
      onclick={onTogglePalette}
    >
      <Icon name="plus" class="size-3.5" />
      <span>添加节点</span>
    </Button>
  </div>
</Panel>
