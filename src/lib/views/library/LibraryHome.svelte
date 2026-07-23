<script lang="ts">
  import { onMount } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import Icon from '$lib/components/Icon.svelte';
  import { resolve } from '$app/paths';
  import PageHeader from '$lib/components/PageHeader.svelte';
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

<section class="mx-auto flex w-full max-w-6xl flex-col gap-6">
  <PageHeader title={m.library_title()} />

  {#if viewState.kind === 'loading'}
    <p class="flex min-h-48 items-center gap-3 text-sm text-ink-muted" role="status">
      <Icon name="database" class="size-5" />
      <span>{m.library_loading()}</span>
    </p>
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
      {#each items as entry (entry.resource_id)}
        <li>
          <article
            class="grid gap-3 px-4 py-3 md:grid-cols-[minmax(0,1fr)_auto]"
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
              <p class="mt-1 text-xs text-ink-subtle">
                {m.library_revision({ revision: entry.state.revision })}
              </p>
            </div>
            <div
              class="flex items-start gap-1"
              role="group"
              aria-label={entry.resource_id}
              aria-busy={pendingResourceIds.has(entry.resource_id)}
            >
              <button
                type="button"
                class="glass-control grid h-11 w-11 place-items-center rounded-md border border-hairline text-ink-muted outline-none hover:bg-lantern-soft hover:text-ink focus-visible:shadow-[var(--focus-ring)] disabled:cursor-wait disabled:opacity-60"
                aria-label={entry.state.favorite ? m.library_unfavorite() : m.library_favorite()}
                aria-pressed={entry.state.favorite}
                aria-busy={pendingResourceIds.has(entry.resource_id)}
                disabled={pendingResourceIds.has(entry.resource_id)}
                onclick={() => toggleState(entry.resource_id, 'favorite')}
              >
                <Icon name={entry.state.favorite ? 'star-fill' : 'star'} class="size-4" />
              </button>
              <button
                type="button"
                class="glass-control grid h-11 w-11 place-items-center rounded-md border border-hairline text-ink-muted outline-none hover:bg-lantern-soft hover:text-ink focus-visible:shadow-[var(--focus-ring)] disabled:cursor-wait disabled:opacity-60"
                aria-label={entry.state.pinned ? m.library_unpin() : m.library_pin()}
                aria-pressed={entry.state.pinned}
                aria-busy={pendingResourceIds.has(entry.resource_id)}
                disabled={pendingResourceIds.has(entry.resource_id)}
                onclick={() => toggleState(entry.resource_id, 'pinned')}
              >
                <Icon name={entry.state.pinned ? 'push-pin-fill' : 'push-pin'} class="size-4" />
              </button>
            </div>
          </article>
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
