<script lang="ts">
  import { cn, type WithElementRef } from '$lib/utils.js';
  import type { HTMLAttributes } from 'svelte/elements';

  let {
    ref = $bindable(null),
    class: className,
    children,
    ...restProps
  }: WithElementRef<HTMLAttributes<HTMLDivElement>> = $props();
</script>

<div
  bind:this={ref}
  data-slot="card-footer"
  class={cn(
    'flex items-center rounded-b-[calc(var(--radius-xl)-1px)] border-t border-hairline bg-muted/50 p-4 group-data-[size=sm]/card:p-3',
    className,
  )}
  {...restProps}
>
  {@render children?.()}
</div>
