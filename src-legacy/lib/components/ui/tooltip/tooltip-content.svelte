<script lang="ts">
  import { Tooltip as TooltipPrimitive } from 'bits-ui';
  import { cn } from '$lib/utils.js';
  import TooltipPortal from './tooltip-portal.svelte';
  import type { ComponentProps } from 'svelte';
  import type { WithoutChildrenOrChild } from '$lib/utils.js';

  let {
    ref = $bindable(null),
    class: className,
    sideOffset = 0,
    side = 'top',
    children,
    arrowClasses,
    portalProps,
    ...restProps
  }: TooltipPrimitive.ContentProps & {
    arrowClasses?: string;
    portalProps?: WithoutChildrenOrChild<ComponentProps<typeof TooltipPortal>>;
  } = $props();
</script>

<TooltipPortal {...portalProps}>
  <TooltipPrimitive.Content
    bind:ref
    data-slot="tooltip-content"
    {sideOffset}
    {side}
    class={cn(
      'elevated-overlay z-(--layer-popover) inline-flex w-fit max-w-xs origin-(--bits-tooltip-content-transform-origin) items-center gap-1.5 rounded-md border border-hairline px-2 py-1 text-xs text-ink transition-[opacity,transform] duration-(--motion-fast) ease-[var(--motion-standard)] has-data-[slot=kbd]:pr-1.5 data-[ending-style]:scale-95 data-[ending-style]:opacity-0 **:data-[slot=kbd]:relative **:data-[slot=kbd]:isolate **:data-[slot=kbd]:z-(--layer-popover) **:data-[slot=kbd]:rounded-sm data-[starting-style]:scale-95 data-[starting-style]:opacity-0',
      className,
    )}
    {...restProps}
  >
    {@render children?.()}
    <TooltipPrimitive.Arrow>
      {#snippet child({ props })}
        <div
          class={cn(
            'z-(--layer-popover) size-2.5 translate-y-[calc(-50%-2px)] rotate-45 rounded-[2px] border border-hairline bg-[var(--surface-overlay)] fill-[var(--surface-overlay)]',
            'data-[side=top]:translate-x-1/2 data-[side=top]:translate-y-[calc(-50%+2px)]',
            'data-[side=bottom]:-translate-x-1/2 data-[side=bottom]:-translate-y-[calc(-50%+1px)]',
            'data-[side=right]:translate-x-[calc(50%+2px)] data-[side=right]:translate-y-1/2',
            'data-[side=left]:-translate-y-[calc(50%-3px)]',
            arrowClasses,
          )}
          {...props}
        ></div>
      {/snippet}
    </TooltipPrimitive.Arrow>
  </TooltipPrimitive.Content>
</TooltipPortal>
