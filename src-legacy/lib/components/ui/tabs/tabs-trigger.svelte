<script lang="ts">
  import { Tabs as TabsPrimitive } from 'bits-ui';
  import { cn } from '$lib/utils.js';

  let {
    ref = $bindable(null),
    class: className,
    ...restProps
  }: TabsPrimitive.TriggerProps = $props();
</script>

<TabsPrimitive.Trigger
  bind:ref
  data-slot="tabs-trigger"
  class={cn(
    "inline-flex h-full min-w-0 flex-1 items-center justify-center gap-1.5 rounded-sm border border-transparent px-2 text-sm font-medium whitespace-nowrap text-ink-muted transition-[color,background-color,border-color,box-shadow] duration-(--motion-fast) outline-none focus-visible:border-lantern-strong/60 focus-visible:shadow-[var(--focus-ring)] disabled:pointer-events-none disabled:opacity-50 aria-busy:cursor-wait aria-busy:opacity-70 data-[state=active]:border-hairline data-[state=active]:bg-surface-selected data-[state=active]:text-ink [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [@media(pointer:coarse)]:min-h-(--density-touch-target) [@media(pointer:coarse)]:min-w-(--density-touch-target)",
    className,
  )}
  {...restProps}
/>
