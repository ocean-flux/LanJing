//! 节点配置摘要：从 config.value 生成一行中文摘要（只读展示）。
//!
//! config 为领域最小镜像（Record<string, unknown>），字段名不做强假设；
//! 各分支优先读取常见字段，缺省回退到类型化文案。纯 TS，无 runes。

import type { FlowNodeKind } from '$lib/rules/native-authoring/wire';

/** 读取非空字符串字段。 */
function str(value: unknown): string | undefined {
  return typeof value === 'string' && value.length > 0 ? value : undefined;
}

/** 截断过长的摘要文本。 */
function truncate(text: string, max = 36): string {
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

/** 生成节点配置摘要。 */
export function summarizeNode(kind: FlowNodeKind, config: Record<string, unknown>): string {
  switch (kind) {
    case 'http': {
      const method = str(config.method)?.toUpperCase();
      const url = str(config.url);
      if (url) return `${method ?? 'GET'} ${truncate(url)}`;
      return '发起 HTTP 请求';
    }
    case 'js': {
      const code = str(config.code);
      if (code) {
        const firstLine = code
          .split('\n')
          .map((line) => line.trim())
          .find((line) => line.length > 0);
        return firstLine ? truncate(firstLine, 40) : '运行 JS 脚本';
      }
      return '运行 JS 脚本';
    }
    case 'extract': {
      const rules = Array.isArray(config.rules) ? config.rules.length : 0;
      const fieldRules =
        typeof config.field_rules === 'object' && config.field_rules !== null
          ? Object.keys(config.field_rules).length
          : 0;
      return fieldRules > 0 ? `${fieldRules} 个字段组，${rules} 条规则` : `${rules} 条提取规则`;
    }
    case 'mapper': {
      const fields = Array.isArray(config.identity_fields) ? config.identity_fields.length : 0;
      return fields > 0 ? `${fields} 个身份字段` : 'JSON 映射为 delta';
    }
    case 'merge':
      return '合并多条输入流';
    case 'condition': {
      const count = Array.isArray(config.branches) ? config.branches.length : 0;
      return count > 0 ? `${count} 个条件分支` : '按条件分流';
    }
    case 'loop':
      return '遍历集合逐项处理';
  }
}
