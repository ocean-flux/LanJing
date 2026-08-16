//! 循环域覆盖层：从扁平语义图推导的 Loop region 可视边界。
//!
//! 只是投影，不写 Definition。渲染在 `ViewportPortal target="back"`，
//! 随画布缩放平移，且不遮挡节点与边。

import { Icon } from '@/components/Icon';
import { cn } from '@/shared/utils';
import { useMessages } from '@/shared/i18n/messages';
import type { LoopRegionView } from './model/flow-adapter';

export type LoopRegionBounds = {
  left: number;
  top: number;
  width: number;
  height: number;
};

export function LoopRegionOverlay({
  region,
  bounds,
  selected,
  onSelect,
  onToggle,
}: {
  region: LoopRegionView;
  bounds: LoopRegionBounds;
  selected: boolean;
  onSelect: () => void;
  onToggle: () => void;
}) {
  const m = useMessages();
  const statusText =
    region.status === 'valid' ? m.rules_loop_region_valid() : m.rules_loop_region_invalid();

  return (
    <div
      className={cn('flow-loop-region', selected && 'flow-loop-region-selected')}
      data-status={region.status}
      data-collapsed={region.collapsed}
      data-selected={selected}
      style={{
        left: `${bounds.left}px`,
        top: `${bounds.top}px`,
        width: `${bounds.width}px`,
        height: `${bounds.height}px`,
      }}
      aria-hidden="true"
    >
      <div className="flow-loop-region-label pointer-events-auto flex items-center gap-1.5">
        <button
          type="button"
          className="flex items-center gap-1.5 outline-none focus-visible:ring-1 focus-visible:ring-ring"
          aria-pressed={selected}
          aria-label={m.rules_loop_region_select({ id: region.loopNodeId })}
          onClick={(event) => {
            event.stopPropagation();
            onSelect();
          }}
        >
          <Icon name="arrow-counter-clockwise" className="text-[0.75rem]" />
          <span>{m.rules_loop_region_title()}</span>
          <span className="opacity-70">{statusText}</span>
        </button>
        <button
          type="button"
          className="outline-none focus-visible:ring-1 focus-visible:ring-ring"
          aria-label={
            region.collapsed ? m.rules_loop_region_expand() : m.rules_loop_region_collapse()
          }
          title={region.collapsed ? m.rules_loop_region_expand() : m.rules_loop_region_collapse()}
          onClick={(event) => {
            event.stopPropagation();
            onToggle();
          }}
        >
          <Icon name={region.collapsed ? 'caret-down' : 'caret-up'} className="text-[0.75rem]" />
        </button>
      </div>
    </div>
  );
}
