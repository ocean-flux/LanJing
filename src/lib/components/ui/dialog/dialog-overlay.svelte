<script lang="ts">
  import { Dialog as DialogPrimitive } from 'bits-ui';
  import { cn } from '$lib/utils.js';

  let {
    ref = $bindable(null),
    class: className,
    ...restProps
  }: DialogPrimitive.OverlayProps = $props();
</script>

<DialogPrimitive.Overlay
  bind:ref
  data-slot="dialog-overlay"
  class={cn(
    'fixed inset-0 isolate z-(--layer-scrim) bg-[var(--surface-scrim)] transition-opacity duration-(--motion-fast) data-[ending-style]:opacity-0 data-[starting-style]:opacity-0',
    className,
  )}
  {...restProps}
/>
