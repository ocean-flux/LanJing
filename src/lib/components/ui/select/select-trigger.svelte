<script lang="ts">
  import { Select as SelectPrimitive } from 'bits-ui';
  import Icon from '$lib/components/Icon.svelte';
  import { cn, type WithoutChild } from '$lib/utils.js';

  let {
    ref = $bindable(null),
    class: className,
    children,
    size = 'default',
    ...restProps
  }: WithoutChild<SelectPrimitive.TriggerProps> & {
    size?: 'sm' | 'default';
  } = $props();
</script>

<SelectPrimitive.Trigger
  bind:ref
  data-slot="select-trigger"
  data-size={size}
  class={cn(
    "glass-control flex w-fit items-center justify-between gap-2 rounded-md border border-hairline px-2.5 text-sm whitespace-nowrap transition-[color,background-color,border-color,box-shadow] duration-(--motion-fast) outline-none select-none focus-visible:border-lantern-strong/60 focus-visible:shadow-[var(--focus-ring)] disabled:cursor-not-allowed disabled:opacity-50 aria-busy:cursor-wait aria-busy:opacity-70 aria-invalid:border-destructive aria-invalid:ring-2 aria-invalid:ring-destructive/25 data-[placeholder]:text-muted-foreground data-[size=default]:h-(--density-control-md) data-[size=sm]:h-(--density-control-sm) *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center *:data-[slot=select-value]:gap-2 dark:aria-invalid:ring-destructive/40 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [&_svg:not([class*='text-'])]:text-muted-foreground [@media(pointer:coarse)]:min-h-(--density-touch-target) [@media(pointer:coarse)]:min-w-(--density-touch-target)",
    className,
  )}
  {...restProps}
>
  {@render children?.()}
  <Icon name="caret-down" class="size-4 opacity-60" />
</SelectPrimitive.Trigger>
