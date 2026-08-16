//! 连线检查器：只读语义 + 删除入口。
//!
//! 连线本身没有可编辑字段，改连接只能在画布上拖端点；这里负责把
//! from/to 的节点与端口语义摊开，让「这条边到底连了什么」可核对。

import { Icon } from '@/components/Icon';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { useMessages } from '@/shared/i18n/messages';
import type { FlowNode } from '@/shared/tauri/rules';
import { uiHandleForSemantic, type FlowViewEdge } from '../model/flow-adapter';
import { getNodePorts } from '../model/ports';
import { nodeKindLabel, portLabelText } from '../labels';
import { useRuleEditorSessionStore } from '../use-session';

type Messages = ReturnType<typeof useMessages>;

/** 语义 handle → 展示端口标签；找不到就退回 handle 原文。 */
function portText(
  m: Messages,
  node: FlowNode | null,
  handle: string,
  direction: 'source' | 'target',
): string {
  if (!node) return handle;
  const uiHandle = uiHandleForSemantic(node.config.kind, node.config.value, handle, direction);
  const ports = getNodePorts(node.config.kind, node.config.value);
  const port =
    direction === 'source'
      ? ports.outputs.find((candidate) => candidate.id === uiHandle)
      : ports.inputs.find((candidate) => candidate.id === uiHandle);
  return port ? portLabelText(m, port.label) : handle;
}

function EdgeEndpoint({
  title,
  node,
  nodeId,
  handle,
  direction,
  accent,
}: {
  title: string;
  node: FlowNode | null;
  nodeId: string;
  handle: string;
  direction: 'source' | 'target';
  accent: boolean;
}) {
  const m = useMessages();
  return (
    <section className="flex flex-col gap-1.5">
      <h3 className="text-ui-sm font-semibold text-ink">{title}</h3>
      <div
        className={
          accent ? 'border-l-2 border-lantern-strong/50 pl-2' : 'border-l-2 border-hairline pl-2'
        }
      >
        <p className="truncate text-ui-sm font-medium text-ink">
          {node ? nodeKindLabel(m, node.config.kind) : m.rules_edge_unknown_node()}
        </p>
        <code className="block truncate font-mono text-[10px] text-ink-muted">{nodeId}</code>
        <p className="mt-1 text-ui-sm text-ink-muted">
          {portText(m, node, handle, direction)}
          <span className="font-mono text-ink-subtle"> · {handle}</span>
        </p>
      </div>
    </section>
  );
}

export function EdgeInspector({
  edge,
  sourceNode,
  targetNode,
}: {
  edge: FlowViewEdge;
  sourceNode: FlowNode | null;
  targetNode: FlowNode | null;
}) {
  const m = useMessages();
  const store = useRuleEditorSessionStore();
  const semantic = edge.data.edge;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-start gap-2">
        <span className="flex size-7 shrink-0 items-center justify-center bg-lantern-soft text-lantern-strong">
          <Icon name="arrow-right" />
        </span>
        <div className="min-w-0">
          <p className="text-ui-sm font-semibold text-ink">{m.rules_edge_inspector_title()}</p>
          <code className="block truncate font-mono text-[10px] text-ink-muted">{edge.id}</code>
        </div>
      </div>

      <Separator />

      <dl className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-3 gap-y-2 text-ui-sm">
        <dt className="text-ink-muted">{m.rules_edge_role()}</dt>
        <dd className="text-right">
          <Badge variant="outline">{edge.data.role}</Badge>
        </dd>
        <dt className="text-ink-muted">{m.rules_edge_value_kind()}</dt>
        <dd className="truncate text-right font-mono text-ink">{edge.data.valueKind}</dd>
      </dl>

      <EdgeEndpoint
        title={m.rules_edge_source()}
        node={sourceNode}
        nodeId={semantic.from.node_id}
        handle={semantic.from.handle}
        direction="source"
        accent
      />
      <EdgeEndpoint
        title={m.rules_edge_target()}
        node={targetNode}
        nodeId={semantic.to.node_id}
        handle={semantic.to.handle}
        direction="target"
        accent={false}
      />

      <p className="text-ui-sm leading-4 text-ink-subtle">{m.rules_edge_reconnect_hint()}</p>

      <Button
        variant="outline"
        size="sm"
        className="w-full"
        onClick={() => store.getState().disconnect(semantic)}
      >
        <Icon name="trash" />
        <span>{m.rules_edge_delete()}</span>
      </Button>
    </div>
  );
}
