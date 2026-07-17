<script lang="ts">
  import { resolve } from '$app/paths';
  import Search from '@lucide/svelte/icons/search';
  import X from '@lucide/svelte/icons/x';
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Input } from '$lib/components/ui/input';
  import { m } from '$lib/i18n';

  type Props = {
    open?: boolean;
    hasSources?: boolean;
    onclose?: () => void;
  };

  let { open = false, hasSources = false, onclose }: Props = $props();
</script>

<Dialog.Root bind:open={() => open, (value) => !value && onclose?.()}>
  <Dialog.Content
    class="surface-material motion-command-lens max-w-2xl border-hairline p-0 shadow-surface-dialog data-[state=open]:motion-safe:scale-100 data-[state=open]:motion-safe:opacity-100 data-[state=closed]:motion-safe:scale-[0.98] data-[state=closed]:motion-safe:opacity-0 sm:max-w-2xl"
    showCloseButton={false}
  >
    <div class="border-b border-hairline p-4 sm:p-5">
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <Dialog.Title id="global-search-title" class="text-lg font-semibold text-ink">
            {m.search_global()}
          </Dialog.Title>
          <Dialog.Description class="mt-1 text-sm text-ink-muted">
            {hasSources ? m.search_hint_ready() : m.search_hint_empty()}
          </Dialog.Description>
        </div>
        <Dialog.Close>
          {#snippet child({ props }: { props: Record<string, unknown> })}
            <Button
              {...props}
              type="button"
              variant="ghost"
              size="icon"
              class="motion-nav-capsule rounded-lg text-ink-muted hover:bg-lantern-soft hover:text-ink"
              aria-label={m.search_close()}
            >
              <X size={16} aria-hidden="true" />
            </Button>
          {/snippet}
        </Dialog.Close>
      </div>

      <div class="surface-control mt-4 flex h-12 items-center gap-3 rounded-xl px-4">
        <Search size={17} class="shrink-0 text-ink-subtle" aria-hidden="true" />
        <label class="sr-only" for="global-search-placeholder">{m.search_input()}</label>
        <Input
          id="global-search-placeholder"
          class="h-10 flex-1 border-0 bg-transparent px-0 shadow-none focus-visible:shadow-none"
          placeholder={hasSources ? m.search_placeholder_ready() : m.search_placeholder_empty()}
          disabled={!hasSources}
        />
      </div>
    </div>

    <div class="p-4 sm:p-5">
      <div class="media-void rounded-xl p-4 text-sm" role="status">
        {#if hasSources}
          <p class="font-medium text-ink">{m.search_hint_ready()}</p>
        {:else}
          <p class="font-medium text-ink">{m.search_global()}</p>
          <p class="mt-1 text-ink-muted">{m.search_hint_empty()}</p>
          <div class="mt-4 flex flex-wrap gap-2">
            <a
              href={resolve('/sources')}
              class="lantern-action inline-flex h-9 items-center rounded-full px-4 text-sm font-medium outline-none focus-visible:shadow-[var(--focus-ring)]"
              onclick={onclose}
            >
              {m.action_add_source()}
            </a>
            <a
              href={resolve('/library')}
              class="inline-flex h-9 items-center rounded-full border border-hairline px-4 text-sm font-medium text-ink outline-none hover:bg-lantern-soft focus-visible:shadow-[var(--focus-ring)]"
              onclick={onclose}
            >
              {m.action_import_local()}
            </a>
          </div>
        {/if}
      </div>
    </div>
  </Dialog.Content>
</Dialog.Root>
