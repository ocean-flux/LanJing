<script lang="ts">
  import EmptyState from '$lib/components/EmptyState.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import PageFrame from '$lib/components/PageFrame.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button/index.js';
  import { m } from '$lib/i18n';
</script>

<PageFrame width="standard">
  <PageHeader title={m.realm_title()} />

  <EmptyState title={m.realm_empty_title()} icon="compass" class="w-full max-w-2xl">
    {#snippet action()}
      <Button href="/sources" variant="outline">
        <span>{m.action_manage_sources()}</span>
        <Icon name="arrow-right" class="size-4" />
      </Button>
    {/snippet}
  </EmptyState>
</PageFrame>
