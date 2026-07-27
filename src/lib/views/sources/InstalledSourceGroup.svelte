<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
  import {
    Collapsible,
    CollapsibleContent,
    CollapsibleTrigger,
  } from '$lib/components/ui/collapsible';
  import { m } from '$lib/i18n';
  import type { InstalledSource } from '$lib/stores/rules.svelte';
  import InstalledSourceRow from './InstalledSourceRow.svelte';

  type Props = {
    groupId: string;
    label: string;
    sources: InstalledSource[];
    highlightSourceId?: string | null;
  };

  let { groupId, label, sources, highlightSourceId = null }: Props = $props();
  let open = $state(true);

  const countLabel = $derived(m.sources_group_count({ count: sources.length }));
  const toggleLabel = $derived(
    open ? m.sources_group_collapse({ group: label }) : m.sources_group_expand({ group: label }),
  );
</script>

<Collapsible bind:open class="min-w-0" data-group-id={groupId}>
  <h3>
    <CollapsibleTrigger
      class="flex min-h-(--density-control-md) w-full items-center gap-2.5 bg-surface-2/45 px-(--density-panel-padding-compact) py-2 text-left transition-[background-color,box-shadow] duration-(--motion-fast) outline-none hover:bg-surface-2 focus-visible:shadow-[inset_var(--focus-ring)] sm:px-(--density-panel-padding)"
      aria-label={toggleLabel}
    >
      <span class="min-w-0 flex-1 text-sm font-semibold break-words text-ink">{label}</span>
      <span class="shrink-0 text-xs text-ink-subtle">{countLabel}</span>
      <Icon
        name="arrow-right"
        class={open
          ? 'size-4 shrink-0 rotate-90 transition-transform duration-(--motion-fast)'
          : 'size-4 shrink-0 transition-transform duration-(--motion-fast)'}
      />
    </CollapsibleTrigger>
  </h3>
  <CollapsibleContent>
    <ul class="divide-y divide-hairline border-t border-hairline">
      {#each sources as source (source.source_id)}
        <li>
          <InstalledSourceRow {source} highlighted={highlightSourceId === source.source_id} />
        </li>
      {/each}
    </ul>
  </CollapsibleContent>
</Collapsible>
