//! 节点配置摘要：从 config.value 推导一行摘要的**结构化描述**。
//!
//! 只出稳定 code 与参数，不出任何面向用户的字符串 —— 展示文案由视图按 locale
//! 组装。config 为领域最小镜像（Record<string, unknown>），字段名不做强假设；
//! 各分支优先读取常见字段，缺省回退到类型化描述。

import type { FlowNodeKind } from '@/shared/tauri/rules';

/**
 * 节点摘要描述。
 *
 * `literal` 分支携带来自用户配置的原文（URL、脚本首行），这类内容不翻译；
 * 其余分支只给 code 与计数，由视图查 `messages/*\/rules.json`。
 */
export type NodeSummary =
  | { code: 'http_request'; method: string; url: string }
  | { code: 'http_fallback' }
  | { code: 'js_first_line'; line: string }
  | { code: 'js_fallback' }
  | { code: 'extract_field_groups'; fieldRules: number; rules: number }
  | { code: 'extract_rules'; rules: number }
  | { code: 'mapper_identity_fields'; fields: number }
  | { code: 'mapper_fallback' }
  | { code: 'merge_fallback' }
  | { code: 'condition_branches'; count: number }
  | { code: 'condition_fallback' }
  | { code: 'loop_fallback' }
  /** 未安装能力的节点：只上报 kind 原文，不猜测内容。 */
  | { code: 'unavailable'; kind: string };

/** 读取非空字符串字段。 */
function str(value: unknown): string | undefined {
  return typeof value === 'string' && value.length > 0 ? value : undefined;
}

/** 截断过长的摘要文本。 */
export function truncate(text: string, max = 36): string {
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

/** 从节点配置推导摘要描述。 */
export function describeNode(kind: FlowNodeKind, config: Record<string, unknown>): NodeSummary {
  switch (kind) {
    case 'http': {
      const url = str(config.url);
      if (!url) return { code: 'http_fallback' };
      return {
        code: 'http_request',
        method: str(config.method)?.toUpperCase() ?? 'GET',
        url: truncate(url),
      };
    }
    case 'js': {
      const code = str(config.code);
      if (!code) return { code: 'js_fallback' };
      const firstLine = code
        .split('\n')
        .map((line) => line.trim())
        .find((line) => line.length > 0);
      return firstLine
        ? { code: 'js_first_line', line: truncate(firstLine, 40) }
        : { code: 'js_fallback' };
    }
    case 'extract': {
      const rules = Array.isArray(config.rules) ? config.rules.length : 0;
      const fieldRules =
        typeof config.field_rules === 'object' && config.field_rules !== null
          ? Object.keys(config.field_rules).length
          : 0;
      return fieldRules > 0
        ? { code: 'extract_field_groups', fieldRules, rules }
        : { code: 'extract_rules', rules };
    }
    case 'mapper': {
      const fields = Array.isArray(config.identity_fields) ? config.identity_fields.length : 0;
      return fields > 0 ? { code: 'mapper_identity_fields', fields } : { code: 'mapper_fallback' };
    }
    case 'merge': {
      return { code: 'merge_fallback' };
    }
    case 'condition': {
      const count = Array.isArray(config.branches) ? config.branches.length : 0;
      return count > 0 ? { code: 'condition_branches', count } : { code: 'condition_fallback' };
    }
    case 'loop': {
      return { code: 'loop_fallback' };
    }
    default: {
      return { code: 'unavailable', kind };
    }
  }
}
