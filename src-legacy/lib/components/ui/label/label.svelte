<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLLabelAttributes } from 'svelte/elements';
  import { cn } from '$lib/utils.js';

  type Props = Omit<HTMLLabelAttributes, 'children'> & {
    children?: Snippet;
  };

  let { class: className, children, ...rest }: Props = $props();
</script>

<label
  class={cn(
    'text-sm leading-none font-medium peer-disabled:cursor-not-allowed peer-disabled:opacity-70',
    className,
  )}
  {...rest}
>
  {@render children?.()}
</label>
