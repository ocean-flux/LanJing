<script lang="ts">
  import { m } from '$lib/i18n';
  import LibraryHome from '$lib/views/library/LibraryHome.svelte';
</script>

<svelte:head>
  <title>{m.library_title()} · {m.app_name()}</title>
</svelte:head>

<LibraryHome />
