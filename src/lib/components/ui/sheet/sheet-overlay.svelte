<script lang="ts">
  import { Dialog as SheetPrimitive } from 'bits-ui';
  import { cn } from '$lib/utils.js';

  let {
    ref = $bindable(null),
    class: className,
    ...restProps
  }: SheetPrimitive.OverlayProps = $props();
</script>

<SheetPrimitive.Overlay
  bind:ref
  data-slot="sheet-overlay"
  class={cn(
    'fixed inset-0 z-(--layer-scrim) bg-[var(--surface-scrim)] transition-opacity duration-(--motion-fast) data-[ending-style]:opacity-0 data-[starting-style]:opacity-0',
    className,
  )}
  {...restProps}
/>
