<script lang="ts">
  import { mediaAppCards } from '$lib/app/demo-state';
  import type { MediaAppCardState } from '$lib/app/shell-types';
  import { m } from '$lib/i18n';
  import MediaAppCard from './MediaAppCard.svelte';

  type Props = {
    apps?: MediaAppCardState[];
  };

  let { apps = mediaAppCards }: Props = $props();
  let selectedKey = $state<MediaAppCardState['key'] | null>(null);
  const selectedPlaceholder = $derived(
    apps.find((app) => app.key === selectedKey && !app.href) ?? null,
  );
  const leadApp = $derived(apps.find((app) => app.key === 'novel') ?? apps[0]);
  const secondaryApps = $derived(apps.filter((app) => app.key !== leadApp?.key));
</script>

<!-- Ethereal 工作台：媒体卡网格；未就绪 → 诚实「媒体体验稍后」 -->
<section class="flex w-full flex-col gap-3" aria-label={m.apps_title()}>
  <header class="flex flex-wrap items-baseline justify-between gap-2 border-b border-hairline pb-2">
    <h1 class="text-base font-semibold tracking-tight text-ink">{m.apps_title()}</h1>
    <p class="max-w-prose text-xs text-ink-muted">{m.apps_desc()}</p>
  </header>

  {#if selectedPlaceholder}
    <div class="double-bezel px-3 py-3 text-sm" role="status" data-testid="apps-media-later">
      <span class="font-semibold text-ink">{m.apps_media_later_title()}</span>
      <p class="mt-1 text-ink-muted">
        {m.apps_media_later_desc({
          label: selectedPlaceholder.label,
          action: selectedPlaceholder.primaryAction,
        })}
      </p>
      <p class="mt-2 text-xs text-ink-subtle">
        {m.apps_placeholder_desc({
          label: selectedPlaceholder.label,
          action: selectedPlaceholder.primaryAction,
        })}
      </p>
    </div>
  {/if}

  <div class="grid grid-flow-dense gap-3 sm:grid-cols-2 lg:grid-cols-3">
    {#if leadApp}
      <MediaAppCard
        app={leadApp}
        lead
        selected={selectedPlaceholder?.key === leadApp.key}
        onselect={(next) => (selectedKey = next.key)}
      />
    {/if}

    {#each secondaryApps as app (app.key)}
      <MediaAppCard
        {app}
        selected={selectedPlaceholder?.key === app.key}
        onselect={(next) => (selectedKey = next.key)}
      />
    {/each}
  </div>
</section>
