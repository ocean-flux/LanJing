import type { ComponentProps } from 'react';
import { cn } from '@/shared/utils';

/**
 * 全站唯一图标出口。图标经由 Iconify 的 Tailwind 4 插件在构建期内联为 mask，
 * 不进运行时依赖。
 *
 * 约束：Tailwind 静态扫描源码，`icon-[ph--${name}]` 这类模板拼接不会被识别，
 * 因此下表必须是字面量白名单。新增图标时同时补 IconName 与 ICON_CLASSES。
 */
export type IconName =
  | 'arrow-clockwise'
  | 'arrow-counter-clockwise'
  | 'arrow-left'
  | 'arrow-right'
  | 'arrow-square-out'
  | 'book-open'
  | 'books'
  | 'brackets-curly'
  | 'broadcast'
  | 'caret-down'
  | 'caret-left'
  | 'caret-right'
  | 'caret-up'
  | 'check'
  | 'check-circle'
  | 'code'
  | 'compass'
  | 'copy'
  | 'database'
  | 'download-simple'
  | 'eye'
  | 'file-plus'
  | 'file-text'
  | 'film-slate'
  | 'floppy-disk'
  | 'gear-six'
  | 'git-merge'
  | 'image'
  | 'info'
  | 'keyboard'
  | 'list-bullets'
  | 'lock'
  | 'magnifying-glass'
  | 'microphone-stage'
  | 'minus'
  | 'monitor'
  | 'moon'
  | 'music-notes'
  | 'pencil-simple'
  | 'play'
  | 'plus'
  | 'push-pin'
  | 'push-pin-fill'
  | 'shield-check'
  | 'sidebar'
  | 'spinner'
  | 'square'
  | 'squares-four'
  | 'star'
  | 'star-fill'
  | 'stop-circle'
  | 'sun'
  | 'translate'
  | 'trash'
  | 'tree-structure'
  | 'upload-simple'
  | 'warning'
  | 'warning-circle'
  | 'x'
  | 'x-circle';

const ICON_CLASSES: Record<IconName, string> = {
  'arrow-clockwise': 'icon-[ph--arrow-clockwise]',
  'arrow-counter-clockwise': 'icon-[ph--arrow-counter-clockwise]',
  'arrow-left': 'icon-[ph--arrow-left]',
  'arrow-right': 'icon-[ph--arrow-right]',
  'arrow-square-out': 'icon-[ph--arrow-square-out]',
  'book-open': 'icon-[ph--book-open]',
  books: 'icon-[ph--books]',
  'brackets-curly': 'icon-[ph--brackets-curly]',
  broadcast: 'icon-[ph--broadcast]',
  'caret-down': 'icon-[ph--caret-down]',
  'caret-left': 'icon-[ph--caret-left]',
  'caret-right': 'icon-[ph--caret-right]',
  'caret-up': 'icon-[ph--caret-up]',
  check: 'icon-[ph--check]',
  'check-circle': 'icon-[ph--check-circle]',
  code: 'icon-[ph--code]',
  compass: 'icon-[ph--compass]',
  copy: 'icon-[ph--copy]',
  database: 'icon-[ph--database]',
  'download-simple': 'icon-[ph--download-simple]',
  eye: 'icon-[ph--eye]',
  'file-plus': 'icon-[ph--file-plus]',
  'file-text': 'icon-[ph--file-text]',
  'film-slate': 'icon-[ph--film-slate]',
  'floppy-disk': 'icon-[ph--floppy-disk]',
  'gear-six': 'icon-[ph--gear-six]',
  'git-merge': 'icon-[ph--git-merge]',
  image: 'icon-[ph--image]',
  info: 'icon-[ph--info]',
  keyboard: 'icon-[ph--keyboard]',
  'list-bullets': 'icon-[ph--list-bullets]',
  lock: 'icon-[ph--lock]',
  'magnifying-glass': 'icon-[ph--magnifying-glass]',
  'microphone-stage': 'icon-[ph--microphone-stage]',
  minus: 'icon-[ph--minus]',
  monitor: 'icon-[ph--monitor]',
  moon: 'icon-[ph--moon]',
  'music-notes': 'icon-[ph--music-notes]',
  'pencil-simple': 'icon-[ph--pencil-simple]',
  play: 'icon-[ph--play]',
  plus: 'icon-[ph--plus]',
  'push-pin': 'icon-[ph--push-pin]',
  'push-pin-fill': 'icon-[ph--push-pin-fill]',
  'shield-check': 'icon-[ph--shield-check]',
  sidebar: 'icon-[ph--sidebar-simple]',
  spinner: 'icon-[ph--spinner]',
  square: 'icon-[ph--square]',
  'squares-four': 'icon-[ph--squares-four]',
  star: 'icon-[ph--star]',
  'star-fill': 'icon-[ph--star-fill]',
  'stop-circle': 'icon-[ph--stop-circle]',
  sun: 'icon-[ph--sun]',
  translate: 'icon-[ph--translate]',
  trash: 'icon-[ph--trash]',
  'tree-structure': 'icon-[ph--tree-structure]',
  'upload-simple': 'icon-[ph--upload-simple]',
  warning: 'icon-[ph--warning]',
  'warning-circle': 'icon-[ph--warning-circle]',
  x: 'icon-[ph--x]',
  'x-circle': 'icon-[ph--x-circle]',
};

export type IconProps = Omit<ComponentProps<'span'>, 'children'> & {
  name: IconName;
  /** 传入则渲染为 role="img" 并带无障碍名；不传则视为装饰性图标。 */
  label?: string;
};

export function Icon({ name, label, className, ...restProps }: IconProps) {
  return (
    <span
      className={cn(
        'inline-block size-[1em] shrink-0 align-[-0.125em]',
        ICON_CLASSES[name],
        className,
      )}
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : 'true'}
      {...restProps}
    />
  );
}

/** 白名单名字的运行时集合；descriptor 声明据此校验，避免拼出未登记的 class。 */
export const ICON_NAMES: readonly IconName[] = Object.keys(ICON_CLASSES) as IconName[];
