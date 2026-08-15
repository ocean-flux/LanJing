<script lang="ts">
  import { cn, type WithElementRef } from '$lib/utils.js';
  import type { HTMLAttributes } from 'svelte/elements';
  import { Dialog as DialogPrimitive } from 'bits-ui';
  import { Button } from '$lib/components/ui/button/index.js';
  import { m } from '$lib/i18n';

  let {
    ref = $bindable(null),
    class: className,
    children,
    showCloseButton = false,
    ...restProps
  }: WithElementRef<HTMLAttributes<HTMLDivElement>> & {
    showCloseButton?: boolean;
  } = $props();
  function attachRef(element: HTMLDivElement) {
    ref = element;
    return () => {
      if (ref === element) ref = null;
    };
  }
</script>

<div
  {@attach attachRef}
  data-slot="dialog-footer"
  class={cn(
    '-mx-4 -mb-4 flex flex-col-reverse gap-2 rounded-b-[calc(var(--radius-overlay)-1px)] border-t border-hairline bg-surface-2/70 p-3 sm:flex-row sm:justify-end',
    className,
  )}
  {...restProps}
>
  {@render children?.()}
  {#if showCloseButton}
    <DialogPrimitive.Close>
      {#snippet child({ props })}
        <Button variant="outline" {...props}>{m.action_close()}</Button>
      {/snippet}
    </DialogPrimitive.Close>
  {/if}
</div>
