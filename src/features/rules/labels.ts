//! 模型层稳定 code → 本地化文案的解析边界。
//!
//! 模型（`model/`）只出 code 与参数，视图在这里查 `messages/*/rules.json`。
//! 规则编辑器的所有展示文案都应经过本模块，不在组件里散写映射。

import type { useMessages } from '@/shared/i18n/messages';
import {
  type ExecutionRunMode,
  type ExecutionRunStatus,
  type ReplayFailureReason,
} from './model/execution';
import { nodeDescriptor } from './model/descriptor-registry';
import type { EdgeLabel } from './model/flow-adapter';
import type { RecommendBlocker } from './model/connection-gate';
import type { NodeSummary } from './model/summary';
import type { PortLabel } from './model/ports';
import type { StandardIntent } from '@/shared/tauri/rules';

type Messages = ReturnType<typeof useMessages>;

/** Descriptor / 模型出的 message key → 文案；key 不存在时返回 `null`。 */
export function messageKeyText(m: Messages, key: string | undefined): string | null {
  if (key === undefined) return null;
  const message = (m as unknown as Record<string, (params?: never) => string>)[key];
  return typeof message === 'function' ? message() : null;
}

/**
 * 节点类型标签（Inspector / 节点卡 / palette 共用）。
 *
 * 文案 key 由 descriptor 声明；未安装能力没有声明，退回 wire kind 原文，
 * 让作者仍能看出节点是什么。
 */
export function nodeKindLabel(m: Messages, kind: string): string {
  return messageKeyText(m, nodeDescriptor(kind)?.label_key) ?? kind;
}

/** 节点类型说明（palette 用）。 */
export function nodeKindDescription(m: Messages, kind: string): string {
  return messageKeyText(m, nodeDescriptor(kind)?.description_key) ?? kind;
}

/** 标准意图标签。 */
export function intentLabel(m: Messages, intent: StandardIntent): string {
  switch (intent) {
    case 'Search': {
      return m.rules_template_wizard_intent_search();
    }
    case 'Discover': {
      return m.rules_template_wizard_intent_discover();
    }
    case 'ResolveItem': {
      return m.rules_template_wizard_intent_resolve_item();
    }
    case 'ListUnits': {
      return m.rules_template_wizard_intent_list_units();
    }
    case 'ResolveAsset': {
      return m.rules_template_wizard_intent_resolve_asset();
    }
    case 'ContinueAction': {
      return m.rules_template_wizard_intent_continue_action();
    }
  }
}

/** 端口标签；`literal` 是用户配置或 compiler 术语，原样返回。 */
export function portLabelText(m: Messages, label: PortLabel): string {
  if (label.kind === 'literal') return label.text;
  const index = label.index ?? 0;
  switch (label.key) {
    case 'http_response': {
      return m.rules_port_label_http_response();
    }
    case 'js_input': {
      return m.rules_port_label_js_input();
    }
    case 'json_output': {
      return m.rules_port_label_json_output();
    }
    case 'raw_output': {
      return m.rules_port_label_raw_output();
    }
    case 'extract_result': {
      return m.rules_port_label_extract_result();
    }
    case 'json_input': {
      return m.rules_port_label_json_input();
    }
    case 'delta_output': {
      return m.rules_port_label_delta_output();
    }
    case 'merge_input': {
      return m.rules_port_label_merge_input({ index });
    }
    case 'merge_result': {
      return m.rules_port_label_merge_result();
    }
    case 'condition_input': {
      return m.rules_port_label_condition_input();
    }
    case 'condition_branch': {
      return m.rules_port_label_condition_branch({ index });
    }
  }
}

/** 边标签。 */
export function edgeLabelText(m: Messages, label: EdgeLabel): string {
  return label.kind === 'condition-branch'
    ? m.rules_edge_label_condition_branch({ branch: portLabelText(m, label.branch) })
    : portLabelText(m, label.label);
}

/** 节点配置摘要。 */
export function nodeSummaryText(m: Messages, summary: NodeSummary): string {
  switch (summary.code) {
    case 'http_request': {
      return m.rules_node_summary_http_request({ method: summary.method, url: summary.url });
    }
    case 'http_fallback': {
      return m.rules_node_summary_http_fallback();
    }
    case 'js_first_line': {
      return m.rules_node_summary_js_first_line({ line: summary.line });
    }
    case 'js_fallback': {
      return m.rules_node_summary_js_fallback();
    }
    case 'extract_field_groups': {
      return m.rules_node_summary_extract_field_groups({
        fieldRules: summary.fieldRules,
        rules: summary.rules,
      });
    }
    case 'extract_rules': {
      return m.rules_node_summary_extract_rules({ rules: summary.rules });
    }
    case 'mapper_identity_fields': {
      return m.rules_node_summary_mapper_identity_fields({ fields: summary.fields });
    }
    case 'mapper_fallback': {
      return m.rules_node_summary_mapper_fallback();
    }
    case 'merge_fallback': {
      return m.rules_node_summary_merge_fallback();
    }
    case 'condition_branches': {
      return m.rules_node_summary_condition_branches({ count: summary.count });
    }
    case 'condition_fallback': {
      return m.rules_node_summary_condition_fallback();
    }
    case 'loop_fallback': {
      return m.rules_node_summary_loop_fallback();
    }
    case 'unavailable': {
      return m.rules_node_unavailable({ kind: summary.kind });
    }
  }
}

/** Palette 推荐的不兼容原因。 */
export function recommendBlockerText(m: Messages, blocker: RecommendBlocker): string {
  return blocker.code === 'incompatible-ports'
    ? m.rules_recommend_blocker_incompatible_ports({
        source: nodeKindLabel(m, blocker.sourceKind),
        target: nodeKindLabel(m, blocker.targetKind),
      })
    : m.rules_recommend_blocker_outside_intent_focus();
}

/** 预览运行状态标签。 */
export function executionStatusLabel(m: Messages, status: ExecutionRunStatus): string {
  switch (status) {
    case 'idle': {
      return m.rules_execution_status_idle();
    }
    case 'running': {
      return m.rules_execution_status_running();
    }
    case 'succeeded': {
      return m.rules_execution_status_succeeded();
    }
    case 'failed': {
      return m.rules_execution_status_failed();
    }
    case 'cancelled': {
      return m.rules_execution_status_cancelled();
    }
  }
}

/**
 * 运行状态的语义色调。
 *
 * 成功 / 失败 / 取消是三种不同的已知结局，各自用既有语义 token；未结束的运行不可
 * 与任何一种结局混为一色。能选色值的只有这一处，组件不自己解释状态。
 */
export function executionStatusClass(status: ExecutionRunStatus): string {
  switch (status) {
    case 'succeeded': {
      return 'text-positive';
    }
    case 'failed': {
      return 'text-danger';
    }
    case 'cancelled': {
      return 'text-warning';
    }
    case 'idle':
    case 'running': {
      return 'text-ink-muted';
    }
  }
}

/** 运行模式标签：重放与实时运行必须一眼可分辨。 */
export function executionModeLabel(m: Messages, mode: ExecutionRunMode): string {
  return mode === 'replay' ? m.rules_execution_mode_replay() : m.rules_execution_mode_live();
}

/** 重放失败归类文案：告诉作者下一步该补什么，而不是只说「失败」。 */
export function replayFailureText(m: Messages, reason: ReplayFailureReason): string {
  switch (reason) {
    case 'capture_missing': {
      return m.rules_execution_replay_failure_capture_missing();
    }
    case 'history_mismatch': {
      return m.rules_execution_replay_failure_history_mismatch();
    }
    case 'history_unavailable': {
      return m.rules_execution_replay_failure_history_unavailable();
    }
  }
}
