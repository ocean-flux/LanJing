<script lang="ts">
  import { onMount } from 'svelte';
  import { getInstalledSources, loadInstalledSources } from '$lib/stores/rules.svelte';
  import {
    startSearch,
    startDiscover,
    selectMediaItem,
    selectMediaUnit,
    goBack,
    cleanup,
    getMediaItems,
    getMediaUnits,
    getResolvedText,
    getLoading,
    getError,
    getCurrentStage,
    getSelectedItem,
    getSelectedUnit,
  } from '$lib/stores/execution.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { m } from '$lib/i18n';

  let selectedRuleId = $state('');
  let searchQuery = $state('');

  onMount(() => {
    loadInstalledSources();
    return () => cleanup();
  });

  function handleSearch() {
    if (!selectedRuleId || !searchQuery.trim()) return;
    startSearch(selectedRuleId, searchQuery.trim());
  }

  function handleDiscover() {
    if (!selectedRuleId) return;
    startDiscover(selectedRuleId);
  }
</script>

<div class="flex h-full flex-col gap-4 overflow-auto">
  <h2 class="text-lg font-semibold">{m.debug_witness_title()}</h2>

  <div class="flex flex-wrap items-end gap-2">
    <div class="flex min-w-40 flex-1 flex-col gap-1">
      <label for="rule-select" class="text-xs text-muted-foreground">{m.debug_select_rule()}</label>
      <select
        id="rule-select"
        bind:value={selectedRuleId}
        class="h-9 rounded-md border border-input bg-background px-3 text-sm"
      >
        <option value="">-- {m.debug_select_rule_placeholder()} --</option>
        {#each getInstalledSources() as source (source.source_id)}
          <option value={source.source_id}>
            {source.profile.title || source.source_id}
          </option>
        {/each}
      </select>
    </div>

    <Input
      type="text"
      placeholder={m.debug_search_placeholder()}
      bind:value={searchQuery}
      disabled={getLoading() || !selectedRuleId}
    />

    <Button
      onclick={handleSearch}
      disabled={getLoading() || !selectedRuleId || !searchQuery.trim()}
    >
      {m.search()}
    </Button>

    <Button onclick={handleDiscover} disabled={getLoading() || !selectedRuleId} variant="outline">
      {m.debug_discover()}
    </Button>
  </div>

  {#if getCurrentStage() === 'units' || getCurrentStage() === 'asset'}
    <div>
      <Button onclick={goBack} variant="ghost" size="sm">← {m.debug_back()}</Button>
    </div>
  {/if}

  {#if getError()}
    <div
      class="rounded-md border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
    >
      {getError()}
    </div>
  {/if}

  {#if getLoading()}
    <div class="flex items-center justify-center py-12 text-muted-foreground">
      <span class="animate-pulse">{m.debug_running()}</span>
    </div>
  {/if}

  {#if getCurrentStage() === 'results' && !getLoading()}
    {#if getMediaItems().length === 0}
      <div class="flex flex-col items-center justify-center gap-2 py-12 text-muted-foreground">
        <p>{m.debug_no_items()}</p>
        {#if searchQuery}
          <p class="text-xs">{m.debug_try_other_keyword()}</p>
        {/if}
      </div>
    {:else}
      <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 md:grid-cols-3">
        {#each getMediaItems() as item (item.id)}
          <Button
            type="button"
            variant="outline"
            class="h-auto flex-col items-start justify-start rounded-lg bg-card p-3 text-left hover:border-accent-foreground/30 hover:bg-accent"
            onclick={() => selectMediaItem(item, selectedRuleId)}
          >
            <span class="space-y-1">
              <span class="block text-sm font-semibold leading-tight">{item.title}</span>
              {#if item.creators.length > 0}
                <span class="block text-xs text-muted-foreground">{item.creators.join(' / ')}</span>
              {/if}
              <span
                class="inline-block rounded-full bg-primary/10 px-1.5 py-0.5 text-[0.7rem] text-primary"
              >
                {item.media_kind}
              </span>
              {#if item.description}
                <span class="line-clamp-2 block text-xs text-muted-foreground"
                  >{item.description}</span
                >
              {/if}
            </span>
          </Button>
        {/each}
      </div>
    {/if}
  {/if}

  {#if getCurrentStage() === 'units'}
    {#if getSelectedItem()}
      <div class="space-y-2 rounded-lg border bg-card p-3">
        <h3 class="font-semibold">{getSelectedItem()!.title}</h3>
        {#if getSelectedItem()!.description}
          <p class="text-sm text-muted-foreground">{getSelectedItem()!.description}</p>
        {/if}
      </div>
    {/if}

    {#if !getLoading()}
      {#if getMediaUnits().length === 0}
        <div class="flex flex-col items-center justify-center gap-2 py-12 text-muted-foreground">
          <p>{m.debug_no_units()}</p>
        </div>
      {:else}
        <div class="space-y-1">
          {#each getMediaUnits() as unit (unit.id)}
            <Button
              type="button"
              variant="ghost"
              class="h-auto w-full justify-start px-3 py-2 text-left text-sm"
              onclick={() => selectMediaUnit(unit, selectedRuleId)}
            >
              {unit.position ? `${unit.position}. ` : ''}{unit.title}
            </Button>
          {/each}
        </div>
      {/if}
    {/if}
  {/if}

  {#if getCurrentStage() === 'asset'}
    {@const unit = getSelectedUnit()}
    {#if unit}
      <div class="rounded-lg border bg-card p-3">
        <h3 class="font-semibold">{unit.title}</h3>
      </div>
    {/if}

    {#if !getLoading()}
      {@const text = getResolvedText()}
      {#if text}
        <div class="prose prose-sm max-w-none whitespace-pre-wrap dark:prose-invert">
          {text}
        </div>
      {:else}
        <div class="flex flex-col items-center justify-center py-12 text-muted-foreground">
          <p>{m.debug_no_asset()}</p>
        </div>
      {/if}
    {/if}
  {/if}
</div>
