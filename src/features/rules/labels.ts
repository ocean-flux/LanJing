//! 模型层稳定 code → 本地化文案的解析边界。
//!
//! 模型（`model/`）只出 code 与参数，视图在这里查 `messages/*/rules.json`。
//! 规则编辑器的所有展示文案都应经过本模块，不在组件里散写映射。

import type { useMessages } from '@/shared/i18n/messages';
import type { EdgeLabel } from './model/flow-adapter';
import type { RecommendBlocker } from './model/connection-gate';
import type { NodeSummary } from './model/summary';
import type { PortLabel } from './model/ports';
import type { FlowNodeKind, StandardIntent } from '@/shared/tauri/rules';

type Messages = ReturnType<typeof useMessages>;

/** 节点类型标签（Inspector / 节点卡 / palette 共用）。 */
export function nodeKindLabel(m: Messages, kind: FlowNodeKind): string {
  switch (kind) {
    case 'http': {
      return m.rules_node_inspector_type_http();
    }
    case 'js': {
      return m.rules_node_inspector_type_js();
    }
    case 'extract': {
      return m.rules_node_inspector_type_extract();
    }
    case 'mapper': {
      return m.rules_node_inspector_type_mapper();
    }
    case 'merge': {
      return m.rules_node_inspector_type_merge();
    }
    case 'condition': {
      return m.rules_node_inspector_type_condition();
    }
    case 'loop': {
      return m.rules_node_inspector_type_loop();
    }
  }
}

/** 节点类型说明（palette 用）。 */
export function nodeKindDescription(m: Messages, kind: FlowNodeKind): string {
  switch (kind) {
    case 'http': {
      return m.rules_node_desc_http();
    }
    case 'js': {
      return m.rules_node_desc_js();
    }
    case 'extract': {
      return m.rules_node_desc_extract();
    }
    case 'mapper': {
      return m.rules_node_desc_mapper();
    }
    case 'merge': {
      return m.rules_node_desc_merge();
    }
    case 'condition': {
      return m.rules_node_desc_condition();
    }
    case 'loop': {
      return m.rules_node_desc_loop();
    }
  }
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
