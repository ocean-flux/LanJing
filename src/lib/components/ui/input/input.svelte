<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';
  import { cn, type WithElementRef } from '$lib/utils.js';

  // 排除 file 分支的标准输入类型，避免宽泛字符串回退重新接纳 file。
  type InputType =
    | 'button'
    | 'checkbox'
    | 'color'
    | 'date'
    | 'datetime-local'
    | 'email'
    | 'hidden'
    | 'image'
    | 'month'
    | 'number'
    | 'password'
    | 'radio'
    | 'range'
    | 'reset'
    | 'search'
    | 'submit'
    | 'tel'
    | 'text'
    | 'time'
    | 'url'
    | 'week';

  type Props = WithElementRef<
    | (Omit<HTMLInputAttributes, 'type' | 'value'> & {
        type: 'file';
        files?: FileList;
        value?: never;
      })
    | (Omit<HTMLInputAttributes, 'type'> & {
        type?: InputType;
        files?: undefined;
      })
  >;

  let {
    ref = $bindable(null),
    value = $bindable(),
    type,
    files = $bindable(),
    class: className,
    'data-slot': dataSlot = 'input',
    ...restProps
  }: Props = $props();
</script>

{#if type === 'file'}
  <input
    bind:this={ref}
    data-slot={dataSlot}
    class={cn(
      'glass-control h-8 w-full min-w-0 rounded-lg border border-hairline px-2.5 py-1 text-base text-ink transition-colors outline-none file:inline-flex file:h-6 file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground placeholder:text-muted-foreground focus-visible:border-lantern-strong/50 focus-visible:ring-3 focus-visible:ring-lantern/35 disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 md:text-sm dark:disabled:bg-surface-2/80 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 [@media(pointer:coarse)]:min-h-11',
      className,
    )}
    type="file"
    bind:files
    {...restProps}
  />
{:else}
  <input
    bind:this={ref}
    data-slot={dataSlot}
    class={cn(
      'glass-control h-8 w-full min-w-0 rounded-lg border border-hairline px-2.5 py-1 text-base text-ink transition-colors outline-none file:inline-flex file:h-6 file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground placeholder:text-muted-foreground focus-visible:border-lantern-strong/50 focus-visible:ring-3 focus-visible:ring-lantern/35 disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 md:text-sm dark:disabled:bg-surface-2/80 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 [@media(pointer:coarse)]:min-h-11',
      className,
    )}
    {type}
    bind:value
    {...restProps}
  />
{/if}
