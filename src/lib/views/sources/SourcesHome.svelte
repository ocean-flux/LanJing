<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from '$lib/components/Icon.svelte';
  import InstallSource from '$lib/components/InstallSource.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { m } from '$lib/i18n';
  import {
    getError,
    getInstalledSources,
    getLoading,
    loadInstalledSources,
    type StandardIntent,
  } from '$lib/stores/rules.svelte';

  let initialLoadPending = $state(true);
  let retrying = $state(false);
  let installerOpen = $state(false);

  const sources = $derived(getInstalledSources());
  const loadError = $derived(getError());
  const loading = $derived(initialLoadPending || retrying || getLoading());

  const intentLabels: Record<StandardIntent, () => string> = {
    Search: () => m.sources_intent_search(),
    Discover: () => m.sources_intent_discover(),
    ResolveItem: () => m.sources_intent_resolve_item(),
    ListUnits: () => m.sources_intent_list_units(),
    ResolveAsset: () => m.sources_intent_resolve_asset(),
    ContinueAction: () => m.sources_intent_continue_action(),
  };

  onMount(() => {
    void initialize();
  });

  async function initialize(): Promise<void> {
    try {
      await loadInstalledSources();
    } finally {
      initialLoadPending = false;
    }
  }

  async function retryLoad(): Promise<void> {
    retrying = true;
    try {
      await loadInstalledSources();
    } finally {
      retrying = false;
    }
  }
</script>

<section class="mx-auto flex w-full max-w-6xl flex-col gap-6">
  <PageHeader
    title={m.sources_title()}
    action={sources.length > 0 && !loading && !loadError
      ? {
          label: installerOpen ? m.sources_add_close() : m.action_add_source(),
          icon: installerOpen ? 'x' : 'plus',
          pressed: installerOpen,
          onclick: () => (installerOpen = !installerOpen),
        }
      : undefined}
  />

  {#if loading}
    <div class="flex min-h-48 items-center gap-3 text-sm text-ink-muted" role="status">
      <Icon name="arrow-clockwise" class="size-5" />
      <span>{m.sources_loading()}</span>
    </div>
  {:else if loadError}
    <div class="border-danger/35 bg-danger/10 rounded-xl border px-5 py-5" role="alert">
      <div class="flex items-start gap-3">
        <Icon name="warning-circle" class="text-danger mt-0.5 size-5" />
        <div class="min-w-0">
          <h2 class="text-danger font-semibold">{m.sources_load_error()}</h2>
          <p class="mt-1 text-sm break-words text-ink-muted">{loadError}</p>
        </div>
      </div>
      <button
        type="button"
        class="glass-control mt-4 inline-flex min-h-11 items-center gap-2 rounded-lg border border-hairline-strong px-4 text-sm font-semibold text-ink outline-none hover:bg-surface-2 focus-visible:shadow-[var(--focus-ring)]"
        onclick={retryLoad}
      >
        <Icon name="arrow-clockwise" class="size-4" />
        <span>{m.action_retry()}</span>
      </button>
    </div>
  {:else if sources.length === 0}
    <div class="glass-panel rounded-xl border border-hairline px-5 py-5" role="status">
      <h2 class="font-semibold text-ink">{m.sources_empty_title()}</h2>
    </div>
    <InstallSource />
  {:else}
    <section aria-labelledby="installed-sources-title">
      <h2 id="installed-sources-title" class="mb-3 text-sm font-semibold text-ink">
        {m.sources_installed_title()}
      </h2>
      <ul class="glass-panel divide-y divide-hairline rounded-xl border border-hairline">
        {#each sources as source (source.source_id)}
          <li>
            <article class="grid gap-4 px-5 py-4 md:grid-cols-[minmax(0,1fr)_auto]">
              <div class="min-w-0">
                <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                  <h3 class="font-semibold text-ink">{source.profile.title}</h3>
                  <span class="text-xs text-ink-subtle">{source.source_id}</span>
                </div>
                <dl class="mt-3 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-sm">
                  <dt class="text-ink-muted">{m.sources_install_version()}</dt>
                  <dd class="text-ink">{source.version}</dd>
                  <dt class="text-ink-muted">{m.sources_revision()}</dt>
                  <dd class="text-ink">{source.revision}</dd>
                </dl>

                <div class="mt-3">
                  <p class="text-xs font-medium text-ink-muted">{m.sources_supported_intents()}</p>
                  {#if source.profile.supported_intents.length > 0}
                    <ul
                      class="mt-1.5 flex flex-wrap gap-1.5"
                      aria-label={m.sources_supported_intents()}
                    >
                      {#each source.profile.supported_intents as intent (intent)}
                        <li
                          class="rounded-md border border-hairline bg-surface-2 px-2 py-1 text-xs text-ink"
                        >
                          {intentLabels[intent]()}
                        </li>
                      {/each}
                    </ul>
                  {:else}
                    <p class="mt-1 text-xs text-ink-subtle">{m.sources_no_intents()}</p>
                  {/if}
                </div>

                {#if source.profile.risk_notes.length > 0}
                  <div class="mt-3">
                    <p class="text-xs font-medium text-ink-muted">{m.sources_risk_notes()}</p>
                    <ul class="mt-1 list-disc space-y-1 pl-5 text-xs leading-5 text-ink-muted">
                      {#each source.profile.risk_notes as note (note)}
                        <li>{note}</li>
                      {/each}
                    </ul>
                  </div>
                {/if}
              </div>
              <Icon name="check-circle" class="size-5 text-positive" />
            </article>
          </li>
        {/each}
      </ul>
    </section>

    {#if installerOpen}
      <InstallSource />
    {/if}
  {/if}
</section>
