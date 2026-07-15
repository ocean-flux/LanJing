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
    'surface-panel motion-dock-wake p-5 transition-colors',
    source.status === 'failed' && 'border-danger/45 bg-danger/10',
    source.status === 'partial' && 'border-warning/45 bg-warning/10',
  ]}
  data-tone={attention ? 'attention' : 'calm'}
>
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div>
      <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">
        {source.kind}
      </p>
      <h2 class="mt-2 text-xl font-semibold">{source.name}</h2>
    </div>
    <span
      class={['rounded-full border px-3 py-1 text-xs font-semibold', statusTone[source.status]]}
    >
      {statusLabels[source.status]}
    </span>
  </div>

  <p class="mt-4 text-sm leading-6 text-muted-foreground">{source.summary}</p>

  <div class="mt-4 flex flex-wrap gap-2" aria-label={m.sources_capabilities()}>
    {#each capabilities as capability (capability.key)}
      <CapabilityChip
        capability={capability.key}
        enabled={source.capabilities[capability.key] === true}
      />
    {/each}
  </div>

  <dl class="mt-5 grid gap-2 text-sm sm:grid-cols-2">
    {#each source.trustFacts as fact (fact.label)}
      <div class="surface-control rounded-2xl p-3">
        <dt class="text-xs text-muted-foreground">{fact.label}</dt>
        <dd class="mt-1 font-medium">{fact.value}</dd>
      </div>
    {/each}
  </dl>

  <div class="mt-5 flex flex-wrap gap-2">
    {#each source.actions as action (action)}
      <Button type="button" variant="outline" class="rounded-full bg-background/70">
        {action}
      </Button>
    {/each}
  </div>

  {#if source.checkedAt}
    <p class="mt-4 text-xs text-muted-foreground">
      {m.sources_checked_at({ time: source.checkedAt })}
    </p>
  {/if}
</article>
