<script lang="ts">
  import { m } from '$lib/i18n';
  import RealmHome from '$lib/views/realm/RealmHome.svelte';
</script>

<svelte:head>
  <title>{m.realm_title()} · {m.app_name()}</title>
</svelte:head>

<RealmHome />
