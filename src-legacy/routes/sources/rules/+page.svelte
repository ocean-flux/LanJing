<script lang="ts">
  import { m } from '$lib/i18n';
  import RuleWorkspace from '$lib/views/rules/RuleWorkspace.svelte';
</script>

<svelte:head>
  <title>{m.rules_title()} · {m.app_name()}</title>
</svelte:head>

<RuleWorkspace />
