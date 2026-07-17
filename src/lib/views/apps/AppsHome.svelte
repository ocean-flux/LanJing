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

<!-- 全宽 denselist：无 marketing hero / surface-panel 大卡 -->
<section class="flex w-full flex-col gap-3" aria-label={m.apps_title()}>
  <header class="flex flex-wrap items-baseline justify-between gap-2 border-b border-hairline pb-2">
    <h1 class="text-base font-semibold tracking-tight text-ink">{m.apps_title()}</h1>
    <p class="max-w-prose text-xs text-ink-muted">{m.apps_desc()}</p>
  </header>

  {#if selectedPlaceholder}
    <div class="border border-hairline bg-surface-2 px-3 py-2 text-sm" role="status">
      <span class="font-semibold text-ink"
        >{m.apps_placeholder_title({ label: selectedPlaceholder.label })}</span
      >
      <p class="mt-0.5 text-ink-muted">
        {m.apps_placeholder_desc({ action: selectedPlaceholder.primaryAction })}
      </p>
    </div>
  {/if}

  <div class="grid grid-flow-dense gap-3 sm:grid-cols-2 lg:grid-cols-3">
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
