//! 选中节点上浮出的操作条。
//!
//! 复制 / 折叠 / 删除三件事都要在画布上就地完成，来回跑右侧检查器太慢。
//! 「设为意图入口」不在这里：IntentExport 同时要 flow_entry 与 mapper_output
//! 两个节点引用，单节点的悬浮条表达不了，留在检查器里做。

import { NodeToolbar, Position } from '@xyflow/react';
import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import { ButtonGroup } from '@/components/ui/button-group';
import { useMessages } from '@/shared/i18n/messages';
import { useRuleEditorSessionStore } from '../use-session';

export function NodeActionToolbar({
  nodeId,
  collapsed,
  visible,
}: {
  nodeId: string;
  collapsed: boolean;
  visible: boolean;
}) {
  const m = useMessages();
  const store = useRuleEditorSessionStore();
  const collapseLabel = collapsed ? m.rules_node_action_expand() : m.rules_node_action_collapse();

  return (
    <NodeToolbar isVisible={visible} position={Position.Top} offset={8}>
      <ButtonGroup aria-label={m.rules_node_actions_aria()}>
        <Button
          variant="outline"
          size="icon-xs"
          aria-label={m.rules_node_action_duplicate()}
          title={m.rules_node_action_duplicate()}
          onClick={() => store.getState().duplicateNodes([nodeId])}
        >
          <Icon name="copy" />
        </Button>
        <Button
          variant="outline"
          size="icon-xs"
          aria-label={collapseLabel}
          title={collapseLabel}
          onClick={() => store.getState().collapseNode(nodeId, !collapsed)}
        >
          <Icon name={collapsed ? 'caret-down' : 'caret-up'} />
        </Button>
        <Button
          variant="outline"
          size="icon-xs"
          aria-label={m.rules_node_action_delete()}
          title={m.rules_node_action_delete()}
          onClick={() => store.getState().deleteNode(nodeId)}
        >
          <Icon name="trash" />
        </Button>
      </ButtonGroup>
    </NodeToolbar>
  );
}
