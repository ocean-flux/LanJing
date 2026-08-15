<script lang="ts">
  import { ViewportPortal } from '@xyflow/svelte';
  import Icon from '$lib/components/Icon.svelte';
  import { Button } from '$lib/components/ui/button/index.js';
  import { cn } from '$lib/utils.js';
  import type { LoopRegionView } from '$lib/rules/native-authoring/flow-adapter';

  type Props = {
    region: LoopRegionView;
    left: number;
    top: number;
    width: number;
    height: number;
    selected: boolean;
    onSelect: () => void;
    onToggle: () => void;
  };

  let { region, left, top, width, height, selected, onSelect, onToggle }: Props = $props();

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    onSelect();
  }
</script>

<div
  class={cn('loop-region-overlay', selected && 'loop-region-overlay-selected')}
  data-loop-region={region.loopNodeId}
  data-loop-region-status={region.status}
  data-loop-region-collapsed={region.collapsed}
  style:left={`${left}px`}
  style:top={`${top}px`}
  style:width={`${width}px`}
  style:height={`${height}px`}
  aria-label={`Loop 区域 ${region.loopNodeId}`}
>
  <ViewportPortal target="front">
    <div
      class="loop-region-heading"
      style:left={`${left + 10}px`}
      style:top={`${top + 6}px`}
      style:max-width={`${Math.max(width - 20, 0)}px`}
    >
      <Button
        type="button"
        variant="outline"
        size="xs"
        class="loop-region-select"
        aria-pressed={selected}
        aria-label={`选择 Loop 区域 ${region.loopNodeId}`}
        onclick={onSelect}
        onkeydown={handleKeydown}
      >
        <Icon name="arrow-counter-clockwise" class="size-3.5" />
        <span>Loop 区域</span>
        <code>{region.loopNodeId}</code>
        <span class="loop-region-state">{region.status === 'valid' ? '有效' : '待修复'}</span>
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        class="loop-region-toggle"
        aria-label={region.collapsed ? '展开 Loop 区域' : '折叠 Loop 区域'}
        title={region.collapsed ? '展开 Loop 区域' : '折叠 Loop 区域'}
        onclick={(event) => {
          event.stopPropagation();
          onToggle();
        }}
      >
        <Icon name={region.collapsed ? 'caret-down' : 'caret-up'} class="size-3.5" />
      </Button>
    </div>
  </ViewportPortal>

  <div class="loop-region-boundary-labels" aria-hidden="true">
    <span>body entry</span>
    <span>yield 回边</span>
    <span>done 出口</span>
  </div>
</div>

<style>
  .loop-region-overlay {
    position: absolute;
    z-index: 0;
    pointer-events: none;
    border: 1px dashed color-mix(in oklab, var(--lantern-strong) 62%, var(--hairline-strong));
    border-radius: 10px;
    background: color-mix(in oklab, var(--lantern-soft) 26%, transparent);
  }

  .loop-region-overlay-selected {
    border-color: var(--lantern-strong);
    box-shadow: 0 0 0 2px var(--ring);
  }

  .loop-region-heading {
    position: absolute;
    display: flex;
    align-items: center;
    gap: 3px;
    pointer-events: auto;
  }

  :global(.loop-region-select),
  :global(.loop-region-toggle) {
    display: inline-flex;
    height: 24px;
    align-items: center;
    border: 1px solid var(--hairline-strong);
    background: var(--surface-panel);
    color: var(--ink-muted);
    box-shadow: var(--surface-control-shadow);
  }

  :global(.loop-region-select) {
    min-width: 0;
    gap: 5px;
    padding: 0 7px;
    border-radius: 5px 0 0 5px;
    font-size: 10px;
    font-weight: 600;
  }

  :global(.loop-region-select:hover),
  :global(.loop-region-select[aria-pressed='true']) {
    color: var(--ink);
    background: var(--lantern-soft);
  }

  :global(.loop-region-select code) {
    min-width: 0;
    overflow: hidden;
    color: var(--ink-subtle);
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .loop-region-state {
    color: var(--warning);
    font-size: 9px;
    font-weight: 500;
  }

  :global(.loop-region-toggle) {
    width: 24px;
    justify-content: center;
    border-left: 0;
    border-radius: 0 5px 5px 0;
  }

  :global(.loop-region-toggle:hover) {
    color: var(--ink);
    background: var(--surface-2);
  }

  .loop-region-boundary-labels {
    position: absolute;
    right: 8px;
    bottom: 7px;
    display: flex;
    gap: 4px;
    color: var(--ink-subtle);
    font-family: var(--font-mono);
    font-size: 8px;
    pointer-events: none;
  }

  .loop-region-boundary-labels span + span::before {
    margin-right: 4px;
    color: var(--hairline-strong);
    content: '/';
  }
</style>
