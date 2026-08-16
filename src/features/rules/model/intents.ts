//! 标准意图的结构元数据（画布聚焦切换 / palette 共用）。
//!
//! 只保留闭集与展示顺序。意图标签是面向用户的文案，走
//! `messages/*/rules.json` 的 `rules_template_wizard_intent_*`，由视图解析。

import type { StandardIntent } from '@/shared/tauri/rules';

/** 六个标准意图（闭集）；顺序即展示顺序。 */
export const STANDARD_INTENTS: StandardIntent[] = [
  'Search',
  'Discover',
  'ResolveItem',
  'ListUnits',
  'ResolveAsset',
  'ContinueAction',
];
