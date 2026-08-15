<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
  import { Badge } from '$lib/components/ui/badge/index.js';
  import { Button } from '$lib/components/ui/button/index.js';
  import { Separator } from '$lib/components/ui/separator/index.js';
  import type { LoopRegionView } from '$lib/rules/native-authoring/flow-adapter';

  type Props = {
    region: LoopRegionView | null;
    onToggleCollapsed: (collapsed: boolean) => void;
  };

  let { region, onToggleCollapsed }: Props = $props();
</script>

{#if !region}
  <div class="flex min-h-40 items-center justify-center px-4">
    <p class="text-center text-sm text-ink-muted">选择 Loop 区域查看结构状态</p>
  </div>
{:else}
  <div class="flex flex-col gap-3">
    <div class="flex items-start gap-2 px-1">
      <span
        class="flex size-7 shrink-0 items-center justify-center rounded-md bg-lantern-soft text-lantern-strong"
      >
        <Icon name="arrow-counter-clockwise" class="size-4" />
      </span>
      <div class="min-w-0 flex-1">
        <p class="text-xs font-semibold text-ink">Loop structured region</p>
        <code class="block truncate text-[10px] text-ink-muted">{region.loopNodeId}</code>
      </div>
      <Badge variant="outline">{region.status === 'valid' ? '有效' : '待修复'}</Badge>
    </div>

    <Separator />

    <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-2 px-1 text-xs">
      <dt class="text-ink-muted">body entry</dt>
      <dd class="truncate text-right font-mono text-ink">
        {region.bodyEntry ? `${region.bodyEntry.node_id} · ${region.bodyEntry.handle}` : '未连接'}
      </dd>
      <dt class="text-ink-muted">yield source</dt>
      <dd class="truncate text-right font-mono text-ink">
        {region.yieldSource
          ? `${region.yieldSource.node_id} · ${region.yieldSource.handle}`
          : '未连接'}
      </dd>
      <dt class="text-ink-muted">body nodes</dt>
      <dd class="text-right font-mono text-ink">{region.bodyNodes.length}</dd>
      <dt class="text-ink-muted">边界边</dt>
      <dd class="text-right font-mono text-ink">{region.boundaryEdgeIds.length}</dd>
    </dl>

    {#if region.diagnostics.length > 0}
      <section class="flex flex-col gap-1.5 px-1" aria-labelledby="loop-region-diagnostics-title">
        <h3 id="loop-region-diagnostics-title" class="text-xs font-semibold text-ink">结构诊断</h3>
        <ul class="flex flex-col gap-1 text-[11px] leading-4 text-warning">
          {#each region.diagnostics as diagnostic (diagnostic.code)}
            <li class="flex items-start gap-1.5">
              <Icon name="warning-circle" class="mt-0.5 size-3 shrink-0" />
              <span>{diagnostic.code}</span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <Button
      type="button"
      variant="outline"
      class="w-full"
      onclick={() => onToggleCollapsed(!region.collapsed)}
    >
      <Icon name={region.collapsed ? 'caret-down' : 'caret-up'} class="size-3.5" />
      <span>{region.collapsed ? '展开区域' : '折叠区域'}</span>
    </Button>

    <p class="px-1 text-[11px] leading-4 text-ink-subtle">
      区域由 flat Flow graph 即时推导；折叠只保存布局，不改变规则定义。
    </p>
  </div>
{/if}
