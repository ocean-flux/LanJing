<script lang="ts">
  import { Select as SelectPrimitive } from 'bits-ui';
  import Icon from '$lib/components/Icon.svelte';
  import { cn, type WithoutChild } from '$lib/utils.js';

  let {
    ref = $bindable(null),
    class: className,
    value,
    label,
    children: childrenProp,
    ...restProps
  }: WithoutChild<SelectPrimitive.ItemProps> = $props();
</script>

<SelectPrimitive.Item
  bind:ref
  {value}
  {label}
  data-slot="select-item"
  class={cn(
    "relative flex min-h-(--density-control-sm) w-full cursor-default items-center gap-2 rounded-md py-1 ps-2 pe-8 text-sm outline-none select-none data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[highlighted]:bg-lantern-soft data-[highlighted]:text-ink [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [&_svg:not([class*='text-'])]:text-muted-foreground *:[span]:last:flex *:[span]:last:items-center *:[span]:last:gap-2 [@media(pointer:coarse)]:min-h-(--density-touch-target)",
    className,
  )}
  {...restProps}
>
  {#snippet children({ selected, highlighted })}
    <span class="absolute end-2 flex size-3.5 items-center justify-center">
      {#if selected}
        <Icon name="check" class="size-4" />
      {/if}
    </span>
    {#if childrenProp}
      {@render childrenProp({ selected, highlighted })}
    {:else}
      {label || value}
    {/if}
  {/snippet}
</SelectPrimitive.Item>
