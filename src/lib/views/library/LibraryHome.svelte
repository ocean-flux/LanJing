<script lang="ts">
  import { onMount } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import Download from '@lucide/svelte/icons/download';
  import Pin from '@lucide/svelte/icons/pin';
  import Plus from '@lucide/svelte/icons/plus';
  import Search from '@lucide/svelte/icons/search';
  import Star from '@lucide/svelte/icons/star';
  import { resolve } from '$app/paths';
  import { m } from '$lib/i18n';
  import {
    loadLibraryProjection,
    updateLibraryEntry,
    type LibraryUpdateReceipt,
  } from './library-api';
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
    update?: (entry: LibraryEntry) => Promise<LibraryUpdateReceipt>;
  };

  type LibraryViewState =
    | { kind: 'loading' }
    | { kind: 'load-error' }
    | { kind: 'empty'; projection: LibraryProjectionResponse }
    | { kind: 'ready'; projection: LibraryProjectionResponse };

  let {
    projection = null,
    load = loadLibraryProjection,
    update = updateLibraryEntry,
  }: Props = $props();

  function stateFromProjection(next: LibraryProjectionResponse): LibraryViewState {
    return projectLibrary(next).length === 0
      ? { kind: 'empty', projection: next }
      : { kind: 'ready', projection: next };
  }

  let viewState = $derived<LibraryViewState>(
    projection ? stateFromProjection(projection) : { kind: 'loading' },
  );
  let updateError = $state(false);
  let pendingResourceIds = new SvelteSet<string>();
  const items = $derived(viewState.kind === 'ready' ? projectLibrary(viewState.projection) : []);

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
    if (!projection) void loadProjection();
  });

  async function loadProjection(): Promise<void> {
    viewState = { kind: 'loading' };
    try {
      viewState = stateFromProjection(await load());
    } catch {
      viewState = { kind: 'load-error' };
    }
  }

  async function toggleState(resourceId: string, key: 'favorite' | 'pinned'): Promise<void> {
    if (pendingResourceIds.has(resourceId)) return;

    const item = items.find((candidate) => candidate.resource_id === resourceId);
    if (!item) return;

    const nextEntry: LibraryEntry = {
      ...item.state,
      [key]: !item.state[key],
    };

    updateError = false;
    pendingResourceIds.add(resourceId);
    try {
      const receipt = await update(nextEntry);
      if (viewState.kind !== 'ready') return;

      const nextProjection: LibraryProjectionResponse = {
        ...viewState.projection,
        global_seq: receipt.global_seq,
        entries: viewState.projection.entries.map((entry) =>
          entry.resource_id === resourceId
            ? {
                ...nextEntry,
                revision: receipt.revision,
                updated_global_seq: receipt.global_seq,
              }
            : entry,
        ),
      };
      viewState = stateFromProjection(nextProjection);
      updateError = false;
    } catch {
      updateError = true;
    } finally {
      pendingResourceIds.delete(resourceId);
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

    {#if viewState.kind === 'loading'}
      <p class="text-sm text-ink-muted" role="status">{m.library_loading()}</p>
    {:else if viewState.kind === 'load-error'}
      <div class="media-void rounded-xl px-3 py-4" role="alert">
        <p class="text-sm font-medium text-danger">{m.library_load_error()}</p>
        <button
          type="button"
          class="mt-3 inline-flex min-h-11 items-center rounded-lg border border-hairline-strong px-4 text-sm font-semibold text-ink outline-none hover:bg-lantern-soft focus-visible:shadow-[var(--focus-ring)]"
          onclick={loadProjection}
        >
          {m.library_retry()}
        </button>
      </div>
    {:else if viewState.kind === 'ready'}
      <ul class="double-bezel divide-y divide-hairline" aria-label={m.library_title()}>
        {#each items as entry (entry.resource_id)}
          <li>
            <article
              class="grid gap-3 px-3 py-2 md:grid-cols-[minmax(0,1fr)_auto]"
              data-resource-id={entry.resource_id}
            >
              <div class="min-w-0">
                <h2 class="truncate text-sm font-semibold text-ink">{entry.resource_id}</h2>
                {#if entry.state.progress}
                  <p class="mt-1 text-xs text-ink-subtle">
                    {entry.state.progress.position}{#if entry.state.progress.total !== null}
                      / {entry.state.progress.total}
                    {/if}
                  </p>
                {/if}
              </div>
              <div
                class="flex items-start gap-1"
                role="group"
                aria-label={entry.resource_id}
                aria-busy={pendingResourceIds.has(entry.resource_id)}
              >
                <button
                  type="button"
                  class="grid h-11 w-11 place-items-center rounded-md text-ink-muted outline-none hover:bg-lantern-soft hover:text-ink focus-visible:shadow-[var(--focus-ring)] disabled:cursor-wait disabled:opacity-60"
                  aria-label={entry.state.favorite ? m.library_unfavorite() : m.library_favorite()}
                  aria-pressed={entry.state.favorite}
                  aria-busy={pendingResourceIds.has(entry.resource_id)}
                  disabled={pendingResourceIds.has(entry.resource_id)}
                  onclick={() => toggleState(entry.resource_id, 'favorite')}
                >
                  <Star
                    size={16}
                    fill={entry.state.favorite ? 'currentColor' : 'none'}
                    aria-hidden="true"
                  />
                </button>
                <button
                  type="button"
                  class="grid h-11 w-11 place-items-center rounded-md text-ink-muted outline-none hover:bg-lantern-soft hover:text-ink focus-visible:shadow-[var(--focus-ring)] disabled:cursor-wait disabled:opacity-60"
                  aria-label={entry.state.pinned ? m.library_unpin() : m.library_pin()}
                  aria-pressed={entry.state.pinned}
                  aria-busy={pendingResourceIds.has(entry.resource_id)}
                  disabled={pendingResourceIds.has(entry.resource_id)}
                  onclick={() => toggleState(entry.resource_id, 'pinned')}
                >
                  <Pin
                    size={16}
                    fill={entry.state.pinned ? 'currentColor' : 'none'}
                    aria-hidden="true"
                  />
                </button>
              </div>
            </article>
          </li>
        {/each}
      </ul>
    {:else}
      <div class="media-void rounded-xl px-3 py-4" role="status" data-testid="library-empty">
        <h2 class="text-sm font-semibold text-ink">{m.library_empty_title()}</h2>
        <p class="mt-1 text-xs leading-5 text-ink-muted">{m.library_empty_next()}</p>
      </div>
    {/if}

    {#if updateError}
      <p class="mt-2 text-xs text-danger" role="alert">{m.library_update_error()}</p>
    {/if}
  </div>

  <aside class="double-bezel p-3 lg:self-start">
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
