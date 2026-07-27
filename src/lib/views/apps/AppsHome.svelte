<script lang="ts">
  import { resolve } from '$app/paths';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import PageFrame from '$lib/components/PageFrame.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button/index.js';
  import { m } from '$lib/i18n';
</script>

<PageFrame width="standard">
  <PageHeader title={m.apps_title()} />

  <EmptyState title={m.apps_empty_title()} icon="squares-four">
    {#snippet action()}
      <Button href={resolve('/sources' as '/')} variant="outline">
        <span>{m.action_manage_sources()}</span>
        <Icon name="arrow-right" class="size-4" />
      </Button>
    {/snippet}
  </EmptyState>
</PageFrame>
