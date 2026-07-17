<script lang="ts">
  import { onMount } from 'svelte';
  import Download from '@lucide/svelte/icons/download';
  import Pin from '@lucide/svelte/icons/pin';
  import Plus from '@lucide/svelte/icons/plus';
  import Search from '@lucide/svelte/icons/search';
  import Star from '@lucide/svelte/icons/star';
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

<section class="flex w-full flex-col gap-3 lg:grid lg:grid-cols-[minmax(0,1fr)_240px] lg:gap-4">
  <div class="min-w-0">
    <header
      class="mb-2 flex flex-wrap items-baseline justify-between gap-2 border-b border-hairline pb-2"
    >
      <h1 class="text-base font-semibold tracking-tight text-ink">{m.library_title()}</h1>
      <p class="max-w-prose text-xs text-ink-muted">{m.library_desc()}</p>
    </header>

    {#if loading}
      <p class="text-sm text-ink-muted" role="status">{m.library_desc()}</p>
    {:else if items.length > 0}
      <div class="grid gap-2" aria-label={m.library_title()}>
        {#each items as entry, index (entry.item.id)}
          <article
            class="grid gap-3 border-b border-hairline py-2 md:grid-cols-[minmax(0,1fr)_auto]"
            data-resource-id={entry.item.id}
          >
            <div class="min-w-0">
              <h2 class="truncate text-sm font-semibold text-ink">{entry.item.title}</h2>
              {#if entry.item.subtitle}
                <p class="mt-0.5 text-xs text-ink-muted">{entry.item.subtitle}</p>
              {/if}
              {#if entry.source}
                <p class="mt-1 text-xs text-ink-subtle">
                  {m.library_source_label({ name: entry.source.title })}
                </p>
              {/if}
              {#if entry.state.progress}
                <p class="mt-1 text-xs text-ink-subtle">
                  {entry.state.progress.position}{#if entry.state.progress.total !== null}
                    / {entry.state.progress.total}
                  {/if}
                </p>
              {/if}
              {#if entry.alternativeRoutes.length > 0}
                <p class="mt-1 text-xs text-ink-subtle">
                  {m.library_alternative_routes()}: {entry.alternativeRoutes
                    .map((route) => route.title)
                    .join('、')}
                </p>
              {/if}
            </div>
            <div class="flex items-start gap-1">
              <button
                type="button"
                class="grid h-9 w-9 place-items-center rounded-md text-ink-muted hover:bg-lantern-soft hover:text-ink"
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
                class="grid h-9 w-9 place-items-center rounded-md text-ink-muted hover:bg-lantern-soft hover:text-ink"
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
      <div class="border border-dashed border-hairline px-3 py-4" role="status">
        <h2 class="text-sm font-semibold text-ink">{m.library_empty_title()}</h2>
        <p class="mt-1 text-xs leading-5 text-ink-muted">{m.library_empty_next()}</p>
      </div>
    {/if}

    {#if loadError}
      <p class="mt-2 text-xs text-ink-muted" role="status">{m.library_empty_next()}</p>
    {/if}
    {#if stateError}
      <p class="mt-2 text-xs text-danger" role="alert">{m.library_empty_next()}</p>
    {/if}
  </div>

  <aside class="border-t border-hairline pt-2 lg:border-l lg:border-t-0 lg:pl-4 lg:pt-0">
    <p class="mb-2 text-xs font-medium text-ink-muted">{m.library_title()}</p>
    <div class="grid gap-1">
      {#each actions as action (action.kind)}
        <a
          href={resolve(action.path as '/')}
          class="flex items-start gap-2 rounded-lg px-2 py-2 text-sm text-ink-muted outline-none hover:bg-lantern-soft hover:text-ink focus-visible:shadow-[var(--focus-ring)]"
        >
          <span class="grid h-8 w-8 shrink-0 place-items-center text-ink-muted">
            {#if action.kind === 'add-source'}
              <Plus size={15} aria-hidden="true" />
            {:else if action.kind === 'import-local'}
              <Download size={15} aria-hidden="true" />
            {:else}
              <Search size={15} aria-hidden="true" />
            {/if}
          </span>
          <span class="min-w-0">
            <span class="block font-medium text-ink">{action.label}</span>
            <span class="mt-0.5 block text-xs leading-5 text-ink-subtle">{action.desc}</span>
          </span>
        </a>
      {/each}
    </div>
  </aside>
</section>
