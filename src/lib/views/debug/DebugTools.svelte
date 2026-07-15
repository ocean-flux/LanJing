<script lang="ts">
  import ImportPreview from '$lib/components/ImportPreview.svelte';
  import { Button } from '$lib/components/ui/button';
  import ExecutionWitness from '$lib/components/ExecutionWitness.svelte';
  import { m } from '$lib/i18n';

  let activeTab = $state<'import' | 'execute'>('import');
</script>

<section class="mx-auto flex h-full max-w-[var(--content-max-width)] flex-col gap-4 py-4">
  <div class="surface-panel p-5">
    <p class="text-sm font-semibold text-muted-foreground">Debug</p>
    <h1 class="mt-3 text-2xl font-semibold">{m.debug_title()}</h1>
  </div>

  <div class="flex gap-1 border-b pb-2" role="tablist" aria-label={m.debug_title()}>
    <Button
      type="button"
      role="tab"
      aria-selected={activeTab === 'import'}
      variant={activeTab === 'import' ? 'secondary' : 'ghost'}
      class="rounded-t px-4 py-1.5"
      onclick={() => (activeTab = 'import')}
    >
      {m.debug_tab_import()}
    </Button>
    <Button
      type="button"
      role="tab"
      aria-selected={activeTab === 'execute'}
      variant={activeTab === 'execute' ? 'secondary' : 'ghost'}
      class="rounded-t px-4 py-1.5"
      onclick={() => (activeTab = 'execute')}
    >
      {m.debug_tab_witness()}
    </Button>
  </div>

  <div class="min-h-0 flex-1 overflow-auto">
    {#if activeTab === 'import'}
      <ImportPreview />
    {:else}
      <ExecutionWitness />
    {/if}
  </div>
</section>
