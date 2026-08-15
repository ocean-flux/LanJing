<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements';
  import { cn, type WithElementRef } from '$lib/utils.js';

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
  data-slot="sheet-header"
  class={cn('flex flex-col gap-1 p-3', className)}
  {...restProps}
>
  {@render children?.()}
</div>
