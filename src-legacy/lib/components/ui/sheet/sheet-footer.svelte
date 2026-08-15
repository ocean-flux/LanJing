<script lang="ts">
  import { cn, type WithElementRef } from '$lib/utils.js';
  import type { HTMLAttributes } from 'svelte/elements';

  let {
    ref = $bindable(null),
    class: className,
    children,
    ...restProps
  }: WithElementRef<HTMLAttributes<HTMLDivElement>> = $props();
  function attachRef(element: HTMLDivElement) {
    ref = element;
    return () => {
      if (ref === element) ref = null;
    };
  }
</script>

<div
  {@attach attachRef}
  data-slot="sheet-footer"
  class={cn('mt-auto flex flex-col gap-2 border-t border-hairline p-3', className)}
  {...restProps}
>
  {@render children?.()}
</div>
