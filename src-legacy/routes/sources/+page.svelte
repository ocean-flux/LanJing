<script lang="ts">
  import { m } from '$lib/i18n';
  import SourcesHome from '$lib/views/sources/SourcesHome.svelte';
</script>

<svelte:head>
  <title>{m.sources_title()} · {m.app_name()}</title>
</svelte:head>

<SourcesHome />
