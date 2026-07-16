<script lang="ts">
  import { mediaAppCards } from '$lib/app/demo-state';
  import type { MediaAppCardState } from '$lib/app/shell-types';
  import { m } from '$lib/i18n';
  import MediaAppCard from './MediaAppCard.svelte';

  type Props = {
    apps?: MediaAppCardState[];
  };

  let { apps = mediaAppCards }: Props = $props();
  let selectedPlaceholder = $state<MediaAppCardState | null>(null);
  const leadApp = $derived(apps.find((app) => app.key === 'novel') ?? apps[0]);
  const secondaryApps = $derived(apps.filter((app) => app.key !== leadApp?.key));
</script>

<section class="mx-auto flex max-w-[var(--content-max-width)] flex-col gap-5 py-4">
  <div class="surface-panel p-5 md:p-6">
    <p class="text-sm font-medium text-muted-foreground">{m.nav_apps()}</p>
    <h1 class="mt-2 text-2xl font-semibold tracking-tight md:text-3xl">{m.apps_title()}</h1>
    <p class="mt-2 max-w-2xl text-sm leading-6 text-muted-foreground">{m.apps_desc()}</p>
  </div>

  {#if selectedPlaceholder}
    <div class="surface-control p-4 text-sm" role="status">
      <span class="font-semibold"
        >{m.apps_placeholder_title({ label: selectedPlaceholder.label })}</span
      >
      <p class="mt-1 text-muted-foreground">
        {m.apps_placeholder_desc({ action: selectedPlaceholder.primaryAction })}
      </p>
    </div>
  {/if}

  <div class="grid grid-flow-dense gap-4 lg:grid-cols-3">
    {#if leadApp}
      <MediaAppCard
        app={leadApp}
        lead
        selected={selectedPlaceholder?.key === leadApp.key}
        onselect={(next) => (selectedPlaceholder = next)}
      />
    {/if}

    {#each secondaryApps as app (app.key)}
      <MediaAppCard
        {app}
        selected={selectedPlaceholder?.key === app.key}
        onselect={(next) => (selectedPlaceholder = next)}
      />
    {/each}
  </div>
</section>
