<script lang="ts">
  import { Select as SelectPrimitive } from 'bits-ui';
  import Icon from '$lib/components/Icon.svelte';
  import { cn, type WithoutChildrenOrChild } from '$lib/utils.js';

  let {
    ref = $bindable(null),
    class: className,
    ...restProps
  }: WithoutChildrenOrChild<SelectPrimitive.ScrollDownButtonProps> = $props();
</script>

<SelectPrimitive.ScrollDownButton
  bind:ref
  data-slot="select-scroll-down-button"
  class={cn(
    'flex min-h-(--density-control-sm) cursor-default items-center justify-center py-1 text-ink-muted [@media(pointer:coarse)]:min-h-(--density-touch-target)',
    className,
  )}
  {...restProps}
>
  <Icon name="caret-down" class="size-4" />
</SelectPrimitive.ScrollDownButton>
