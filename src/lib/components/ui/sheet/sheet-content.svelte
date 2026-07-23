<script lang="ts" module>
  export type Side = 'top' | 'right' | 'bottom' | 'left';
</script>

<script lang="ts">
  import { Dialog as SheetPrimitive } from 'bits-ui';
  import type { Snippet } from 'svelte';
  import SheetPortal from './sheet-portal.svelte';
  import SheetOverlay from './sheet-overlay.svelte';
  import { Button } from '$lib/components/ui/button/index.js';
  import Icon from '$lib/components/Icon.svelte';
  import { cn, type WithoutChildrenOrChild } from '$lib/utils.js';
  import type { ComponentProps } from 'svelte';
  import { m } from '$lib/i18n';

  let {
    ref = $bindable(null),
    class: className,
    side = 'right',
    showCloseButton = true,
    portalProps,
    children,
    ...restProps
  }: WithoutChildrenOrChild<SheetPrimitive.ContentProps> & {
    portalProps?: WithoutChildrenOrChild<ComponentProps<typeof SheetPortal>>;
    side?: Side;
    showCloseButton?: boolean;
    children: Snippet;
  } = $props();
</script>

<SheetPortal {...portalProps}>
  <SheetOverlay />
  <SheetPrimitive.Content
    bind:ref
    data-slot="sheet-content"
    data-side={side}
    class={cn(
      'glass-panel fixed z-50 flex max-h-dvh flex-col gap-4 overflow-y-auto overscroll-contain border border-hairline bg-clip-padding text-sm text-popover-foreground transition duration-200 ease-in-out data-[side=bottom]:inset-x-0 data-[side=bottom]:bottom-0 data-[side=bottom]:h-auto data-[side=bottom]:rounded-t-xl data-[side=bottom]:rounded-b-none data-[side=left]:inset-y-0 data-[side=left]:left-0 data-[side=left]:h-full data-[side=left]:w-3/4 data-[side=left]:rounded-l-none data-[side=left]:rounded-r-xl data-[side=right]:inset-y-0 data-[side=right]:right-0 data-[side=right]:h-full data-[side=right]:w-3/4 data-[side=right]:rounded-l-xl data-[side=right]:rounded-r-none data-[side=top]:inset-x-0 data-[side=top]:top-0 data-[side=top]:h-auto data-[side=top]:rounded-t-none data-[side=top]:rounded-b-xl sm:max-w-sm',
      className,
    )}
    {...restProps}
  >
    {@render children?.()}
    {#if showCloseButton}
      <SheetPrimitive.Close data-slot="sheet-close">
        {#snippet child({ props })}
          <Button variant="ghost" class="absolute top-3 right-3" size="icon-sm" {...props}>
            <Icon name="x" class="size-4" />
            <span class="sr-only">{m.action_close()}</span>
          </Button>
        {/snippet}
      </SheetPrimitive.Close>
    {/if}
  </SheetPrimitive.Content>
</SheetPortal>
