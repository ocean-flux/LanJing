<script lang="ts">
  import { page } from '$app/state';
  import { m } from '$lib/i18n';
  import LibraryDetailEntry from '$lib/views/library/LibraryDetailEntry.svelte';

  const resourceId = $derived(page.params.resourceId ?? null);
</script>

<svelte:head>
  <title
    >{resourceId
      ? `${m.library_detail_title()} · ${resourceId} · ${m.app_name()}`
      : `${m.library_detail_title()} · ${m.app_name()}`}</title
  >
</svelte:head>

<LibraryDetailEntry {resourceId} />
