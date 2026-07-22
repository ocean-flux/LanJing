<script lang="ts">
  import { capabilities } from '$lib/brand';
  import { CapabilityChip } from '$lib/components/brand';
  import { Button } from '$lib/components/ui/button';
  import { m } from '$lib/i18n';
  import type { SourceCardAction, SourceCardState } from '$lib/app/shell-types';

  type Props = {
    source: SourceCardState;
    attention?: boolean;
    onaction?: (action: SourceCardAction) => void;
  };

  const statusTone: Record<SourceCardState['status'], string> = {
    ready: 'border-positive/40 bg-positive/10 text-positive',
    partial: 'border-warning/40 bg-warning/10 text-warning',
    failed: 'border-danger/40 bg-danger/10 text-danger',
    disabled: 'border-hairline bg-surface-2 text-ink-muted',
    unchecked: 'border-hairline bg-surface-2 text-ink-muted',
  };

  const statusLabels: Record<SourceCardState['status'], string> = {
    ready: m.status_ready(),
    partial: m.status_partial(),
    failed: m.status_failed(),
    disabled: m.status_disabled(),
    unchecked: m.status_unchecked(),
  };

  let { source, attention = false, onaction }: Props = $props();
</script>

<!-- Ethereal 来源行卡：double-bezel + 语义 status；操作面 lantern hover，无橙紫 -->
<article
  class={[
    'motion-dock-wake double-bezel p-3 transition-colors',
    source.status === 'failed' && 'border-danger/45 bg-danger/5',
    source.status === 'partial' && 'border-warning/40 bg-warning/5',
    attention && source.status !== 'failed' && source.status !== 'partial' && 'border-lantern/35',
  ]}
  data-tone={attention ? 'attention' : 'calm'}
  data-status={source.status}
>
  <div class="flex flex-wrap items-start justify-between gap-2">
    <div class="min-w-0">
      <p class="text-[0.68rem] font-medium uppercase tracking-wide text-ink-subtle">
        {source.kind}
      </p>
      <h2 class="mt-0.5 text-sm font-semibold tracking-tight text-ink">{source.name}</h2>
    </div>
    <span
      class={[
        'rounded-full border px-2 py-0.5 text-[0.68rem] font-medium',
        statusTone[source.status],
      ]}
    >
      {statusLabels[source.status]}
    </span>
  </div>

  <p class="mt-2 text-xs leading-5 text-ink-muted">{source.summary}</p>

  <div class="mt-2 flex flex-wrap gap-1.5" role="group" aria-label={m.sources_capabilities()}>
    {#each capabilities as capability (capability.key)}
      <CapabilityChip
        capability={capability.key}
        enabled={source.capabilities[capability.key] === true}
      />
    {/each}
  </div>

  <dl class="mt-2 grid gap-1.5 text-xs sm:grid-cols-2">
    {#each source.trustFacts as fact (fact.label)}
      <div
        class="rounded-lg border border-hairline bg-surface-2/80 px-2 py-1.5 shadow-[inset_0_1px_0_color-mix(in_oklab,white_4%,transparent)]"
      >
        <dt class="text-[0.68rem] text-ink-subtle">{fact.label}</dt>
        <dd class="mt-0.5 font-medium text-ink">{fact.value}</dd>
      </div>
    {/each}
  </dl>

  {#if onaction && source.actions.length > 0}
    <div class="mt-2.5 flex flex-wrap gap-1.5 border-t border-hairline pt-2.5" role="group">
      {#each source.actions as action (action)}
        <Button
          type="button"
          variant="outline"
          size="sm"
          class="h-auto min-h-11 rounded-lg px-3 text-xs"
          onclick={() => onaction({ sourceId: source.id, action })}
        >
          {action}
        </Button>
      {/each}
    </div>
  {/if}

  {#if source.checkedAt}
    <p class="mt-2 text-[0.68rem] text-ink-subtle">
      {m.sources_checked_at({ time: source.checkedAt })}
    </p>
  {/if}
</article>
