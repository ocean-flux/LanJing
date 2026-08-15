<script lang="ts" module>
  import type { Pathname } from '$app/types';
  import { cn, type WithElementRef } from '$lib/utils.js';
  import type { HTMLAnchorAttributes, HTMLButtonAttributes } from 'svelte/elements';
  import { type VariantProps, tv } from 'tailwind-variants';

  /** 主 CTA 使用 lantern 强调；outline/secondary 保持单层边框与表面。 */
  export const buttonVariants = tv({
    base: "group/button inline-flex shrink-0 items-center justify-center rounded-md border border-transparent bg-clip-padding text-sm font-medium whitespace-nowrap transition-[color,background-color,border-color,box-shadow,transform] duration-(--motion-fast) outline-none select-none focus-visible:border-lantern-strong/60 focus-visible:shadow-[var(--focus-ring)] active:not-aria-[haspopup]:translate-y-px disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 aria-busy:cursor-wait aria-busy:opacity-70 aria-disabled:pointer-events-none aria-disabled:cursor-not-allowed aria-disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-2 aria-invalid:ring-destructive/25 dark:aria-invalid:ring-destructive/40 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [@media(pointer:coarse)]:min-h-(--density-touch-target) [@media(pointer:coarse)]:min-w-(--density-touch-target)",
    variants: {
      variant: {
        default:
          'border-lantern-strong/40 bg-lantern-strong text-on-lantern hover:bg-lantern-hover [a]:hover:bg-lantern-hover',
        outline:
          'glass-control border-hairline text-ink hover:bg-surface-2 aria-expanded:bg-lantern-soft aria-expanded:text-ink',
        secondary:
          'border-hairline bg-surface-2 text-ink hover:bg-surface-3 aria-expanded:bg-surface-3 aria-expanded:text-ink',
        ghost:
          'text-ink hover:bg-lantern-soft hover:text-ink aria-expanded:bg-lantern-soft aria-expanded:text-ink',
        destructive:
          'border-destructive/25 bg-destructive/10 text-destructive hover:bg-destructive/20 focus-visible:border-destructive/50 focus-visible:ring-destructive/20 dark:bg-destructive/20 dark:hover:bg-destructive/30 dark:focus-visible:ring-destructive/40',
        link: 'text-lantern-strong underline-offset-4 hover:underline',
      },
      size: {
        default:
          'h-(--density-control-md) gap-1.5 px-2.5 has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2',
        xs: "h-(--density-control-sm) gap-1 rounded-md px-2 text-xs in-data-[slot=button-group]:rounded-lg has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&_svg:not([class*='size-'])]:size-3",
        sm: "h-(--density-control-sm) gap-1 rounded-md px-2.5 text-[0.8rem] in-data-[slot=button-group]:rounded-lg has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&_svg:not([class*='size-'])]:size-3.5",
        lg: 'h-(--density-control-lg) gap-1.5 rounded-md px-3 has-data-[icon=inline-end]:pr-2.5 has-data-[icon=inline-start]:pl-2.5',
        icon: 'size-(--density-control-md)',
        'icon-xs':
          "size-(--density-control-sm) rounded-md in-data-[slot=button-group]:rounded-lg [&_svg:not([class*='size-'])]:size-3",
        'icon-sm': 'size-(--density-control-sm) rounded-md in-data-[slot=button-group]:rounded-lg',
        'icon-lg': 'size-(--density-control-lg)',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  });

  export type ButtonVariant = VariantProps<typeof buttonVariants>['variant'];
  export type ButtonSize = VariantProps<typeof buttonVariants>['size'];

  export type ButtonProps = WithElementRef<HTMLButtonAttributes> &
    WithElementRef<HTMLAnchorAttributes> & {
      variant?: ButtonVariant;
      size?: ButtonSize;
      href?: Pathname | '';
    };
</script>

<script lang="ts">
  import { resolve } from '$app/paths';
  let {
    class: className,
    variant = 'default',
    size = 'default',
    ref = $bindable(null),
    href = undefined,
    type = 'button',
    disabled,
    onclick,
    'aria-disabled': ariaDisabled,
    role,
    tabindex,
    children,
    ...restProps
  }: ButtonProps = $props();

  function attachRef(element: HTMLAnchorElement | HTMLButtonElement) {
    ref = element;
    return () => {
      if (ref === element) ref = null;
    };
  }

  function handleAnchorClick(event: MouseEvent) {
    if (disabled) {
      event.preventDefault();
      event.stopImmediatePropagation();
      return;
    }

    onclick?.(event as MouseEvent & { currentTarget: HTMLAnchorElement });
  }
</script>

{#if href != null}
  <a
    {@attach attachRef}
    data-slot="button"
    class={cn(buttonVariants({ variant, size }), className)}
    {...restProps}
    href={disabled ? undefined : href === '' ? href : resolve(href)}
    aria-disabled={disabled ? true : ariaDisabled}
    role={disabled ? 'link' : role}
    tabindex={disabled ? -1 : tabindex}
    onclick={handleAnchorClick}
  >
    {@render children?.()}
  </a>
{:else}
  <button
    {@attach attachRef}
    data-slot="button"
    class={cn(buttonVariants({ variant, size }), className)}
    {...restProps}
    {type}
    {disabled}
    {onclick}
    aria-disabled={ariaDisabled}
    {role}
    {tabindex}
  >
    {@render children?.()}
  </button>
{/if}
