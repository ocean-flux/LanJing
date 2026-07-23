<script lang="ts">
  import { m } from '$lib/i18n';
  import SettingsHome from '$lib/views/settings/SettingsHome.svelte';
</script>

<svelte:head>
  <title>{m.settings_title()} · {m.app_name()}</title>
</svelte:head>

<SettingsHome />
