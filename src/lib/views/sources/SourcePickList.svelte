<script lang="ts">
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
</script>

<div class="flex min-h-0 flex-1 flex-col gap-3" data-testid="source-pick-list">
  {#if truncated}
    <p class="text-xs leading-5 text-ink-muted" data-testid="source-pick-display-cap">
      {m.sources_deeplink_display_cap({ shown: items.length, total: totalCount })}
    </p>
  {/if}
  {#if overInstallCap}
    <p
      class="rounded-lg border border-lantern/35 bg-lantern-soft/25 px-3 py-2 text-xs font-medium text-ink"
      role="status"
      data-testid="source-pick-install-cap"
    >
      {m.sources_deeplink_install_cap({ selected: selectedIds.length, cap: installCap })}
    </p>
  {/if}

  <ScrollArea class="min-h-0 flex-1 rounded-xl border border-hairline" orientation="vertical">
    <div class="divide-y divide-hairline" data-testid="source-pick-groups">
      {#each groups as group (group.id)}
        {@const groupIds = group.items.map((item) => item.id)}
        {@const groupChecked = groupCheckState(groupIds)}
        {@const groupLabel = group.label ?? m.sources_group_ungrouped()}
        <Collapsible open={true} class="bg-surface-1">
          <div
            class="flex min-h-11 items-center gap-2 border-b border-hairline px-2 sm:px-3"
            data-testid="source-pick-group-header"
            data-group-id={group.id}
          >
            <label
              class="inline-flex min-h-11 min-w-11 cursor-pointer items-center justify-center"
              data-testid="source-pick-group-select"
            >
              <span class="sr-only"
                >{m.sources_deeplink_group_select_all({ group: groupLabel })}</span
              >
              <Checkbox
                checked={groupChecked === true}
                indeterminate={groupChecked === 'indeterminate'}
                {disabled}
                onCheckedChange={(value) => {
                  setGroupSelected(groupIds, value === true);
                }}
              />
            </label>
            <CollapsibleTrigger
              class="flex min-h-11 min-w-0 flex-1 items-center gap-2 rounded-md px-1 text-left outline-none focus-visible:shadow-[var(--focus-ring)]"
              aria-label={m.sources_group_expand({ group: groupLabel })}
            >
              <span class="min-w-0 flex-1 truncate text-sm font-semibold text-ink"
                >{groupLabel}</span
              >
              <span class="shrink-0 text-xs text-ink-subtle"
                >{m.sources_group_count({ count: group.items.length })}</span
              >
              <Icon name="arrow-right" class="size-4" />
            </CollapsibleTrigger>
          </div>
          <CollapsibleContent>
            <ul class="divide-y divide-hairline">
              {#each group.items as item (item.id)}
                <li>
                  <label
                    class="flex min-h-11 cursor-pointer items-center gap-3 px-3 py-2.5 outline-none hover:bg-surface-2 has-focus-visible:shadow-[inset_var(--focus-ring)] sm:px-4"
                    data-testid="source-pick-row"
                    data-item-id={item.id}
                  >
                    <Checkbox
                      class="size-5"
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
