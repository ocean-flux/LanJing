<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements';
  import { cn } from '$lib/utils';

  export type IconName =
    | 'arrow-clockwise'
    | 'arrow-counter-clockwise'
    | 'arrow-left'
    | 'arrow-right'
    | 'brackets-curly'
    | 'broadcast'
    | 'caret-down'
    | 'caret-up'
    | 'check'
    | 'check-circle'
    | 'code'
    | 'compass'
    | 'copy'
    | 'database'
    | 'eye'
    | 'file-plus'
    | 'file-text'
    | 'floppy-disk'
    | 'gear-six'
    | 'git-merge'
    | 'list-bullets'
    | 'lock'
    | 'minus'
    | 'monitor'
    | 'moon'
    | 'pencil-simple'
    | 'plus'
    | 'push-pin'
    | 'push-pin-fill'
    | 'shield-check'
    | 'squares-four'
    | 'star'
    | 'star-fill'
    | 'sun'
    | 'translate'
    | 'upload-simple'
    | 'trash'
    | 'tree-structure'
    | 'warning-circle'
    | 'x';
  const iconClasses: Record<IconName, string> = {
    'arrow-clockwise': 'icon-[ph--arrow-clockwise]',
    'arrow-counter-clockwise': 'icon-[ph--arrow-counter-clockwise]',
    'arrow-left': 'icon-[ph--arrow-left]',
    'arrow-right': 'icon-[ph--arrow-right]',
    'brackets-curly': 'icon-[ph--brackets-curly]',
    broadcast: 'icon-[ph--broadcast]',
    'caret-down': 'icon-[ph--caret-down]',
    'caret-up': 'icon-[ph--caret-up]',
    check: 'icon-[ph--check]',
    'check-circle': 'icon-[ph--check-circle]',
    code: 'icon-[ph--code]',
    compass: 'icon-[ph--compass]',
    copy: 'icon-[ph--copy]',
    database: 'icon-[ph--database]',
    eye: 'icon-[ph--eye]',
    'file-plus': 'icon-[ph--file-plus]',
    'file-text': 'icon-[ph--file-text]',
    'floppy-disk': 'icon-[ph--floppy-disk]',
    'gear-six': 'icon-[ph--gear-six]',
    'git-merge': 'icon-[ph--git-merge]',
    'list-bullets': 'icon-[ph--list-bullets]',
    lock: 'icon-[ph--lock]',
    minus: 'icon-[ph--minus]',
    monitor: 'icon-[ph--monitor]',
    moon: 'icon-[ph--moon]',
    'pencil-simple': 'icon-[ph--pencil-simple]',
    plus: 'icon-[ph--plus]',
    'push-pin': 'icon-[ph--push-pin]',
    'push-pin-fill': 'icon-[ph--push-pin-fill]',
    'shield-check': 'icon-[ph--shield-check]',
    'squares-four': 'icon-[ph--squares-four]',
    star: 'icon-[ph--star]',
    'star-fill': 'icon-[ph--star-fill]',
    sun: 'icon-[ph--sun]',
    translate: 'icon-[ph--translate]',
    'upload-simple': 'icon-[ph--upload-simple]',
    trash: 'icon-[ph--trash]',
    'tree-structure': 'icon-[ph--tree-structure]',
    'warning-circle': 'icon-[ph--warning-circle]',
    x: 'icon-[ph--x]',
  };

  type Props = Omit<HTMLAttributes<HTMLSpanElement>, 'children'> & {
    name: IconName;
    label?: string;
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
