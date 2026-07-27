<script lang="ts">
  import Notice from '$lib/components/Notice.svelte';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import {
    Collapsible,
    CollapsibleContent,
    CollapsibleTrigger,
  } from '$lib/components/ui/collapsible';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import Icon from '$lib/components/Icon.svelte';
  import { CATALOG_INSTALL_CAP, groupCatalogItems, type CatalogItem } from '$lib/deeplink/catalog';
  import { m } from '$lib/i18n';
  import { SvelteSet } from 'svelte/reactivity';

  type Props = {
    items: CatalogItem[];
    /** 展示是否因硬顶截断。 */
    truncated?: boolean;
    totalCount?: number;
    selectedIds?: string[];
    installCap?: number;
    disabled?: boolean;
    onSelectedIdsChange?: (ids: string[]) => void;
  };

  let {
    items,
    truncated = false,
    totalCount = items.length,
    selectedIds = $bindable<string[]>([]),
    installCap = CATALOG_INSTALL_CAP,
    disabled = false,
    onSelectedIdsChange,
  }: Props = $props();

  const groups = $derived(groupCatalogItems(items));
  const selectedSet = $derived(new Set(selectedIds));
  const overInstallCap = $derived(selectedIds.length > installCap);
  const collapsedGroupIds = new SvelteSet<string>();

  function commit(next: string[]): void {
    selectedIds = next;
    onSelectedIdsChange?.(next);
  }

  function toggleItem(id: string, checked: boolean | 'indeterminate'): void {
    if (disabled) return;
    const next = new SvelteSet(selectedIds);
    if (checked === true) {
      next.add(id);
    } else {
      next.delete(id);
    }
    commit([...next]);
  }

  function setGroupSelected(groupItemIds: string[], selectAll: boolean): void {
    if (disabled) return;
    const next = new SvelteSet(selectedIds);
    if (selectAll) {
      for (const id of groupItemIds) next.add(id);
    } else {
      for (const id of groupItemIds) next.delete(id);
    }
    commit([...next]);
  }

  function groupCheckState(groupItemIds: string[]): boolean | 'indeterminate' {
    const selectedCount = groupItemIds.filter((id) => selectedSet.has(id)).length;
    if (selectedCount === 0) return false;
    if (selectedCount === groupItemIds.length) return true;
    return 'indeterminate';
  }

  function isGroupOpen(groupId: string): boolean {
    return !collapsedGroupIds.has(groupId);
  }

  function setGroupOpen(groupId: string, open: boolean): void {
    if (open) {
      collapsedGroupIds.delete(groupId);
    } else {
      collapsedGroupIds.add(groupId);
    }
  }
</script>

<div class="flex min-h-0 flex-1 flex-col gap-2.5" data-testid="source-pick-list">
  {#if truncated}
    <p class="text-xs leading-5 text-ink-muted" data-testid="source-pick-display-cap">
      {m.sources_deeplink_display_cap({ shown: items.length, total: totalCount })}
    </p>
  {/if}
  {#if overInstallCap}
    <div data-testid="source-pick-install-cap">
      <Notice tone="warning" role="status" icon="warning-circle">
        {m.sources_deeplink_install_cap({ selected: selectedIds.length, cap: installCap })}
      </Notice>
    </div>
  {/if}

  <ScrollArea
    class="min-h-0 flex-1 rounded-[var(--radius-panel)] border border-hairline"
    orientation="vertical"
  >
    <div class="divide-y divide-hairline" data-testid="source-pick-groups">
      {#each groups as group (group.id)}
        {@const groupIds = group.items.map((item) => item.id)}
        {@const groupChecked = groupCheckState(groupIds)}
        {@const groupLabel = group.label ?? m.sources_group_ungrouped()}
        {@const groupOpen = isGroupOpen(group.id)}
        <Collapsible
          open={groupOpen}
          onOpenChange={(open) => setGroupOpen(group.id, open)}
          class="bg-surface-1"
          role="group"
          aria-label={groupLabel}
        >
          <div
            class="flex min-h-(--density-control-md) items-center gap-2 border-b border-hairline bg-surface-2/45 px-2 sm:px-3"
            data-testid="source-pick-group-header"
            data-group-id={group.id}
            data-open={groupOpen ? 'true' : 'false'}
          >
            <div
              class="inline-flex min-h-(--density-control-md) min-w-(--density-control-md) items-center justify-center"
              data-testid="source-pick-group-select"
            >
              <Checkbox
                aria-label={m.sources_deeplink_group_select_all({ group: groupLabel })}
                checked={groupChecked === true}
                indeterminate={groupChecked === 'indeterminate'}
                {disabled}
                onCheckedChange={(value) => {
                  setGroupSelected(groupIds, value === true);
                }}
              />
            </div>
            <CollapsibleTrigger
              class="flex min-h-(--density-control-md) min-w-0 flex-1 items-center gap-2 rounded-md px-1 text-left outline-none focus-visible:shadow-[var(--focus-ring)]"
              aria-label={groupOpen
                ? m.sources_group_collapse({ group: groupLabel })
                : m.sources_group_expand({ group: groupLabel })}
            >
              <span class="min-w-0 flex-1 truncate text-sm font-semibold text-ink">
                {groupLabel}
              </span>
              <span class="shrink-0 text-xs text-ink-subtle">
                {m.sources_group_count({ count: group.items.length })}
              </span>
              <Icon
                name="arrow-right"
                class={groupOpen
                  ? 'size-4 shrink-0 rotate-90 transition-transform duration-(--motion-fast)'
                  : 'size-4 shrink-0 transition-transform duration-(--motion-fast)'}
              />
            </CollapsibleTrigger>
          </div>
          <CollapsibleContent>
            <ul class="divide-y divide-hairline">
              {#each group.items as item (item.id)}
                <li>
                  <label
                    class={[
                      'flex min-h-(--density-control-md) items-center gap-2.5 px-3 py-2 outline-none has-focus-visible:shadow-[inset_var(--focus-ring)] sm:px-4',
                      disabled ? 'cursor-default' : 'cursor-pointer hover:bg-surface-2',
                    ]}
                    data-testid="source-pick-row"
                    data-item-id={item.id}
                  >
                    <Checkbox
                      checked={selectedSet.has(item.id)}
                      {disabled}
                      onCheckedChange={(value) => toggleItem(item.id, value)}
                    />
                    <span class="min-w-0 flex-1 truncate text-sm text-ink">{item.name}</span>
                  </label>
                </li>
              {/each}
            </ul>
          </CollapsibleContent>
        </Collapsible>
      {/each}
    </div>
  </ScrollArea>
</div>
