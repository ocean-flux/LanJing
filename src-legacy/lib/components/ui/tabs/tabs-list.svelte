<script lang="ts">
  import { Tabs as TabsPrimitive } from 'bits-ui';
  import { cn } from '$lib/utils.js';

  let { ref = $bindable(null), class: className, ...restProps }: TabsPrimitive.ListProps = $props();
</script>

<TabsPrimitive.List
  bind:ref
  data-slot="tabs-list"
  class={cn(
    'glass-control inline-flex h-(--density-control-md) w-fit items-center justify-center rounded-md border border-hairline p-0.5 text-muted-foreground [@media(pointer:coarse)]:h-(--density-touch-target)',
    className,
  )}
  {...restProps}
/>
