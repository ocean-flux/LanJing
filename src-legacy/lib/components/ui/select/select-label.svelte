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
  data-slot="select-label"
  class={cn('px-2 py-1 text-xs font-medium text-muted-foreground', className)}
  {...restProps}
>
  {@render children?.()}
</div>
