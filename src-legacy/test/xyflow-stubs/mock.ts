//! @xyflow/svelte 的测试替身装配：vi.mock('@xyflow/svelte') 工厂引用本模块。
//! 仅测试用；提供 Handle/Panel/SvelteFlow/Background/Controls/Position/useSvelteFlow
//! 的最小实现，flowHelpers 供断言 fitView 等调用。

import { vi } from 'vitest';
import HandleStub from './HandleStub.svelte';
import PanelStub from './PanelStub.svelte';
import SvelteFlowStub from './SvelteFlowStub.svelte';
import BackgroundStub from './BackgroundStub.svelte';
import ControlsStub from './ControlsStub.svelte';
import ViewportPortalStub from './ViewportPortalStub.svelte';

/** useSvelteFlow() 返回的帮助函数（fitView 供断言）。 */
export const flowHelpers = {
  fitView: vi.fn(),
  getNodes: vi.fn(() => []),
  getEdges: vi.fn(() => []),
  getNode: vi.fn(() => undefined),
  getInternalNode: vi.fn(() => undefined),
  updateNodeInternals: vi.fn(),
};

/** Handle position 枚举替身。 */
export const Position = {
  Left: 'left',
  Right: 'right',
  Top: 'top',
  Bottom: 'bottom',
} as const;

export function useSvelteFlow() {
  return flowHelpers;
}

export function useUpdateNodeInternals() {
  return (nodeId?: string | string[]) => flowHelpers.updateNodeInternals(nodeId);
}

/** 组件别名（const 导出，避免 re-export 绑定问题）。 */
export const Handle = HandleStub;
export const Panel = PanelStub;
export const SvelteFlow = SvelteFlowStub;
export const Background = BackgroundStub;
export const Controls = ControlsStub;
export const ViewportPortal = ViewportPortalStub;
