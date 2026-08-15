//! 节点元数据：七类节点的展示标签、描述与图标（palette/节点卡共用）。
//! 纯 TS，无 runes。

import type { FlowNodeKind } from '$lib/rules/native-authoring/wire';
import type { IconName } from '$lib/components/Icon.svelte';

/** 节点类型全集（与 Rust FlowNodeKind 闭集一致，展示顺序即此顺序）。 */
export const NODE_KINDS: FlowNodeKind[] = [
  'http',
  'js',
  'extract',
  'mapper',
  'merge',
  'condition',
  'loop',
];

/** 节点展示元数据（中文文案）。 */
export type NodeKindMeta = {
  label: string;
  description: string;
  icon: IconName;
};

export const NODE_KIND_META: Record<FlowNodeKind, NodeKindMeta> = {
  http: {
    label: 'HTTP 请求',
    description: '发起网络请求，作为意图流程的入口',
    icon: 'broadcast',
  },
  js: {
    label: 'JS 脚本',
    description: '运行脚本处理数据，输出 JSON 或原文',
    icon: 'code',
  },
  extract: {
    label: '提取',
    description: '从 HTTP 响应中提取结构化 JSON',
    icon: 'tree-structure',
  },
  mapper: {
    label: '映射',
    description: '将 JSON 映射为增量 delta',
    icon: 'translate',
  },
  merge: {
    label: '合并',
    description: '合并多条 JSON 流为一条',
    icon: 'git-merge',
  },
  condition: {
    label: '条件分支',
    description: '按条件分流到多个分支输出',
    icon: 'compass',
  },
  loop: {
    label: '循环',
    description: '遍历集合，逐项产出并给出完成信号',
    icon: 'arrow-counter-clockwise',
  },
};
