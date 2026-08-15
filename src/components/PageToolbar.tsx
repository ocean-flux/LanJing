import type { ReactNode } from 'react';
import { cn } from '@/shared/utils';

type PageToolbarProps = {
  /** 左侧计数或状态，以等宽字呈现。 */
  meta?: ReactNode;
  /** 中间可放搜索等常驻控件。 */
  children?: ReactNode;
  /** 右侧动作区。 */
  actions?: ReactNode;
  className?: string;
};

/**
 * 页面工具条：单行 40px，吸顶，细线分隔。
 * 页标题由 Titlebar 的 h1 统一承担，这里不再重复出现标题，
 * 只放计数、常驻控件与动作；页面没有这些时不渲染本组件。
 */
export function PageToolbar({ meta, children, actions, className }: PageToolbarProps) {
  return (
    <div
      className={cn(
        'sticky top-0 z-(--layer-chrome) flex h-(--shell-toolbar-height) items-center gap-2 border-b border-hairline bg-canvas/90 px-(--page-gutter) backdrop-blur-sm',
        className,
      )}
    >
      {meta ? <span className="shrink-0 font-mono text-ui-sm text-ink-subtle">{meta}</span> : null}
      {children}
      {actions ? <div className="ml-auto flex items-center gap-1">{actions}</div> : null}
    </div>
  );
}
