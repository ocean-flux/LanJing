//! 节点配置检查器：类型标识 + 对应 kind 的配置面板。

import { useMemo } from 'react';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import { useMessages } from '@/shared/i18n/messages';
import { canonicalConfig } from '../model/node-defaults';
import type { FlowViewNode } from '../model/flow-adapter';
import { nodeKindLabel } from '../labels';
import { useRuleEditorSessionStore } from '../use-session';
import { NodeConfigPanel } from './node-panels';

export function NodeInspector({ node }: { node: FlowViewNode }) {
  const m = useMessages();
  const store = useRuleEditorSessionStore();
  const config = useMemo(
    () => canonicalConfig(node.data.kind, node.data.config),
    [node.data.kind, node.data.config],
  );

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <Badge variant="outline" className="uppercase">
          {nodeKindLabel(m, node.data.kind)}
        </Badge>
        <code className="min-w-0 truncate font-mono text-ui-sm text-ink-muted">{node.id}</code>
      </div>

      <Separator />

      {/* 按 node.id 重挂载：面板里的本地编辑态（草稿行、CodeMirror 实例）
          属于当前这个节点，换节点必须从零开始。 */}
      <NodeConfigPanel
        key={node.id}
        kind={node.data.kind}
        config={config}
        onChange={(patch) => store.getState().setNodeConfig(node.id, patch)}
      />

      <p className="text-ui-sm leading-4 text-ink-subtle">{m.rules_node_inspector_hint()}</p>
    </div>
  );
}
