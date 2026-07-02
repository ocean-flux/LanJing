<script lang="ts">
  import ImportPreview from '$lib/components/import-preview.svelte';
  import ExecutionWitness from '$lib/components/execution-witness.svelte';

  let activeTab = $state<'import' | 'execute'>('import');
</script>

<div class="flex flex-col h-full gap-4 px-4 py-4">
  <!-- Tab 切换 -->
  <div class="flex gap-1 border-b pb-2" role="tablist">
    <button
      type="button"
      role="tab"
      aria-selected={activeTab === 'import'}
      class="px-4 py-1.5 text-sm rounded-t transition-colors
      {activeTab === 'import'
        ? 'bg-primary/10 text-primary font-semibold border-b-2 border-primary'
        : 'text-muted-foreground hover:text-foreground hover:bg-accent'}"
      onclick={() => (activeTab = 'import')}
    >
      导入规则
    </button>
    <button
      type="button"
      role="tab"
      aria-selected={activeTab === 'execute'}
      class="px-4 py-1.5 text-sm rounded-t transition-colors
      {activeTab === 'execute'
        ? 'bg-primary/10 text-primary font-semibold border-b-2 border-primary'
        : 'text-muted-foreground hover:text-foreground hover:bg-accent'}"
      onclick={() => (activeTab = 'execute')}
    >
      执行 Witness
    </button>
  </div>

  <!-- Tab 内容 -->
  {#if activeTab === 'import'}
    <ImportPreview />
  {:else}
    <ExecutionWitness />
  {/if}
</div>
