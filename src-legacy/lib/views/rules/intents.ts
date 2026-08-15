//! 标准意图展示元数据（画布聚焦切换 / palette 提示共用）。
//! 纯 TS，无 runes。顺序即展示顺序。

import type { StandardIntent } from '$lib/rules/native-authoring/wire';

/** 六个标准意图（闭集）。 */
export const STANDARD_INTENTS: StandardIntent[] = [
  'Search',
  'Discover',
  'ResolveItem',
  'ListUnits',
  'ResolveAsset',
  'ContinueAction',
];

/** 意图中文标签。 */
export const INTENT_LABELS: Record<StandardIntent, string> = {
  Search: '搜索',
  Discover: '发现',
  ResolveItem: '解析条目',
  ListUnits: '列出剧集',
  ResolveAsset: '解析资源',
  ContinueAction: '继续操作',
};
