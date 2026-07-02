<script lang="ts">
  import { Toaster } from 'svelte-sonner';
  import { cn } from '$lib/utils.js';

  let {
    class: className,
    theme = 'system',
    ...rest
  }: {
    class?: string;
    theme?: 'light' | 'dark' | 'system';
  } = $props();
</script>

<Toaster class={cn(className)} {theme} richColors closeButton {...rest} />
