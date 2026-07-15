//! 规则列表 store — 管理已导入规则列表
//!
//! 遵循 theme.svelte.ts 模式：模块级 $state + 函数导出。
//! 字段名与 Rust serde 序列化一致（snake_case）。

import { invoke } from '@tauri-apps/api/core';

/** 规则列表项（与 Rust `RuleListItem` 对应）。 */
export interface RuleListItem {
  id: string;
  source_url: string;
  node_count: number;
}

/** 导入预览响应（与 Rust `ImportPreviewResponse` 对应）。 */
export interface ImportPreview {
  source_url: string;
  node_count: number;
  edge_count: number;
  js_block_count: number;
  sandbox: {
    network: boolean;
    system: { fs: boolean; env: boolean; process: boolean };
  };
  http_target_urls: string[];
  js_sources: string[];
  /** Graph JSON 字符串，供 confirmImport 提交。 */
  graph_json: string;
}

// 规则列表状态
let rules = $state<RuleListItem[]>([]);
let loading = $state(false);
let error = $state<string | null>(null);

/** 从后端加载已导入规则列表。 */
export async function loadRules(): Promise<void> {
  loading = true;
  error = null;
  try {
    rules = await invoke<RuleListItem[]>('list_rules');
  } catch (e) {
    error = String(e);
  } finally {
    loading = false;
  }
}

/** 导入规则并返回预览（不含落库）。 */
export async function importRule(json: string): Promise<ImportPreview> {
  return await invoke<ImportPreview>('import_rule_with_preview', {
    request: { rule_json: json },
  });
}

/** 确认导入规则并落库，返回规则 ID。 */
export async function confirmImport(graphJson: string): Promise<string> {
  const id = await invoke<string>('confirm_import', {
    request: { graph_json: graphJson },
  });
  await loadRules(); // 刷新列表
  return id;
}

export function getRules(): RuleListItem[] {
  return rules;
}
export function getLoading(): boolean {
  return loading;
}
export function getError(): string | null {
  return error;
}
