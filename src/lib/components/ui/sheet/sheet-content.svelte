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
      'elevated-overlay fixed z-(--layer-overlay) flex max-h-dvh flex-col gap-3 overflow-y-auto overscroll-contain border border-hairline bg-clip-padding text-sm text-ink transition-[transform,opacity] duration-(--motion-base) ease-[var(--motion-standard)] data-[ending-style]:opacity-0 data-[side=bottom]:inset-x-0 data-[side=bottom]:bottom-0 data-[side=bottom]:h-auto data-[side=bottom]:max-h-[calc(100dvh-var(--safe-area-top))] data-[side=bottom]:rounded-t-[var(--radius-overlay)] data-[side=bottom]:rounded-b-none data-[side=bottom]:pr-(--safe-area-right) data-[side=bottom]:pb-(--safe-area-bottom) data-[side=bottom]:pl-(--safe-area-left) data-[side=bottom]:data-[ending-style]:translate-y-full data-[side=left]:inset-y-0 data-[side=left]:left-0 data-[side=left]:h-full data-[side=left]:w-3/4 data-[side=left]:rounded-l-none data-[side=left]:rounded-r-[var(--radius-overlay)] data-[side=left]:pt-(--safe-area-top) data-[side=left]:pb-(--safe-area-bottom) data-[side=left]:pl-(--safe-area-left) data-[side=left]:data-[ending-style]:-translate-x-full data-[side=right]:inset-y-0 data-[side=right]:right-0 data-[side=right]:h-full data-[side=right]:w-3/4 data-[side=right]:rounded-l-[var(--radius-overlay)] data-[side=right]:rounded-r-none data-[side=right]:pt-(--safe-area-top) data-[side=right]:pr-(--safe-area-right) data-[side=right]:pb-(--safe-area-bottom) data-[side=right]:data-[ending-style]:translate-x-full data-[side=top]:inset-x-0 data-[side=top]:top-0 data-[side=top]:h-auto data-[side=top]:max-h-[calc(100dvh-var(--safe-area-bottom))] data-[side=top]:rounded-t-none data-[side=top]:rounded-b-[var(--radius-overlay)] data-[side=top]:pt-(--safe-area-top) data-[side=top]:pr-(--safe-area-right) data-[side=top]:pl-(--safe-area-left) data-[side=top]:data-[ending-style]:-translate-y-full data-[starting-style]:opacity-0 data-[side=bottom]:data-[starting-style]:translate-y-full data-[side=left]:data-[starting-style]:-translate-x-full data-[side=right]:data-[starting-style]:translate-x-full data-[side=top]:data-[starting-style]:-translate-y-full sm:max-w-sm',
      className,
    )}
    {...restProps}
  >
    {@render children?.()}
    {#if showCloseButton}
      <SheetPrimitive.Close data-slot="sheet-close">
        {#snippet child({ props })}
          <Button
            variant="ghost"
            class="absolute top-3 right-3 in-data-[side=bottom]:right-[calc(var(--safe-area-right)+0.75rem)] in-data-[side=left]:top-[calc(var(--safe-area-top)+0.75rem)] in-data-[side=right]:top-[calc(var(--safe-area-top)+0.75rem)] in-data-[side=right]:right-[calc(var(--safe-area-right)+0.75rem)] in-data-[side=top]:top-[calc(var(--safe-area-top)+0.75rem)] in-data-[side=top]:right-[calc(var(--safe-area-right)+0.75rem)]"
            size="icon-sm"
            {...props}
          >
            <Icon name="x" class="size-4" />
            <span class="sr-only">{m.action_close()}</span>
          </Button>
        {/snippet}
      </SheetPrimitive.Close>
    {/if}
  </SheetPrimitive.Content>
</SheetPortal>
