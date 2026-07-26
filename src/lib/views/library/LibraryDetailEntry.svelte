<script lang="ts">
  import { resolve } from '$app/paths';
  import Icon from '$lib/components/Icon.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { m } from '$lib/i18n';

  type Props = {
    resourceId?: string | null;
  };

  let { resourceId = null }: Props = $props();

  const hasId = $derived(Boolean(resourceId && resourceId.trim().length > 0));
</script>

<section class="mx-auto flex w-full max-w-6xl flex-col gap-6" data-testid="library-detail-entry">
  <PageHeader title={m.library_detail_title()} />

  {#if hasId}
    <div
      class="glass-panel rounded-xl border border-hairline px-5 py-5"
      data-resource-id={resourceId}
    >
      <p class="text-xs font-medium text-ink-muted">{m.library_detail_resource_label()}</p>
      <p class="mt-1 font-mono text-sm break-all text-ink" data-testid="library-detail-resource-id">
        {resourceId}
      </p>
      <p class="mt-4 text-sm text-ink-subtle">{m.library_detail_placeholder()}</p>
      <a
        href={resolve('/library' as '/')}
        class="glass-control mt-6 inline-flex min-h-11 items-center gap-2 rounded-lg border border-hairline-strong px-4 text-sm font-semibold text-ink outline-none hover:bg-surface-2 focus-visible:shadow-[var(--focus-ring)]"
      >
        <Icon name="arrow-left" class="size-4" />
        <span>{m.library_detail_back()}</span>
      </a>
    </div>
  {:else}
    <div
      class="border-danger/35 bg-danger/10 rounded-xl border px-5 py-5"
      role="alert"
      data-testid="library-detail-missing-id"
    >
      <p class="text-danger text-sm font-medium">{m.library_detail_missing_id()}</p>
      <a
        href={resolve('/library' as '/')}
        class="glass-control mt-4 inline-flex min-h-11 items-center gap-2 rounded-lg border border-hairline-strong px-4 text-sm font-semibold text-ink outline-none hover:bg-surface-2 focus-visible:shadow-[var(--focus-ring)]"
      >
        <Icon name="arrow-left" class="size-4" />
        <span>{m.library_detail_back()}</span>
      </a>
    </div>
  {/if}
</section>
