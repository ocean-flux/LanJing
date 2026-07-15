<script lang="ts">
  import { cn } from '$lib/utils.js';

  let {
    class: className,
    ...rest
  }: {
    class?: string;
  } & import('svelte/elements').HTMLLabelAttributes = $props();
</script>

<label
  class={cn(
    'text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70',
    className,
  )}
  {...rest}
>
  {@render children?.()}
</label>

{#snippet children()}{/snippet}
