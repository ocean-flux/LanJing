<script lang="ts">
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import Notice from '$lib/components/Notice.svelte';
  import PageFrame from '$lib/components/PageFrame.svelte';
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

<PageFrame width="standard" class="gap-(--density-panel-padding-compact)">
  <PageHeader
    title={m.sources_title()}
    action={{
      label: m.sources_rules_open_workspace(),
      icon: 'arrow-right',
      href: '/sources/rules',
    }}
  />

  {#if loading}
    <Notice tone="info" role="status" icon="arrow-clockwise" class="w-full max-w-2xl">
      {m.sources_loading()}
    </Notice>
  {:else if loadError}
    <div class="w-full max-w-2xl space-y-2">
      <Notice
        tone="danger"
        role="alert"
        title={m.sources_load_error()}
        icon="warning-circle"
        class="w-full"
      >
        <span class="break-words">{loadError}</span>
      </Notice>
      <Button type="button" variant="outline" onclick={retryLoad}>
        <Icon name="arrow-clockwise" class="size-4" />
        <span>{m.action_retry()}</span>
      </Button>
    </div>
  {:else if sources.length === 0}
    <div class="w-full max-w-2xl space-y-2">
      <EmptyState
        title={m.sources_empty_title()}
        description={m.sources_empty_hint()}
        icon="database"
        class="w-full"
      />
      <Button type="button" onclick={openInstaller}>
        <Icon name="plus" class="size-4" />
        <span>{m.action_add_source()}</span>
      </Button>
    </div>
  {:else}
    <section aria-labelledby="installed-sources-title" class="flex min-w-0 flex-col gap-2.5">
      <div class="flex flex-wrap items-end justify-between gap-2">
        <div class="min-w-0">
          <h2 id="installed-sources-title" class="text-sm font-semibold text-ink">
            {m.sources_installed_title()}
          </h2>
          <p class="mt-0.5 text-xs text-ink-subtle">
            {m.sources_group_count({ count: sources.length })}
          </p>
        </div>
        <Button type="button" onclick={openInstaller}>
          <Icon name="plus" class="size-4" />
          <span>{m.action_add_source()}</span>
        </Button>
      </div>

      <div
        class="-mx-1 flex [scrollbar-width:none] gap-1 overflow-x-auto px-1 pb-1 [-ms-overflow-style:none] sm:mx-0 sm:flex-wrap sm:overflow-visible sm:px-0 [&::-webkit-scrollbar]:hidden"
        role="group"
        aria-label={m.sources_group_filter()}
      >
        <Button
          type="button"
          size="sm"
          variant={activeGroupFilter === ALL_FILTER ? 'secondary' : 'ghost'}
          class="shrink-0"
          aria-pressed={activeGroupFilter === ALL_FILTER}
          onclick={() => selectGroup(ALL_FILTER)}
        >
          {m.sources_group_all()}
        </Button>
        {#each groupNames as group (group)}
          <Button
            type="button"
            size="sm"
            variant={activeGroupFilter === groupFilter(group) ? 'secondary' : 'ghost'}
            class="shrink-0"
            aria-pressed={activeGroupFilter === groupFilter(group)}
            onclick={() => selectGroup(groupFilter(group))}
          >
            {group}
          </Button>
        {/each}
        {#if hasUngrouped}
          <Button
            type="button"
            size="sm"
            variant={activeGroupFilter === UNGROUPED_FILTER ? 'secondary' : 'ghost'}
            class="shrink-0"
            aria-pressed={activeGroupFilter === UNGROUPED_FILTER}
            onclick={() => selectGroup(UNGROUPED_FILTER)}
          >
            {m.sources_group_ungrouped()}
          </Button>
        {/if}
      </div>

      <div
        class="glass-panel divide-y divide-hairline overflow-hidden rounded-[var(--radius-panel)] border border-hairline"
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
      class="w-full gap-0 overflow-hidden p-0 data-[side=bottom]:max-h-[90dvh] data-[side=right]:sm:max-w-2xl data-[side=right]:lg:max-w-4xl"
    >
      <SheetHeader class="border-b border-hairline pr-12">
        <SheetTitle>{m.sources_install_title()}</SheetTitle>
      </SheetHeader>
      <div
        class="min-h-0 flex-1 overflow-y-auto p-(--density-panel-padding-compact) sm:p-(--density-panel-padding)"
      >
        <SourceInstallEntry showHeading={false} stickyActions onInstalled={handleInstalled} />
      </div>
    </SheetContent>
  </Sheet>
</PageFrame>
