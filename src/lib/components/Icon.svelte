<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements';
  import { cn } from '$lib/utils';

  export type IconName =
    | 'arrow-clockwise'
    | 'arrow-right'
    | 'broadcast'
    | 'check-circle'
    | 'compass'
    | 'database'
    | 'gear-six'
    | 'monitor'
    | 'moon'
    | 'plus'
    | 'push-pin'
    | 'push-pin-fill'
    | 'squares-four'
    | 'star'
    | 'star-fill'
    | 'sun'
    | 'translate'
    | 'warning-circle'
    | 'x';

  type Props = Omit<HTMLAttributes<HTMLSpanElement>, 'children'> & {
    name: IconName;
    label?: string;
  };

  const iconClasses: Record<IconName, string> = {
    'arrow-clockwise': 'icon-[ph--arrow-clockwise]',
    'arrow-right': 'icon-[ph--arrow-right]',
    broadcast: 'icon-[ph--broadcast]',
    'check-circle': 'icon-[ph--check-circle]',
    compass: 'icon-[ph--compass]',
    database: 'icon-[ph--database]',
    'gear-six': 'icon-[ph--gear-six]',
    monitor: 'icon-[ph--monitor]',
    moon: 'icon-[ph--moon]',
    plus: 'icon-[ph--plus]',
    'push-pin': 'icon-[ph--push-pin]',
    'push-pin-fill': 'icon-[ph--push-pin-fill]',
    'squares-four': 'icon-[ph--squares-four]',
    star: 'icon-[ph--star]',
    'star-fill': 'icon-[ph--star-fill]',
    sun: 'icon-[ph--sun]',
    translate: 'icon-[ph--translate]',
    'warning-circle': 'icon-[ph--warning-circle]',
    x: 'icon-[ph--x]',
  };

  let { name, label, class: className, ...restProps }: Props = $props();
</script>

<span
  class={cn('inline-block size-[1em] shrink-0 align-[-0.125em]', iconClasses[name], className)}
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
  {...restProps}
></span>
