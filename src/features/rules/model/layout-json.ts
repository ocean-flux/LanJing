//! `layout_json` 的解析边界。
//!
//! wire 侧布局是字符串，core 的 `readLayout` 只接受已解析的值且不解释未知结构。
//! 这里承担「字符串 → unknown」这一步，坏 JSON 视为无布局，让画布退回默认排布，
//! 而不是让整个文档加载失败。

import { readLayout, type NativeDocumentLayout } from './core';

/** 解析 `layout_json`；非法 JSON 返回 null 而不抛出。 */
export function parseLayoutJson(raw: string | null | undefined): unknown {
  if (!raw) return null;
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    return null;
  }
}

/** 从 `layout_json` 直接读出布局；解析失败或结构不符时返回 null。 */
export function readDocumentLayout(raw: string | null | undefined): NativeDocumentLayout | null {
  return readLayout(parseLayoutJson(raw));
}
