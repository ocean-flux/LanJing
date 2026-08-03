//! SvelteFlow 测试替身的共享捕获槽。
//! 仅测试用；被 vi.mock('@xyflow/svelte') 工厂引用。

/** 捕获 SvelteFlow 收到的 props（挂载时写入，测试读取）。 */
export const captured: { props: Record<string, unknown> | undefined } = {
  props: undefined,
};

export function setCaptured(props: Record<string, unknown>): void {
  captured.props = props;
}
