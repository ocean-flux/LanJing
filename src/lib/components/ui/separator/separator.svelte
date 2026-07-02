<script lang="ts">
  import { cn } from '$lib/utils.js';
  import { Separator as SeparatorPrimitive } from 'bits-ui';
  let {
    class: className,
    orientation = 'horizontal',
    decorative = true,
    ...rest
  }: {
    class?: string;
    orientation?: 'horizontal' | 'vertical';
    decorative?: boolean;
  } = $props();
</script>

<SeparatorPrimitive.Root
  {decorative}
  {orientation}
  class={cn(
    'shrink-0 bg-border',
    orientation === 'horizontal' ? 'h-[1px] w-full' : 'h-full w-[1px]',
    className,
  )}
  {...rest}
/>
