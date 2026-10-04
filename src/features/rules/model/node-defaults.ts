//! 新节点的默认 config。
//!
//! 默认值只有 descriptor（Rust `lj-rule-model::descriptor`）一个来源：本模块不再
//! 自带 kind → 默认值表，因为那份表会与 Rust 的 typed config 漂移。

import { descriptorDefaultConfig } from './descriptor-registry';
import type { JsonObject } from './node-config';

function clone<T>(value: T): T {
  try {
    return structuredClone(value);
  } catch {
    throw new Error('无法复制节点配置');
  }
}

/**
 * 为新节点按 descriptor 声明补齐默认 config。
 *
 * 查不到声明（未安装能力）时返回空 config：节点仍可展示、保存与 round-trip，
 * 但不能通过 validate/compile/execute。
 */
export function canonicalConfig(kind: string, config: JsonObject | null): JsonObject {
  const declared = descriptorDefaultConfig(kind);
  return { ...clone(declared ?? {}), ...config };
}
