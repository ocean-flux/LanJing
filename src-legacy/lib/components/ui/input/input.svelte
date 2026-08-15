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
  function attachRef(element: HTMLInputElement) {
    ref = element;
    return () => {
      if (ref === element) ref = null;
    };
  }
</script>

{#if type === 'file'}
  <input
    {@attach attachRef}
    data-slot={dataSlot}
    class={cn(
      'glass-control h-(--density-control-md) w-full min-w-0 rounded-md border border-hairline px-2.5 py-1 text-base text-ink transition-[color,background-color,border-color,box-shadow] duration-(--motion-fast) outline-none file:inline-flex file:h-(--density-control-sm) file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground placeholder:text-muted-foreground read-only:bg-surface-2/70 focus-visible:border-lantern-strong/60 focus-visible:shadow-[var(--focus-ring)] disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 aria-busy:cursor-wait aria-busy:opacity-70 aria-invalid:border-destructive aria-invalid:ring-2 aria-invalid:ring-destructive/25 md:text-sm dark:disabled:bg-surface-2/80 dark:aria-invalid:ring-destructive/40 [&[type=file]]:py-0.5 [@media(pointer:coarse)]:min-h-(--density-touch-target)',
      className,
    )}
    type="file"
    bind:files
    {...restProps}
  />
{:else}
  <input
    {@attach attachRef}
    data-slot={dataSlot}
    class={cn(
      'glass-control h-(--density-control-md) w-full min-w-0 rounded-md border border-hairline px-2.5 py-1 text-base text-ink transition-[color,background-color,border-color,box-shadow] duration-(--motion-fast) outline-none file:inline-flex file:h-(--density-control-sm) file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground placeholder:text-muted-foreground read-only:bg-surface-2/70 focus-visible:border-lantern-strong/60 focus-visible:shadow-[var(--focus-ring)] disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 aria-busy:cursor-wait aria-busy:opacity-70 aria-invalid:border-destructive aria-invalid:ring-2 aria-invalid:ring-destructive/25 md:text-sm dark:disabled:bg-surface-2/80 dark:aria-invalid:ring-destructive/40 [&[type=file]]:py-0.5 [@media(pointer:coarse)]:min-h-(--density-touch-target)',
      className,
    )}
    {type}
    bind:value
    {...restProps}
  />
{/if}
