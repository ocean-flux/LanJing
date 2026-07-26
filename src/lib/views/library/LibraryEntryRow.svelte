<script lang="ts">
  import { resolve } from '$app/paths';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import Icon from '$lib/components/Icon.svelte';
  import { m } from '$lib/i18n';
  import type { LibraryEntryRowModel } from './library-media';

  type Props = {
    entry: LibraryEntryRowModel;
    pending?: boolean;
    onToggleFavorite?: () => void;
    onTogglePinned?: () => void;
  };

  let { entry, pending = false, onToggleFavorite, onTogglePinned }: Props = $props();

  const actionsLabel = $derived(entry.mediaMissing ? entry.resource_id : entry.title);
</script>

<article
  class="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-3 px-4 py-3 sm:px-5 sm:py-3.5"
  data-resource-id={entry.resource_id}
  data-media-missing={entry.mediaMissing ? 'true' : 'false'}
>
  <a
    href={resolve(`/library/item/${encodeURIComponent(entry.resource_id)}` as '/')}
    class="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-start gap-3 rounded-md outline-none focus-visible:shadow-[var(--focus-ring)]"
    data-testid="library-entry-link"
    data-resource-id={entry.resource_id}
  >
    <div
      class="size-11 shrink-0 rounded-md border border-hairline bg-media-void sm:size-12"
      aria-hidden="true"
      data-testid="library-entry-cover"
      data-has-cover-asset={entry.cover_asset_id ? 'true' : 'false'}
    ></div>

    <div class="min-w-0 self-center">
      {#if entry.mediaMissing}
        <h2 class="truncate text-sm font-semibold text-ink">{entry.resource_id}</h2>
        <p class="mt-1 text-xs text-ink-subtle">{m.library_media_missing()}</p>
      {:else}
        <div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
          <h2 class="truncate text-sm font-semibold text-ink">{entry.title}</h2>
          {#if entry.kind}
            <Badge variant="outline" class="border-hairline bg-surface-2 text-ink"
              >{entry.kind}</Badge
            >
          {/if}
        </div>
        {#if entry.state.progress}
          <p class="mt-1 text-xs text-ink-subtle">
            {entry.state.progress.position}{#if entry.state.progress.total !== null}
              / {entry.state.progress.total}
            {/if}
          </p>
        {/if}
      {/if}
      <p class="mt-1 text-xs text-ink-subtle">
        {m.library_revision({ revision: entry.state.revision })}
      </p>
    </div>
  </a>

  <div class="flex items-start gap-1" role="group" aria-label={actionsLabel} aria-busy={pending}>
    <Button
      type="button"
      variant="ghost"
      size="icon"
      class="glass-control grid h-11 w-11 place-items-center rounded-md border border-hairline text-ink-muted outline-none hover:bg-lantern-soft hover:text-ink focus-visible:border-hairline focus-visible:shadow-[var(--focus-ring)] focus-visible:ring-0 disabled:pointer-events-auto disabled:cursor-wait disabled:opacity-60"
      aria-label={entry.state.favorite ? m.library_unfavorite() : m.library_favorite()}
      aria-pressed={entry.state.favorite}
      aria-busy={pending}
      disabled={pending}
      onclick={(event) => {
        event.preventDefault();
        event.stopPropagation();
        onToggleFavorite?.();
      }}
    >
      <Icon name={entry.state.favorite ? 'star-fill' : 'star'} class="size-4" />
    </Button>
    <Button
      type="button"
      variant="ghost"
      size="icon"
      class="glass-control grid h-11 w-11 place-items-center rounded-md border border-hairline text-ink-muted outline-none hover:bg-lantern-soft hover:text-ink focus-visible:border-hairline focus-visible:shadow-[var(--focus-ring)] focus-visible:ring-0 disabled:pointer-events-auto disabled:cursor-wait disabled:opacity-60"
      aria-label={entry.state.pinned ? m.library_unpin() : m.library_pin()}
      aria-pressed={entry.state.pinned}
      aria-busy={pending}
      disabled={pending}
      onclick={(event) => {
        event.preventDefault();
        event.stopPropagation();
        onTogglePinned?.();
      }}
    >
      <Icon name={entry.state.pinned ? 'push-pin-fill' : 'push-pin'} class="size-4" />
    </Button>
  </div>
</article>
