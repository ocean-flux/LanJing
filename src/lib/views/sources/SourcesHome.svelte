<script lang="ts">
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import Icon from '$lib/components/Icon.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Sheet, SheetContent, SheetHeader, SheetTitle } from '$lib/components/ui/sheet';
  import { IsMobile } from '$lib/hooks/is-mobile.svelte';
  import { m } from '$lib/i18n';
  import {
    getError,
    getInstalledSources,
    getLoading,
    loadInstalledSources,
    type InstalledSource,
  } from '$lib/stores/rules.svelte';
  import InstalledSourceGroup from './InstalledSourceGroup.svelte';
  import SourceInstallEntry from './SourceInstallEntry.svelte';

  const UNGROUPED_FILTER = '__filter_ungrouped__';
  const ALL_FILTER = '__filter_all__';

  type SourceGroup = {
    id: string;
    label: string;
    sources: InstalledSource[];
  };

  let initialLoadPending = $state(true);
  let retrying = $state(false);
  let installerOpen = $state(false);
  let selectedGroup = $state(ALL_FILTER);
  const isMobile = new IsMobile();

  const sources = $derived(getInstalledSources());
  const loadError = $derived(getError());
  const loading = $derived(initialLoadPending || retrying || getLoading());
  // 深链 lanjing://source/<id> 经 runtime 跳到 ?highlight=；仅读 query，不耦合 session。
  const highlightSourceId = $derived(page.url.searchParams.get('highlight'));

  const groupNames = $derived.by(() => {
    const names: string[] = [];
    for (const source of sources) {
      const group = normalizeGroup(source);
      if (group && !names.includes(group)) names.push(group);
    }
    return names.sort((a, b) => a.localeCompare(b));
  });

  const hasUngrouped = $derived(sources.some((source) => normalizeGroup(source) === null));

  const activeGroupFilter = $derived.by(() => {
    if (selectedGroup === ALL_FILTER) return ALL_FILTER;
    if (selectedGroup === UNGROUPED_FILTER && hasUngrouped) return UNGROUPED_FILTER;
    if (groupNames.some((group) => groupFilter(group) === selectedGroup)) return selectedGroup;
    return ALL_FILTER;
  });

  const filteredSources = $derived.by(() => {
    if (activeGroupFilter === ALL_FILTER) return sources;
    if (activeGroupFilter === UNGROUPED_FILTER) {
      return sources.filter((source) => normalizeGroup(source) === null);
    }
    const group = activeGroupFilter.slice('group:'.length);
    return sources.filter((source) => normalizeGroup(source) === group);
  });

  const visibleGroups = $derived.by(() => {
    const groups = new SvelteMap<string, SourceGroup>();
    for (const source of filteredSources) {
      const group = normalizeGroup(source);
      const id = group ? groupFilter(group) : UNGROUPED_FILTER;
      const current = groups.get(id);
      if (current) {
        current.sources.push(source);
      } else {
        groups.set(id, {
          id,
          label: group ?? m.sources_group_ungrouped(),
          sources: [source],
        });
      }
    }
    return [...groups.values()].sort((left, right) => {
      if (left.id === UNGROUPED_FILTER) return 1;
      if (right.id === UNGROUPED_FILTER) return -1;
      return left.label.localeCompare(right.label);
    });
  });

  onMount(() => {
    void initialize();
  });

  function normalizeGroup(source: InstalledSource): string | null {
    const group = source.profile.group?.trim();
    return group ? group : null;
  }

  function groupFilter(group: string): string {
    return `group:${group}`;
  }

  async function initialize(): Promise<void> {
    try {
      await loadInstalledSources();
    } finally {
      initialLoadPending = false;
    }
  }

  async function retryLoad(): Promise<void> {
    retrying = true;
    try {
      await loadInstalledSources();
    } finally {
      retrying = false;
    }
  }

  function openInstaller(): void {
    installerOpen = true;
  }

  function handleInstalled(): void {
    installerOpen = false;
  }

  function selectGroup(filter: string): void {
    selectedGroup = filter;
  }
</script>

<section class="mx-auto flex w-full max-w-6xl flex-col gap-6">
  <PageHeader
    title={m.sources_title()}
    action={{
      label: m.sources_rules_open_workspace(),
      icon: 'arrow-right',
      href: '/sources/rules',
    }}
  />

  {#if loading}
    <div class="flex min-h-48 items-center gap-3 text-sm text-ink-muted" role="status">
      <Icon name="arrow-clockwise" class="size-5" />
      <span>{m.sources_loading()}</span>
    </div>
  {:else if loadError}
    <div class="border-danger/35 bg-danger/10 rounded-xl border px-5 py-5" role="alert">
      <div class="flex items-start gap-3">
        <Icon name="warning-circle" class="text-danger mt-0.5 size-5" />
        <div class="min-w-0">
          <h2 class="text-danger font-semibold">{m.sources_load_error()}</h2>
          <p class="mt-1 text-sm break-words text-ink-muted">{loadError}</p>
        </div>
      </div>
      <button
        type="button"
        class="glass-control mt-4 inline-flex min-h-11 items-center gap-2 rounded-lg border border-hairline-strong px-4 text-sm font-semibold text-ink outline-none hover:bg-surface-2 focus-visible:shadow-[var(--focus-ring)]"
        onclick={retryLoad}
      >
        <Icon name="arrow-clockwise" class="size-4" />
        <span>{m.action_retry()}</span>
      </button>
    </div>
  {:else if sources.length === 0}
    <div
      class="glass-panel flex flex-col items-start rounded-xl border border-hairline px-5 py-6 sm:px-6"
      role="status"
      aria-labelledby="sources-empty-title"
    >
      <Icon name="database" class="size-7 text-ink-muted" />
      <h2 id="sources-empty-title" class="mt-4 font-semibold text-ink">
        {m.sources_empty_title()}
      </h2>
      <p class="mt-1 max-w-2xl text-sm leading-6 text-ink-muted">{m.sources_empty_hint()}</p>
      <Button type="button" onclick={openInstaller} class="mt-5 min-h-11 w-full sm:w-auto">
        <Icon name="plus" class="size-4" />
        <span>{m.action_add_source()}</span>
      </Button>
    </div>
  {:else}
    <section aria-labelledby="installed-sources-title" class="flex flex-col gap-3">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <h2 id="installed-sources-title" class="text-sm font-semibold text-ink">
          {m.sources_installed_title()}
        </h2>
        <Button type="button" variant="outline" onclick={openInstaller} class="min-h-11">
          <Icon name="plus" class="size-4" />
          <span>{m.action_add_source()}</span>
        </Button>
      </div>

      <div
        class="-mx-1 flex [scrollbar-width:none] gap-2 overflow-x-auto px-1 pb-1 [-ms-overflow-style:none] sm:flex-wrap sm:overflow-visible [&::-webkit-scrollbar]:hidden"
        role="toolbar"
        aria-label={m.sources_group_filter()}
      >
        <button
          type="button"
          class="inline-flex min-h-11 shrink-0 items-center rounded-lg border px-3 text-sm font-medium outline-none focus-visible:shadow-[var(--focus-ring)] {activeGroupFilter ===
          ALL_FILTER
            ? 'border-hairline-strong bg-lantern-soft text-ink'
            : 'border-hairline bg-surface-1 text-ink-muted hover:bg-surface-2'}"
          aria-pressed={activeGroupFilter === ALL_FILTER}
          onclick={() => selectGroup(ALL_FILTER)}
        >
          {m.sources_group_all()}
        </button>
        {#each groupNames as group (group)}
          <button
            type="button"
            class="inline-flex min-h-11 shrink-0 items-center rounded-lg border px-3 text-sm font-medium outline-none focus-visible:shadow-[var(--focus-ring)] {activeGroupFilter ===
            groupFilter(group)
              ? 'border-hairline-strong bg-lantern-soft text-ink'
              : 'border-hairline bg-surface-1 text-ink-muted hover:bg-surface-2'}"
            aria-pressed={activeGroupFilter === groupFilter(group)}
            onclick={() => selectGroup(groupFilter(group))}
          >
            {group}
          </button>
        {/each}
        {#if hasUngrouped}
          <button
            type="button"
            class="inline-flex min-h-11 shrink-0 items-center rounded-lg border px-3 text-sm font-medium outline-none focus-visible:shadow-[var(--focus-ring)] {activeGroupFilter ===
            UNGROUPED_FILTER
              ? 'border-hairline-strong bg-lantern-soft text-ink'
              : 'border-hairline bg-surface-1 text-ink-muted hover:bg-surface-2'}"
            aria-pressed={activeGroupFilter === UNGROUPED_FILTER}
            onclick={() => selectGroup(UNGROUPED_FILTER)}
          >
            {m.sources_group_ungrouped()}
          </button>
        {/if}
      </div>

      <div
        class="glass-panel divide-y divide-hairline overflow-hidden rounded-xl border border-hairline"
      >
        {#each visibleGroups as group (group.id)}
          <InstalledSourceGroup
            groupId={group.id}
            label={group.label}
            sources={group.sources}
            {highlightSourceId}
          />
        {/each}
      </div>
    </section>
  {/if}

  <Sheet bind:open={installerOpen}>
    <SheetContent
      side={isMobile.current ? 'bottom' : 'right'}
      class="w-full gap-0 overflow-hidden p-0 data-[side=bottom]:!h-[90dvh] data-[side=bottom]:max-h-[90dvh] data-[side=right]:sm:max-w-2xl data-[side=right]:lg:max-w-4xl"
    >
      <SheetHeader class="border-b border-hairline pr-12">
        <SheetTitle>{m.sources_install_title()}</SheetTitle>
      </SheetHeader>
      <div
        class="min-h-0 flex-1 overflow-y-auto px-4 py-4 pb-[max(1rem,env(safe-area-inset-bottom))] sm:px-5"
      >
        <SourceInstallEntry showHeading={false} stickyActions onInstalled={handleInstalled} />
      </div>
    </SheetContent>
  </Sheet>
</section>
