<script lang="ts">
  import { onMount } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import Icon from '$lib/components/Icon.svelte';
  import { resolve } from '$app/paths';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { Skeleton } from '$lib/components/ui/skeleton';
  import { m } from '$lib/i18n';
  import type { MediaItem } from '$lib/views/media/media-api';
  import {
    loadLibraryProjection,
    updateLibraryEntry,
    type LibraryUpdateReceipt,
  } from './library-api';
  import LibraryEntryRow from './LibraryEntryRow.svelte';
  import {
    enrichLibraryItems,
    loadMediaByResourceIds,
    mediaItemsToMap,
    type LibraryEntryRowModel,
    type LoadMediaItems,
  } from './library-media';
  import {
    projectLibrary,
    type LibraryEntry,
    type LibraryProjectionResponse,
  } from './library-projection';

  type Props = {
    projection?: LibraryProjectionResponse | null;
    /** 同步注入媒体（测试）；缺省则按 resource_id 分批 loadMedia。 */
    mediaItems?: MediaItem[] | null;
    load?: () => Promise<LibraryProjectionResponse>;
    loadMedia?: LoadMediaItems;
    update?: (entry: LibraryEntry) => Promise<LibraryUpdateReceipt>;
  };

  type LibraryViewState =
    | { kind: 'loading' }
    | { kind: 'load-error' }
    | { kind: 'empty'; projection: LibraryProjectionResponse }
    | {
        kind: 'ready';
        projection: LibraryProjectionResponse;
        rows: LibraryEntryRowModel[];
      };

  let {
    projection = null,
    mediaItems = null,
    load = loadLibraryProjection,
    loadMedia = undefined,
    update = updateLibraryEntry,
  }: Props = $props();

  function rowsFromProjection(
    next: LibraryProjectionResponse,
    mediaById: ReadonlyMap<string, MediaItem>,
  ): LibraryViewState {
    const projected = projectLibrary(next);
    if (projected.length === 0) {
      return { kind: 'empty', projection: next };
    }
    return {
      kind: 'ready',
      projection: next,
      rows: enrichLibraryItems(projected, mediaById),
    };
  }

  function initialState(): LibraryViewState {
    if (!projection) return { kind: 'loading' };
    const mediaById = mediaItems ? mediaItemsToMap(mediaItems) : new Map<string, MediaItem>();
    return rowsFromProjection(projection, mediaById);
  }

  let viewState = $state<LibraryViewState>(initialState());
  let updateError = $state(false);
  let pendingResourceIds = new SvelteSet<string>();
  const rows = $derived(viewState.kind === 'ready' ? viewState.rows : []);

  onMount(() => {
    if (!projection) {
      void loadProjection();
      return;
    }
    // 有同步 projection 但未注入 media：后台补齐 enrichment。
    if (mediaItems === null && viewState.kind === 'ready') {
      void enrichReadyProjection(viewState.projection);
    }
  });

  async function enrichReadyProjection(next: LibraryProjectionResponse): Promise<void> {
    const projected = projectLibrary(next);
    if (projected.length === 0) {
      viewState = { kind: 'empty', projection: next };
      return;
    }

    let mediaById: Map<string, MediaItem>;
    try {
      mediaById = await loadMediaByResourceIds(
        projected.map((item) => item.resource_id),
        loadMedia,
      );
    } catch {
      // 媒体批失败：整表仍可用，行内诚实降级。
      mediaById = new Map();
    }

    if (viewState.kind !== 'ready') return;
    viewState = rowsFromProjection(viewState.projection, mediaById);
  }

  async function loadProjection(): Promise<void> {
    viewState = { kind: 'loading' };
    try {
      const next = await load();
      const projected = projectLibrary(next);
      if (projected.length === 0) {
        viewState = { kind: 'empty', projection: next };
        return;
      }

      let mediaById: Map<string, MediaItem>;
      try {
        mediaById = await loadMediaByResourceIds(
          projected.map((item) => item.resource_id),
          loadMedia,
        );
      } catch {
        mediaById = new Map();
      }

      viewState = rowsFromProjection(next, mediaById);
    } catch {
      viewState = { kind: 'load-error' };
    }
  }

  async function toggleState(resourceId: string, key: 'favorite' | 'pinned'): Promise<void> {
    if (pendingResourceIds.has(resourceId)) return;
    if (viewState.kind !== 'ready') return;

    const item = viewState.rows.find((candidate) => candidate.resource_id === resourceId);
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

      const mediaById = new Map(
        viewState.rows
          .filter((row) => row.media)
          .map((row) => [row.resource_id, row.media as MediaItem]),
      );
      viewState = rowsFromProjection(nextProjection, mediaById);
      updateError = false;
    } catch {
      updateError = true;
    } finally {
      pendingResourceIds.delete(resourceId);
    }
  }
</script>

<section class="mx-auto flex w-full max-w-6xl flex-col gap-6">
  <PageHeader title={m.library_title()} />

  {#if viewState.kind === 'loading'}
    <ul
      class="glass-panel divide-y divide-hairline rounded-xl border border-hairline"
      aria-busy="true"
      aria-label={m.library_loading()}
      role="status"
      data-testid="library-loading"
    >
      {#each [0, 1, 2] as skeleton (skeleton)}
        <li class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 px-4 py-3 sm:px-5">
          <Skeleton class="size-11 rounded-md sm:size-12" />
          <div class="min-w-0 space-y-2">
            <Skeleton class="h-4 w-2/3 max-w-48" />
            <Skeleton class="h-3 w-1/3 max-w-24" />
          </div>
          <div class="flex gap-1">
            <Skeleton class="size-11 rounded-md" />
            <Skeleton class="size-11 rounded-md" />
          </div>
        </li>
      {/each}
    </ul>
  {:else if viewState.kind === 'load-error'}
    <div class="border-danger/35 bg-danger/10 rounded-xl border px-5 py-5" role="alert">
      <p class="text-danger text-sm font-medium">{m.library_load_error()}</p>
      <button
        type="button"
        class="glass-control mt-4 inline-flex min-h-11 items-center gap-2 rounded-lg border border-hairline-strong px-4 text-sm font-semibold text-ink outline-none hover:bg-surface-2 focus-visible:shadow-[var(--focus-ring)]"
        onclick={loadProjection}
      >
        <Icon name="arrow-clockwise" class="size-4" />
        <span>{m.library_retry()}</span>
      </button>
    </div>
  {:else if viewState.kind === 'ready'}
    <ul
      class="glass-panel divide-y divide-hairline rounded-xl border border-hairline"
      aria-label={m.library_title()}
    >
      {#each rows as entry (entry.resource_id)}
        <li>
          <LibraryEntryRow
            {entry}
            pending={pendingResourceIds.has(entry.resource_id)}
            onToggleFavorite={() => toggleState(entry.resource_id, 'favorite')}
            onTogglePinned={() => toggleState(entry.resource_id, 'pinned')}
          />
        </li>
      {/each}
    </ul>
  {:else}
    <div
      class="glass-panel rounded-xl border border-hairline px-5 py-5"
      role="status"
      data-testid="library-empty"
    >
      <h2 class="text-sm font-semibold text-ink">{m.library_empty_title()}</h2>
      <a
        href={resolve('/sources' as '/')}
        class="glass-control mt-4 inline-flex min-h-11 items-center gap-2 rounded-lg border border-hairline-strong px-4 text-sm font-semibold text-ink outline-none hover:bg-surface-2 focus-visible:shadow-[var(--focus-ring)]"
      >
        <Icon name="arrow-right" class="size-4" />
        <span>{m.action_manage_sources()}</span>
      </a>
    </div>
  {/if}

  {#if updateError}
    <p class="text-danger text-sm" role="alert">{m.library_update_error()}</p>
  {/if}
</section>
