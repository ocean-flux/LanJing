<script lang="ts">
  import { resolve } from '$app/paths';
  import Icon from '$lib/components/Icon.svelte';
  import Notice from '$lib/components/Notice.svelte';
  import PageFrame from '$lib/components/PageFrame.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button/index.js';
  import { m } from '$lib/i18n';

  type Props = {
    resourceId?: string | null;
  };

  let { resourceId = null }: Props = $props();

  const hasId = $derived(Boolean(resourceId && resourceId.trim().length > 0));
</script>

<div data-testid="library-detail-entry">
  <PageFrame width="standard">
    <div class="flex min-w-0 flex-col gap-3">
      <Button href={resolve('/library' as '/')} variant="outline" class="self-start">
        <Icon name="arrow-left" class="size-4" />
        <span>{m.library_detail_back()}</span>
      </Button>
      <PageHeader title={m.library_detail_title()} />
    </div>

    {#if hasId}
      <div
        class="glass-panel overflow-hidden rounded-[var(--radius-panel)] border border-hairline p-(--density-panel-padding-compact) sm:p-(--density-panel-padding)"
        data-resource-id={resourceId}
      >
        <p class="text-xs font-medium text-ink-muted">{m.library_detail_resource_label()}</p>
        <p
          class="mt-1 font-mono text-sm break-all text-ink"
          data-testid="library-detail-resource-id"
        >
          {resourceId}
        </p>
        <Notice tone="info" role="status" class="mt-4" icon="warning-circle">
          {m.library_detail_placeholder()}
        </Notice>
      </div>
    {:else}
      <div data-testid="library-detail-missing-id">
        <Notice tone="danger" role="alert" icon="warning-circle">
          {m.library_detail_missing_id()}
        </Notice>
      </div>
    {/if}
  </PageFrame>
</div>
