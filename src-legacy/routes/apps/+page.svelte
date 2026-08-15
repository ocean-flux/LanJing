<script lang="ts">
  import { m } from '$lib/i18n';
  import AppsHome from '$lib/views/apps/AppsHome.svelte';
</script>

<svelte:head>
  <title>{m.apps_title()} · {m.app_name()}</title>
</svelte:head>

<AppsHome />
