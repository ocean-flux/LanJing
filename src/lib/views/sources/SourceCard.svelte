<script lang="ts">
  import { capabilities } from '$lib/brand';
  import { CapabilityChip } from '$lib/components/brand';
  import { Button } from '$lib/components/ui/button';
  import { m } from '$lib/i18n';
  import type { SourceCardState } from '$lib/app/shell-types';

  type Props = {
    source: SourceCardState;
    attention?: boolean;
  };

  const statusTone: Record<SourceCardState['status'], string> = {
    ready: 'border-positive/40 bg-positive/10 text-positive',
    partial: 'border-warning/40 bg-warning/10 text-warning',
    failed: 'border-danger/40 bg-danger/10 text-danger',
    disabled: 'border-border/70 bg-muted/60 text-muted-foreground',
    unchecked: 'border-border/70 bg-background/70 text-muted-foreground',
  };

  const statusLabels: Record<SourceCardState['status'], string> = {
    ready: m.status_ready(),
    partial: m.status_partial(),
    failed: m.status_failed(),
    disabled: m.status_disabled(),
    unchecked: m.status_unchecked(),
  };

  let { source, attention = false }: Props = $props();
</script>

<article
  class={[
    'motion-dock-wake rounded-xl border border-hairline bg-surface-1 p-3 transition-colors',
    source.status === 'failed' && 'border-danger/45 bg-danger/10',
    source.status === 'partial' && 'border-warning/45 bg-warning/10',
  ]}
  data-tone={attention ? 'attention' : 'calm'}
>
  <div class="flex flex-wrap items-start justify-between gap-2">
    <div class="min-w-0">
      <p class="text-[0.68rem] font-medium uppercase tracking-wide text-ink-subtle">
        {source.kind}
      </p>
      <h2 class="mt-0.5 text-sm font-semibold text-ink">{source.name}</h2>
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

  <div class="mt-2 flex flex-wrap gap-1.5" aria-label={m.sources_capabilities()}>
    {#each capabilities as capability (capability.key)}
      <CapabilityChip
        capability={capability.key}
        enabled={source.capabilities[capability.key] === true}
      />
    {/each}
  </div>

  <dl class="mt-2 grid gap-1.5 text-xs sm:grid-cols-2">
    {#each source.trustFacts as fact (fact.label)}
      <div class="rounded-lg border border-hairline bg-surface-2 px-2 py-1.5">
        <dt class="text-[0.68rem] text-ink-subtle">{fact.label}</dt>
        <dd class="mt-0.5 font-medium text-ink">{fact.value}</dd>
      </div>
    {/each}
  </dl>

  <div class="mt-2 flex flex-wrap gap-1.5">
    {#each source.actions as action (action)}
      <Button type="button" variant="outline" size="sm" class="h-8 rounded-lg text-xs">
        {action}
      </Button>
    {/each}
  </div>

  {#if source.checkedAt}
    <p class="mt-2 text-[0.68rem] text-ink-subtle">
      {m.sources_checked_at({ time: source.checkedAt })}
    </p>
  {/if}
</article>
