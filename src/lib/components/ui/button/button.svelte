<script lang="ts" module>
  import { cn, type WithElementRef } from '$lib/utils.js';
  import type { HTMLAnchorAttributes, HTMLButtonAttributes } from 'svelte/elements';
  import { type VariantProps, tv } from 'tailwind-variants';

  /** 主 CTA 使用 lantern 强调；outline/secondary 保持单层边框与表面。 */
  export const buttonVariants = tv({
    base: "group/button inline-flex shrink-0 items-center justify-center rounded-lg border border-transparent bg-clip-padding text-sm font-medium whitespace-nowrap transition-all outline-none select-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 active:not-aria-[haspopup]:translate-y-px disabled:pointer-events-none disabled:opacity-50 aria-disabled:pointer-events-none aria-disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [@media(pointer:coarse)]:min-h-11 [@media(pointer:coarse)]:min-w-11",
    variants: {
      variant: {
        default:
          'border-lantern-strong/40 bg-lantern-strong text-on-lantern shadow-[inset_0_1px_0_color-mix(in_oklab,var(--on-lantern)_16%,transparent),0_0_0_1px_color-mix(in_oklab,var(--lantern)_20%,transparent)] hover:bg-lantern-hover [a]:hover:bg-lantern-hover',
        outline:
          'border-hairline bg-surface-1 text-ink shadow-sm hover:bg-lantern-soft hover:text-ink aria-expanded:bg-lantern-soft aria-expanded:text-ink dark:bg-surface-2/80 dark:hover:bg-surface-3',
        secondary:
          'border-hairline bg-surface-2 text-ink shadow-sm hover:bg-surface-3 aria-expanded:bg-surface-3 aria-expanded:text-ink',
        ghost:
          'hover:bg-lantern-soft hover:text-ink aria-expanded:bg-lantern-soft aria-expanded:text-ink dark:hover:bg-lantern-soft/50',
        destructive:
          'bg-destructive/10 text-destructive hover:bg-destructive/20 focus-visible:border-destructive/40 focus-visible:ring-destructive/20 dark:bg-destructive/20 dark:hover:bg-destructive/30 dark:focus-visible:ring-destructive/40',
        link: 'text-lantern-strong underline-offset-4 hover:underline',
      },
      size: {
        default:
          'h-8 gap-1.5 px-2.5 has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2',
        xs: "h-6 gap-1 rounded-md px-2 text-xs in-data-[slot=button-group]:rounded-lg has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&_svg:not([class*='size-'])]:size-3",
        sm: "h-7 gap-1 rounded-md px-2.5 text-[0.8rem] in-data-[slot=button-group]:rounded-lg has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&_svg:not([class*='size-'])]:size-3.5",
        lg: 'h-9 gap-1.5 rounded-lg px-3 has-data-[icon=inline-end]:pr-2.5 has-data-[icon=inline-start]:pl-2.5',
        icon: 'size-8',
        'icon-xs':
          "size-6 rounded-md in-data-[slot=button-group]:rounded-lg [&_svg:not([class*='size-'])]:size-3",
        'icon-sm': 'size-7 rounded-md in-data-[slot=button-group]:rounded-lg',
        'icon-lg': 'size-9',
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
    };
</script>

<script lang="ts">
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

  function handleAnchorClick(event: MouseEvent) {
    if (disabled) {
      event.preventDefault();
      event.stopImmediatePropagation();
      return;
    }

    onclick?.(event as MouseEvent & { currentTarget: HTMLAnchorElement });
  }
</script>

<!-- eslint-disable svelte/no-navigation-without-resolve -->
{#if href != null}
  <a
    bind:this={ref}
    data-slot="button"
    class={cn(buttonVariants({ variant, size }), className)}
    {...restProps}
    href={disabled ? undefined : href}
    aria-disabled={disabled ? true : ariaDisabled}
    role={disabled ? 'link' : role}
    tabindex={disabled ? -1 : tabindex}
    onclick={handleAnchorClick}
  >
    {@render children?.()}
  </a>
{:else}
  <button
    bind:this={ref}
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
