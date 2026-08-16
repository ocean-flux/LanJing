//! 端口检查器：端口的连接合同与已有连接。
//!
//! 显示句柄与真实句柄要分开列：merge/condition 的 UI handle 是按下标生成的，
//! 与 definition 里的语义 handle 不是同一个字符串，排查连线问题时必须都能看到。

import { Icon } from '@/components/Icon';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { useMessages } from '@/shared/i18n/messages';
import type { FlowNode } from '@/shared/tauri/rules';
import { uiHandleForSemantic, type FlowViewEdge } from '../model/flow-adapter';
import { getNodePorts, type InputPortDef, type OutputPortDef } from '../model/ports';
import { nodeKindLabel } from '../labels';
import { useRuleEditorSessionStore } from '../use-session';

const PORT_TYPE_SEPARATOR = ' / ';

export function PortInspector({
  node,
  direction,
  handle,
  edges,
}: {
  node: FlowNode;
  direction: 'source' | 'target';
  handle: string;
  edges: readonly FlowViewEdge[];
}) {
  const m = useMessages();
  const store = useRuleEditorSessionStore();

  const uiHandle = uiHandleForSemantic(node.config.kind, node.config.value, handle, direction);
  const ports = getNodePorts(node.config.kind, node.config.value);
  const port: InputPortDef | OutputPortDef | undefined =
    direction === 'source'
      ? ports.outputs.find((candidate) => candidate.id === uiHandle)
      : ports.inputs.find((candidate) => candidate.id === uiHandle);

  const related = edges.filter((edge) =>
    direction === 'source'
      ? edge.data.edge.from.node_id === node.id && edge.data.edge.from.handle === handle
      : edge.data.edge.to.node_id === node.id && edge.data.edge.to.handle === handle,
  );

  const typeLabel = port
    ? 'accepts' in port
      ? port.accepts.join(PORT_TYPE_SEPARATOR)
      : port.emits
    : '—';

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-start gap-2">
        <span className="flex size-7 shrink-0 items-center justify-center bg-lantern-soft text-lantern-strong">
          <Icon name={direction === 'source' ? 'arrow-right' : 'arrow-left'} />
        </span>
        <div className="min-w-0">
          <p className="text-ui-sm font-semibold text-ink">{m.rules_port_inspector_title()}</p>
          <code className="block truncate font-mono text-[10px] text-ink-muted">{node.id}</code>
        </div>
      </div>

      <Separator />

      <dl className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-3 gap-y-2 text-ui-sm">
        <dt className="text-ink-muted">{m.rules_port_inspector_node()}</dt>
        <dd className="truncate text-right text-ink">{nodeKindLabel(m, node.config.kind)}</dd>
        <dt className="text-ink-muted">{m.rules_port_inspector_direction()}</dt>
        <dd className="text-right text-ink">
          {direction === 'source'
            ? m.rules_port_inspector_direction_output()
            : m.rules_port_inspector_direction_input()}
        </dd>
        <dt className="text-ink-muted">{m.rules_port_inspector_ui_handle()}</dt>
        <dd className="truncate text-right font-mono text-ink">{uiHandle}</dd>
        <dt className="text-ink-muted">{m.rules_port_inspector_semantic_handle()}</dt>
        <dd className="truncate text-right font-mono text-ink">{handle}</dd>
        <dt className="text-ink-muted">{m.rules_port_inspector_role()}</dt>
        <dd className="text-right">
          <Badge variant="outline">{port?.role ?? 'data'}</Badge>
        </dd>
        <dt className="text-ink-muted">{m.rules_edge_value_kind()}</dt>
        <dd className="truncate text-right font-mono text-ink">{typeLabel}</dd>
      </dl>

      <section className="flex flex-col gap-1.5">
        <div className="flex items-center justify-between gap-2">
          <h3 className="text-ui-sm font-semibold text-ink">
            {m.rules_port_inspector_connections()}
          </h3>
          <span className="font-mono text-[10px] text-ink-subtle">{related.length}</span>
        </div>
        {related.length === 0 ? (
          <p className="text-ui-sm leading-4 text-ink-subtle">
            {m.rules_port_inspector_no_connections()}
          </p>
        ) : (
          <ul
            className="flex flex-col gap-1.5"
            aria-label={m.rules_port_inspector_connection_list()}
          >
            {related.map((edge) => {
              const peer = direction === 'source' ? edge.data.edge.to : edge.data.edge.from;
              return (
                <li key={edge.id} className="border-l-2 border-lantern-strong/50 pl-2">
                  <code className="block truncate font-mono text-[10px] text-ink-muted">
                    {edge.id}
                  </code>
                  <p className="truncate text-ui-sm text-ink">
                    {peer.node_id} · {peer.handle}
                  </p>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      <Button
        variant="outline"
        size="sm"
        className="w-full"
        onClick={() => store.getState().selectNode(node.id)}
      >
        <Icon name="pencil-simple" />
        <span>{m.rules_port_inspector_edit_node()}</span>
      </Button>
    </div>
  );
}
