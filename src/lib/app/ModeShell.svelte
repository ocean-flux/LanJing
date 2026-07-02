<script lang="ts">
  import AppShell from './AppShell.svelte';

  let { children }: { children?: import('svelte').Snippet } = $props();
</script>

<AppShell>
  {#if children}
    {@render children()}
  {/if}
</AppShell>
