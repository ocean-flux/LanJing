//! 自定义节点注册表：node.type（FlowNodeKind）→ 组件。
//! Svelte Flow 用 `nodeTypes` 按字符串类型解析节点组件。

import HttpNode from './HttpNode.svelte';
import JsNode from './JsNode.svelte';
import ExtractNode from './ExtractNode.svelte';
import MapperNode from './MapperNode.svelte';
import MergeNode from './MergeNode.svelte';
import ConditionNode from './ConditionNode.svelte';
import LoopNode from './LoopNode.svelte';

/** 七类自定义节点组件映射（与 FlowNodeKind 闭集一致）。 */
export const nodeTypes = {
  http: HttpNode,
  js: JsNode,
  extract: ExtractNode,
  mapper: MapperNode,
  merge: MergeNode,
  condition: ConditionNode,
  loop: LoopNode,
} as const;
