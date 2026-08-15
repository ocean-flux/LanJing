<script lang="ts">
  //! 添加节点弹层：按当前选中节点/意图显示兼容推荐，
  //! 「全部节点」同层可达，不兼容项禁用并解释原因。
  //! 渲染在 Svelte Flow Panel 内；只调 session.addNode，不直接持有 core。

  import { onMount } from 'svelte';
  import { Panel } from '@xyflow/svelte';
  import Icon from '$lib/components/Icon.svelte';
  import { Button } from '$lib/components/ui/button/index.js';
  import { cn } from '$lib/utils.js';
  import type { FlowNodeKind } from '$lib/rules/native-authoring/wire';
  import type { NativeRuleEditorSession } from '$lib/rules/native-authoring/session.svelte';
  import {
    recommendKinds,
    flowEntryByIntentFromDefinition,
    type KindRecommendation,
  } from './connection-gate';
  import { INTENT_LABELS } from './intents';
  import { NODE_KIND_META } from './nodes/meta';

  type Props = {
    session: NativeRuleEditorSession;
    onClose: () => void;
  };

  let { session, onClose }: Props = $props();

  const nodes = $derived(session.flowProjection.nodes);
  const edges = $derived(session.flowProjection.edges);
  const selection = $derived(session.selection);
  const intentFocus = $derived(session.intentFocus);
  const entryByIntent = $derived(flowEntryByIntentFromDefinition(session.definition));
  const focusEntry = $derived(intentFocus ? (entryByIntent[intentFocus] ?? null) : null);

  const recommendations = $derived(
    recommendKinds({
      nodes: nodes.map((node) => ({ id: node.id, kind: node.data.kind })),
      edges: edges.map((edge) => edge.data.edge),
      selection,
      focusEntry,
    }),
  );

  const compatible = $derived(recommendations.filter((item) => item.compatible));

  /** 推荐区仅在实际存在过滤上下文时展示（避免与全部节点区完全重复）。 */
  const showRecommended = $derived(
    compatible.length > 0 && compatible.length < recommendations.length,
  );

  function addNode(kind: FlowNodeKind) {
    session.addNode(kind);
    onClose();
  }

  // 面板元素引用（供外部点击判定；非响应式，事件时读取即可）。
  let panel: HTMLDivElement | undefined = undefined;
  /** {@attach} 附着：挂载记引用，卸载清空。 */
  function panelRef(node: HTMLDivElement) {
    panel = node;
    return () => {
      panel = undefined;
    };
  }

  // 外部点击 / Esc 关闭（挂载即订阅，卸载即 teardown）。
  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    const onPointer = (event: PointerEvent) => {
      if (panel && event.target instanceof Node && !panel.contains(event.target)) {
        onClose();
      }
    };
    document.addEventListener('keydown', onKey);
    document.addEventListener('pointerdown', onPointer);
    return () => {
      document.removeEventListener('keydown', onKey);
      document.removeEventListener('pointerdown', onPointer);
    };
  });
</script>

{#snippet renderItem(item: KindRecommendation)}
  <li class="rounded-md">
    <Button
      type="button"
      variant="ghost"
      size="sm"
      disabled={!item.compatible}
      aria-disabled={!item.compatible}
      class={cn(
        'h-auto w-full items-start justify-start gap-2 rounded-md border px-2.5 py-1.5 text-left',
        item.compatible
          ? 'border-transparent bg-surface-2/60 hover:border-hairline-strong hover:bg-surface-2'
          : 'cursor-not-allowed border-transparent opacity-55',
      )}
      onclick={() => {
        if (!item.compatible) return;
        addNode(item.kind);
      }}
    >
      <Icon
        name={NODE_KIND_META[item.kind].icon}
        class={cn(
          'mt-0.5 size-3.5 shrink-0',
          item.compatible ? 'text-lantern-strong' : 'text-ink-subtle',
        )}
      />
      <span class="min-w-0">
        <span class="block text-xs font-medium text-ink">{item.label}</span>
        <span class="mt-0.5 block text-[11px] leading-3.5 text-ink-muted">
          {item.description}
        </span>
      </span>
    </Button>
    {#if item.reason}
      <p class="mt-1 px-2 text-[11px] leading-3.5 text-ink-subtle">{item.reason}</p>
    {/if}
  </li>
{/snippet}

<Panel position="top-right" class="flow-palette">
  <div
    {@attach panelRef}
    role="dialog"
    aria-label="添加节点"
    class="w-64 rounded-[var(--radius-overlay)] border border-hairline-strong bg-surface-overlay p-2 shadow-[var(--surface-overlay-shadow)] backdrop-blur-[var(--material-blur)]"
  >
    <header class="flex items-center justify-between px-2 pt-1 pb-1.5">
      <h2 class="text-sm font-semibold text-ink">添加节点</h2>
      <Button type="button" variant="ghost" size="icon-xs" aria-label="关闭" onclick={onClose}>
        <Icon name="x" class="size-4" />
      </Button>
    </header>

    <div class="max-h-[min(60vh,26rem)] overflow-y-auto px-1 pb-1">
      {#if showRecommended}
        <p
          data-palette-section="recommended"
          class="px-1 pt-1 pb-1 text-[11px] font-medium tracking-wide text-ink-subtle"
        >
          推荐
        </p>
        <ul class="space-y-1">
          {#each compatible as item (item.kind)}
            {@render renderItem(item)}
          {/each}
        </ul>
      {/if}

      <p
        data-palette-section="all"
        class="px-1 pt-2 pb-1 text-[11px] font-medium tracking-wide text-ink-subtle"
      >
        全部节点
      </p>
      <ul class="space-y-1">
        {#each recommendations as item (item.kind)}
          {@render renderItem(item)}
        {/each}
      </ul>
    </div>

    <footer class="border-t border-hairline px-2 pt-1.5 pb-0.5">
      <p class="text-[11px] leading-3.5 text-ink-subtle">
        {#if intentFocus}
          当前聚焦：{INTENT_LABELS[intentFocus]}（新增节点先连接焦点内的入口节点）
        {:else}
          全部视图：可从任一选中节点的输出继续添加
        {/if}
      </p>
    </footer>
  </div>
</Panel>
