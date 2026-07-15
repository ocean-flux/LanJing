<script lang="ts">
  import { onMount } from 'svelte';
  import { Download, Pin, Plus, Search, Star } from '@lucide/svelte';
  import { resolve } from '$app/paths';
  import { m } from '$lib/i18n';
  import { loadLibraryProjection, updateLibraryEntry } from './library-api';
  import {
    projectLibrary,
    type LibraryEntry,
    type LibraryProjectionResponse,
  } from './library-projection';

  type ActionCard = {
    kind: 'add-source' | 'import-local' | 'search-content';
    path: '/' | '/apps' | '/sources';
    label: string;
    desc: string;
  };

  type Props = {
    projection?: LibraryProjectionResponse | null;
    load?: () => Promise<LibraryProjectionResponse>;
    update?: (entry: LibraryEntry) => Promise<void>;
  };

  let {
    projection = null,
    load = loadLibraryProjection,
    update = updateLibraryEntry,
  }: Props = $props();
  let activeProjection = $state<LibraryProjectionResponse | null>(null);
  let loading = $state(false);
  let loadError = $state(false);
  let stateError = $state(false);
  const items = $derived(projectLibrary(activeProjection));

  const actions: ActionCard[] = [
    {
      kind: 'add-source',
      path: '/sources',
      label: m.action_add_source(),
      desc: m.library_add_source_desc(),
    },
    {
      kind: 'import-local',
      path: '/sources',
      label: m.action_import_local(),
      desc: m.library_import_desc(),
    },
    {
      kind: 'search-content',
      path: '/apps',
      label: m.action_search_content(),
      desc: m.library_search_desc(),
    },
  ];

  onMount(() => {
    if (projection) {
      activeProjection = projection;
      return;
    }
    loading = true;
    load()
      .then((next) => {
        activeProjection = next;
      })
      .catch(() => {
        loadError = true;
      })
      .finally(() => {
        loading = false;
      });
  });

  async function toggleState(itemIndex: number, key: 'favorite' | 'pinned'): Promise<void> {
    const item = items[itemIndex];
    if (!item) return;
    const nextEntry: LibraryEntry = {
      ...item.state,
      [key]: !item.state[key],
    };

    try {
      await update(nextEntry);
      if (!activeProjection) return;
      activeProjection = {
        ...activeProjection,
        entries: activeProjection.entries.map((entry) =>
          entry.resource_id === nextEntry.resource_id ? nextEntry : entry,
        ),
      };
    } catch {
      stateError = true;
    }
  }
</script>

<section class="grid min-h-full gap-4 py-4 lg:grid-cols-[minmax(0,1fr)_360px]">
  <div class="surface-panel media-void relative min-h-[560px] overflow-hidden p-5 md:p-8">
    <div class="relative flex min-h-[500px] flex-col justify-between gap-10">
      <div class="max-w-5xl py-8 md:py-12">
        <p class="text-sm font-semibold text-muted-foreground">{m.nav_library()}</p>
        <h1
          class="mt-4 max-w-4xl text-balance text-5xl font-semibold tracking-[-0.055em] text-foreground md:text-7xl xl:text-8xl"
        >
          {m.library_title()}
        </h1>
        <p class="mt-6 max-w-2xl text-base leading-7 text-muted-foreground md:text-lg">
          {m.library_desc()}
        </p>
      </div>

      {#if loading}
        <p class="text-sm text-muted-foreground" role="status">{m.library_desc()}</p>
      {:else if items.length > 0}
        <div class="grid gap-3" aria-label={m.library_title()}>
          {#each items as entry, index (entry.item.id)}
            <article
              class="surface-control grid gap-4 rounded-xl border border-border bg-background/55 p-4 md:grid-cols-[minmax(0,1fr)_auto]"
              data-resource-id={entry.item.id}
            >
              <div class="min-w-0">
                <h2 class="truncate text-base font-semibold text-foreground">{entry.item.title}</h2>
                {#if entry.item.subtitle}
                  <p class="mt-1 text-sm text-muted-foreground">{entry.item.subtitle}</p>
                {/if}
                {#if entry.source}
                  <p class="mt-2 text-xs text-muted-foreground">
                    {m.library_source_label({ name: entry.source.title })}
                  </p>
                {/if}
                {#if entry.state.progress}
                  <p class="mt-2 text-xs text-muted-foreground">
                    {entry.state.progress.position}{#if entry.state.progress.total !== null}
                      / {entry.state.progress.total}
                    {/if}
                  </p>
                {/if}
                {#if entry.alternativeRoutes.length > 0}
                  <p class="mt-2 text-xs text-muted-foreground">
                    {m.library_alternative_routes()}: {entry.alternativeRoutes
                      .map((route) => route.title)
                      .join('、')}
                  </p>
                {/if}
              </div>
              <div class="flex items-start gap-2">
                <button
                  type="button"
                  class="grid h-9 w-9 place-items-center rounded-md border border-border hover:bg-accent"
                  aria-label={entry.state.favorite ? m.library_unfavorite() : m.library_favorite()}
                  aria-pressed={entry.state.favorite}
                  onclick={() => toggleState(index, 'favorite')}
                >
                  <Star
                    size={16}
                    fill={entry.state.favorite ? 'currentColor' : 'none'}
                    aria-hidden="true"
                  />
                </button>
                <button
                  type="button"
                  class="grid h-9 w-9 place-items-center rounded-md border border-border hover:bg-accent"
                  aria-label={entry.state.pinned ? m.library_unpin() : m.library_pin()}
                  aria-pressed={entry.state.pinned}
                  onclick={() => toggleState(index, 'pinned')}
                >
                  <Pin
                    size={16}
                    fill={entry.state.pinned ? 'currentColor' : 'none'}
                    aria-hidden="true"
                  />
                </button>
              </div>
            </article>
          {/each}
        </div>
      {:else}
        <div
          class="grid gap-3 rounded-xl border border-dashed border-border bg-background/30 p-5"
          role="status"
        >
          <h2 class="text-base font-semibold text-foreground">{m.library_empty_title()}</h2>
          <p class="text-sm leading-6 text-muted-foreground">{m.library_empty_next()}</p>
        </div>
      {/if}

      {#if loadError}
        <p class="text-xs text-muted-foreground" role="status">{m.library_empty_next()}</p>
      {/if}
      {#if stateError}
        <p class="text-xs text-destructive" role="alert">{m.library_empty_next()}</p>
      {/if}
    </div>
  </div>

  <aside class="grid content-start gap-4">
    <div class="surface-panel p-5">
      <h2 class="text-sm font-semibold">{m.library_title()}</h2>
      <p class="mt-2 text-xs leading-5 text-muted-foreground">{m.library_desc()}</p>

      <div class="mt-5 grid gap-2">
        {#each actions as action (action.kind)}
          <a
            href={resolve(action.path as '/')}
            class="surface-control flex items-start gap-3 rounded-xl px-3 py-2 text-sm hover:bg-accent"
          >
            <span
              class="grid h-8 w-8 shrink-0 place-items-center rounded-lg border border-border bg-surface-1 text-foreground"
            >
              {#if action.kind === 'add-source'}
                <Plus size={15} aria-hidden="true" />
              {:else if action.kind === 'import-local'}
                <Download size={15} aria-hidden="true" />
              {:else}
                <Search size={15} aria-hidden="true" />
              {/if}
            </span>
            <span class="min-w-0">
              <span class="block font-medium text-foreground">{action.label}</span>
              <span class="mt-1 block text-xs leading-5 text-muted-foreground">{action.desc}</span>
            </span>
          </a>
        {/each}
      </div>
    </div>
  </aside>
</section>
