<script lang="ts">
  import { Dialog as DialogPrimitive } from 'bits-ui';
  import DialogPortal from './dialog-portal.svelte';
  import type { Snippet } from 'svelte';
  import * as Dialog from './index.js';
  import { cn, type WithoutChildrenOrChild } from '$lib/utils.js';
  import type { ComponentProps } from 'svelte';
  import { Button } from '$lib/components/ui/button/index.js';
  import Icon from '$lib/components/Icon.svelte';
  import { m } from '$lib/i18n';

  let {
    ref = $bindable(null),
    class: className,
    portalProps,
    children,
    showCloseButton = true,
    ...restProps
  }: WithoutChildrenOrChild<DialogPrimitive.ContentProps> & {
    portalProps?: WithoutChildrenOrChild<ComponentProps<typeof DialogPortal>>;
    children: Snippet;
    showCloseButton?: boolean;
  } = $props();
</script>

<DialogPortal {...portalProps}>
  <Dialog.Overlay />
  <DialogPrimitive.Content
    bind:ref
    data-slot="dialog-content"
    class={cn(
      'elevated-overlay fixed top-1/2 left-1/2 z-(--layer-overlay) grid max-h-[calc(100dvh-var(--safe-area-top)-var(--safe-area-bottom)-2rem)] w-full max-w-[calc(100%-var(--safe-area-left)-var(--safe-area-right)-2rem)] -translate-x-1/2 -translate-y-1/2 gap-3 overflow-y-auto overscroll-contain rounded-[var(--radius-overlay)] border border-hairline p-4 text-sm text-ink transition-[opacity,transform] duration-(--motion-fast) ease-[var(--motion-standard)] outline-none data-[ending-style]:scale-95 data-[ending-style]:opacity-0 data-[starting-style]:scale-95 data-[starting-style]:opacity-0 sm:max-w-sm',
      className,
    )}
    {...restProps}
  >
    {@render children?.()}
    {#if showCloseButton}
      <DialogPrimitive.Close data-slot="dialog-close">
        {#snippet child({ props })}
          <Button variant="ghost" class="absolute top-2 right-2" size="icon-sm" {...props}>
            <Icon name="x" class="size-4" />
            <span class="sr-only">{m.action_close()}</span>
          </Button>
        {/snippet}
      </DialogPrimitive.Close>
    {/if}
  </DialogPrimitive.Content>
</DialogPortal>
