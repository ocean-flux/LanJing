<script lang="ts">
  import { cn } from '$lib/utils.js';
  let {
    class: className,
    value = $bindable(),
    ...rest
  }: {
    class?: string;
    value?: string;
  } & Omit<import('svelte/elements').HTMLTextareaAttributes, 'value'> = $props();
</script>

<textarea
  data-slot="textarea"
  bind:value
  class={cn(
    'glass-control flex min-h-[calc(var(--density-control-lg)*2)] w-full rounded-md border border-hairline px-2.5 py-2 text-sm text-ink transition-[color,background-color,border-color,box-shadow] duration-(--motion-fast) outline-none placeholder:text-muted-foreground read-only:bg-surface-2/70 focus-visible:border-lantern-strong/60 focus-visible:shadow-[var(--focus-ring)] disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 aria-busy:cursor-wait aria-busy:opacity-70 aria-invalid:border-destructive aria-invalid:ring-2 aria-invalid:ring-destructive/25 dark:aria-invalid:ring-destructive/40',
    className,
  )}
  {...rest}></textarea>
