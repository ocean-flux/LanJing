<script lang="ts">
  import type { CapabilityKey } from '$lib/brand';
  import { m } from '$lib/i18n';

  type Props = {
    capability: CapabilityKey;
    label?: string;
    enabled?: boolean;
    class?: string;
  };

  const labels: Record<CapabilityKey, string> = {
    search: m.capability_search(),
    discover: m.capability_discover(),
    detail: m.capability_detail(),
    units: m.capability_units(),
    asset: m.capability_asset(),
  };

  let { capability, label, enabled = true, class: className = '' }: Props = $props();
  const resolvedLabel = $derived(label ?? labels[capability]);
</script>

<span class={['capability-chip', enabled && 'is-enabled', className]} aria-disabled={!enabled}>
  {resolvedLabel}
</span>

<style>
  .capability-chip {
    display: inline-flex;
    align-items: center;
    gap: 0;
    min-height: 2rem;
    padding: 0 0.78rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--muted-foreground);
    font-size: 0.78rem;
    line-height: 1;
    white-space: nowrap;
  }

  .is-enabled {
    border-color: color-mix(in oklab, var(--positive) 42%, transparent);
    background: color-mix(in oklab, var(--positive) 10%, var(--surface-2));
    color: var(--positive);
  }
</style>
