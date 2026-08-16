//! 节点结构元数据：七类节点的展示顺序与图标（palette / 节点卡共用）。
//!
//! 只放不随 locale 变化的事实。标签与描述是面向用户的文案，走
//! `messages/*/rules.json`，由视图层解析 —— 模型层不持有任何展示字符串。

import type { FlowNodeKind } from '@/shared/tauri/rules';
import type { IconName } from '@/components/Icon';

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

/** 节点图标（Iconify 白名单里的 Phosphor 名）。 */
export const NODE_KIND_ICONS: Record<FlowNodeKind, IconName> = {
  http: 'broadcast',
  js: 'code',
  extract: 'tree-structure',
  mapper: 'translate',
  merge: 'git-merge',
  condition: 'compass',
  loop: 'arrow-counter-clockwise',
};
