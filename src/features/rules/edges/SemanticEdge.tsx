//! 语义边：按 lane / role 区分车道，Loop 回边走独立通道。
//!
//! React 版没有 Svelte 的 `EdgeLabel`，标签要自己经 `EdgeLabelRenderer`
//! 投到共享容器里，并用 transform 定位到边中点。

import { BaseEdge, EdgeLabelRenderer, getSmoothStepPath, type EdgeProps } from '@xyflow/react';
import { useState } from 'react';
import { cn } from '@/shared/utils';
import { useMessages } from '@/shared/i18n/messages';
import type { FlowEdgeData } from '../model/flow-adapter';
import { edgeLabelText } from '../labels';

/** 不同车道的 smoothstep 让位距离，避免多条边重叠。 */
const LANE_OFFSET = { branch: 26, auxiliary: 22, loop: 18, main: 18 } as const;

/** Loop 回边绕到节点右侧的独立通道，不与正向边抢路径。 */
const LOOP_BACK_CHANNEL = 96;

function loopBackPath(
  sourceX: number,
  sourceY: number,
  targetX: number,
  targetY: number,
): [string, number, number] {
  const channelX = Math.max(sourceX, targetX) + LOOP_BACK_CHANNEL;
  return [
    `M ${sourceX} ${sourceY} L ${channelX} ${sourceY} L ${channelX} ${targetY} L ${targetX} ${targetY}`,
    channelX,
    (sourceY + targetY) / 2,
  ];
}

export function SemanticEdge({
  id,
  sourceX,
  sourceY,
  sourcePosition,
  targetX,
  targetY,
  targetPosition,
  selected = false,
  data,
}: EdgeProps & { data?: FlowEdgeData }) {
  const m = useMessages();
  const [hovering, setHovering] = useState(false);

  const [path, labelX, labelY] =
    data?.route === 'loop-back'
      ? loopBackPath(sourceX, sourceY, targetX, targetY)
      : getSmoothStepPath({
          sourceX,
          sourceY,
          sourcePosition,
          targetX,
          targetY,
          targetPosition,
          borderRadius: 6,
          offset: LANE_OFFSET[data?.lane ?? 'main'],
        });

  const isControlEdge = data?.role === 'control' || data?.role === 'binding';
  const label = data?.label ? edgeLabelText(m, data.label) : (data?.valueKind ?? '');
  // 控制/绑定边始终标注（语义不能只靠线型区分），数据边只在选中或悬停时标注。
  const showLabel = Boolean(label) && (isControlEdge || selected || hovering);

  return (
    <g
      className="flow-edge"
      data-lane={data?.lane}
      data-selected={selected}
      data-dimmed={data?.dimmed}
      onMouseEnter={() => setHovering(true)}
      onMouseLeave={() => setHovering(false)}
    >
      <BaseEdge id={id} path={path} className="flow-edge-path" interactionWidth={24} />
      {showLabel ? (
        <EdgeLabelRenderer>
          <div
            className={cn('flow-edge-label', 'nodrag', 'nopan')}
            style={{
              position: 'absolute',
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
            }}
          >
            {label}
          </div>
        </EdgeLabelRenderer>
      ) : null}
    </g>
  );
}
