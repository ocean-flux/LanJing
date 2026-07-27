<script lang="ts">
  import { m } from '$lib/i18n';
  import RuleWorkspace from '$lib/views/sources/rules/RuleWorkspace.svelte';
</script>

<svelte:head>
  <title>{m.sources_rules_page_title()} · {m.app_name()}</title>
</svelte:head>

<RuleWorkspace />
