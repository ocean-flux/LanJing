<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements';
  import { cn, type WithElementRef } from '$lib/utils.js';

  let {
    ref = $bindable(null),
    class: className,
    children,
    size = 'default',
    ...restProps
  }: WithElementRef<HTMLAttributes<HTMLDivElement>> & { size?: 'default' | 'sm' } = $props();
</script>

<!-- double-bezel 面板：外环 hairline + 内芯 surface + inset highlight -->
<div
  bind:this={ref}
  data-slot="card"
  data-size={size}
  class={cn(
    'double-bezel group/card text-card-foreground flex flex-col gap-4 overflow-hidden py-4 text-sm has-data-[slot=card-footer]:pb-0 has-[>img:first-child]:pt-0 data-[size=sm]:gap-3 data-[size=sm]:py-3 data-[size=sm]:has-data-[slot=card-footer]:pb-0 *:[img:first-child]:rounded-t-[calc(var(--radius-xl)-1px)] *:[img:last-child]:rounded-b-[calc(var(--radius-xl)-1px)]',
    className,
  )}
  {...restProps}
>
  {@render children?.()}
</div>
