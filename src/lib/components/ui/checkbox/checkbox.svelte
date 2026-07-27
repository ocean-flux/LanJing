<script lang="ts">
  import { Checkbox as CheckboxPrimitive } from 'bits-ui';
  import { cn, type WithoutChildrenOrChild } from '$lib/utils.js';
  import Icon from '$lib/components/Icon.svelte';

  let {
    ref = $bindable(null),
    checked = $bindable(false),
    indeterminate = $bindable(false),
    class: className,
    ...restProps
  }: WithoutChildrenOrChild<CheckboxPrimitive.RootProps> = $props();
</script>

<CheckboxPrimitive.Root
  bind:ref
  data-slot="checkbox"
  class={cn(
    'peer relative flex size-4 shrink-0 items-center justify-center rounded-sm border border-input transition-[color,background-color,border-color,box-shadow] duration-(--motion-fast) outline-none group-has-disabled/field:opacity-50 after:absolute after:-inset-1.5 focus-visible:border-lantern-strong/60 focus-visible:shadow-[var(--focus-ring)] disabled:cursor-not-allowed disabled:opacity-50 aria-busy:cursor-wait aria-busy:opacity-70 aria-invalid:border-destructive aria-invalid:ring-2 aria-invalid:ring-destructive/25 aria-invalid:aria-checked:border-primary data-checked:border-primary data-checked:bg-primary data-checked:text-primary-foreground dark:bg-input/30 dark:aria-invalid:ring-destructive/40 [@media(pointer:coarse)]:after:-inset-3.5',
    className,
  )}
  bind:checked
  bind:indeterminate
  {...restProps}
>
  {#snippet children({ checked, indeterminate })}
    <div
      data-slot="checkbox-indicator"
      class="grid place-content-center text-current transition-none"
    >
      {#if checked}
        <Icon name="check" class="size-3.5" />
      {:else if indeterminate}
        <Icon name="minus" class="size-3.5" />
      {/if}
    </div>
  {/snippet}
</CheckboxPrimitive.Root>
