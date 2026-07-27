<script lang="ts">
  import { onMount } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import { resolve } from '$app/paths';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import Notice from '$lib/components/Notice.svelte';
  import PageFrame from '$lib/components/PageFrame.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button/index.js';
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

<PageFrame width="standard">
  <PageHeader title={m.library_title()} />

  {#if viewState.kind === 'loading'}
    <ul
      class="glass-panel divide-y divide-hairline overflow-hidden rounded-[var(--radius-panel)] border border-hairline"
      aria-busy="true"
      aria-label={m.library_loading()}
      role="status"
      data-testid="library-loading"
    >
      {#each [0, 1, 2] as skeleton (skeleton)}
        <li
          class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2 px-(--density-panel-padding-compact) py-2.5 sm:gap-3 sm:py-3"
        >
          <div class="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-center gap-2.5 sm:gap-3">
            <Skeleton class="size-10 shrink-0 rounded-md sm:size-11" />
            <div class="min-w-0 space-y-2">
              <Skeleton class="h-4 w-2/3 max-w-48" />
              <Skeleton class="h-3 w-1/3 max-w-24" />
            </div>
          </div>
          <div class="flex shrink-0 gap-1">
            <Skeleton class="size-(--density-control-md) rounded-md" />
            <Skeleton class="size-(--density-control-md) rounded-md" />
          </div>
        </li>
      {/each}
    </ul>
  {:else if viewState.kind === 'load-error'}
    <Notice tone="danger" role="alert" icon="warning-circle">
      {m.library_load_error()}
      {#snippet action()}
        <Button type="button" variant="outline" onclick={loadProjection}>
          <Icon name="arrow-clockwise" class="size-4" />
          <span>{m.library_retry()}</span>
        </Button>
      {/snippet}
    </Notice>
  {:else if viewState.kind === 'ready'}
    <ul
      class="glass-panel divide-y divide-hairline overflow-hidden rounded-[var(--radius-panel)] border border-hairline"
      aria-label={m.library_title()}
    >
      {#each rows as entry (entry.resource_id)}
        <li class="min-w-0">
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
    <div data-testid="library-empty">
      <EmptyState title={m.library_empty_title()} icon="list-bullets">
        {#snippet action()}
          <Button href={resolve('/sources' as '/')} variant="outline">
            <span>{m.action_manage_sources()}</span>
            <Icon name="arrow-right" class="size-4" />
          </Button>
        {/snippet}
      </EmptyState>
    </div>
  {/if}

  {#if updateError}
    <Notice tone="danger" role="alert" icon="warning-circle">
      {m.library_update_error()}
    </Notice>
  {/if}
</PageFrame>
