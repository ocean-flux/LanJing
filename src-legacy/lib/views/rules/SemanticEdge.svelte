<script lang="ts">
  import { BaseEdge, EdgeLabel, getSmoothStepPath, type EdgeProps } from '@xyflow/svelte';
  import { cn } from '$lib/utils.js';
  import type { FlowEdgeData } from './nodes/types';

  type Props = EdgeProps & { data?: FlowEdgeData };

  let {
    id,
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
    selected = false,
    data,
  }: Props = $props();

  let hovering = $state(false);
  function loopBackPath(): [string, number, number] {
    const channelX = Math.max(sourceX, targetX) + 96;
    const labelX = channelX;
    const labelY = (sourceY + targetY) / 2;
    return [
      `M ${sourceX} ${sourceY} L ${channelX} ${sourceY} L ${channelX} ${targetY} L ${targetX} ${targetY}`,
      labelX,
      labelY,
    ];
  }

  const [path, labelX, labelY] = $derived(
    data?.route === 'loop-back'
      ? loopBackPath()
      : getSmoothStepPath({
          sourceX,
          sourceY,
          sourcePosition,
          targetX,
          targetY,
          targetPosition,
          borderRadius: 6,
          offset: data?.lane === 'branch' ? 26 : data?.lane === 'auxiliary' ? 22 : 18,
        }),
  );
  const isControlEdge = $derived(data?.role === 'control' || data?.role === 'binding');
  const label = $derived(data?.label ?? data?.valueKind ?? 'unknown');
  const showLabel = $derived(isControlEdge || selected || hovering);
</script>

<g
  role="group"
  onmouseenter={() => (hovering = true)}
  onmouseleave={() => (hovering = false)}
  aria-label={data?.label ?? data?.valueKind ?? '规则连接'}
>
  <BaseEdge
    {id}
    {path}
    {labelX}
    {labelY}
    class={cn(
      'semantic-edge-path',
      data?.lane === 'branch' && 'semantic-edge-branch',
      data?.lane === 'auxiliary' && 'semantic-edge-auxiliary',
      data?.lane === 'loop' && 'semantic-edge-lane-loop',
      data?.role === 'control' && 'semantic-edge-control',
      data?.role === 'binding' && 'semantic-edge-binding',
      data?.route === 'loop-back' && 'semantic-edge-loop-back',
      selected && 'semantic-edge-selected',
    )}
    interactionWidth={24}
  />
  {#if showLabel}
    <EdgeLabel
      x={labelX}
      y={labelY}
      selectEdgeOnClick
      class={cn(
        'semantic-edge-label',
        data?.role === 'control' && 'semantic-edge-label-control',
        data?.role === 'binding' && 'semantic-edge-label-binding',
      )}
    >
      {label}
    </EdgeLabel>
  {/if}
</g>
