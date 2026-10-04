//! 节点结构元数据：展示顺序与图标来自 descriptor 声明（Rust 唯一来源）。
//!
//! 只放不随 locale 变化的事实。标签与描述是面向用户的文案，走
//! `messages/*/rules.json`，由视图层按 descriptor 的 key 解析。

import { ICON_NAMES, type IconName } from '@/components/Icon';

import { nodeDescriptor, nodeDescriptors } from './descriptor-registry';

/** Descriptor 声明了白名单外的图标名时的回退：未安装能力也要能画出来。 */
const FALLBACK_NODE_ICON: IconName = 'warning-circle';

const ICON_NAME_SET: ReadonlySet<string> = new Set<string>(ICON_NAMES);

/** 节点类型的展示顺序（palette / 节点卡共用）；顺序即 descriptor 声明顺序。 */
export function nodeKinds(): string[] {
  return nodeDescriptors().map((descriptor) => descriptor.kind);
}

/** 节点图标；未声明或不在白名单内时回退到通用图标。 */
export function nodeIcon(kind: string): IconName {
  const icon = nodeDescriptor(kind)?.icon;
  return icon !== undefined && ICON_NAME_SET.has(icon) ? (icon as IconName) : FALLBACK_NODE_ICON;
}
