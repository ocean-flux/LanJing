<script lang="ts">
  import { demoSources } from '$lib/app/demo-state';
  import type { SourceCardState } from '$lib/app/shell-types';
  import { m } from '$lib/i18n';
  import AddSourcePanel from './AddSourcePanel.svelte';
  import SourceCard from './SourceCard.svelte';

  type Props = {
    sources?: SourceCardState[];
  };

  const order: Record<SourceCardState['status'], number> = {
    failed: 0,
    partial: 1,
    ready: 2,
    unchecked: 3,
    disabled: 4,
  };

  let { sources = demoSources }: Props = $props();
  const sortedSources = $derived([...sources].sort((a, b) => order[a.status] - order[b.status]));
  const failedSources = $derived(sortedSources.filter((s) => s.status === 'failed'));
  const partialSources = $derived(sortedSources.filter((s) => s.status === 'partial'));
  const readySources = $derived(sortedSources.filter((s) => s.status === 'ready'));
  const uncheckedSources = $derived(sortedSources.filter((s) => s.status === 'unchecked'));
  const disabledSources = $derived(sortedSources.filter((s) => s.status === 'disabled'));
</script>

<section class="mx-auto flex max-w-[var(--content-max-width)] flex-col gap-5 py-4">
  <div class="surface-panel p-6">
    <p class="text-sm font-semibold text-muted-foreground">{m.nav_sources()}</p>
    <h1 class="mt-3 text-3xl font-semibold tracking-tight md:text-5xl">{m.sources_title()}</h1>
    <p class="mt-3 max-w-2xl text-sm leading-6 text-muted-foreground">{m.sources_desc()}</p>
  </div>

  <AddSourcePanel />

  {#if sortedSources.length === 0}
    <section
      class="surface-panel border-dashed p-8 text-center"
      aria-labelledby="sources-empty-title"
    >
      <h2 id="sources-empty-title" class="text-xl font-semibold">{m.sources_empty_title()}</h2>
      <p class="mt-2 text-sm text-muted-foreground">{m.sources_empty_desc()}</p>
    </section>
  {:else}
    {#if failedSources.length > 0}
      <section class="grid gap-4" aria-label={m.status_failed()}>
        {#each failedSources as source (source.id)}
          <SourceCard {source} attention />
        {/each}
      </section>
    {/if}

    {#if partialSources.length > 0}
      <section class="grid gap-4" aria-label={m.status_partial()}>
        {#each partialSources as source (source.id)}
          <SourceCard {source} attention />
        {/each}
      </section>
    {/if}

    {#if readySources.length > 0}
      <section class="grid gap-4 lg:grid-cols-2" aria-label={m.status_ready()}>
        {#each readySources as source (source.id)}
          <SourceCard {source} />
        {/each}
      </section>
    {/if}

    {#if uncheckedSources.length > 0}
      <section class="grid gap-4 lg:grid-cols-2" aria-label={m.status_unchecked()}>
        {#each uncheckedSources as source (source.id)}
          <SourceCard {source} />
        {/each}
      </section>
    {/if}

    {#if disabledSources.length > 0}
      <section class="grid gap-4 lg:grid-cols-2 opacity-70" aria-label={m.status_disabled()}>
        {#each disabledSources as source (source.id)}
          <SourceCard {source} />
        {/each}
      </section>
    {/if}
  {/if}
</section>
