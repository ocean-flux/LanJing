//! 七类节点的画布组件。
//!
//! legacy 是七个内容相同的薄壳文件，这里合成一个：节点 type 就是 kind，
//! 差异只有 Loop 的端口图例，没有必要拆成七个文件。

import type { NodeProps, Node } from '@xyflow/react';
import { useMessages } from '@/shared/i18n/messages';
import type { FlowNodeData } from '../model/flow-adapter';
import { getNodePorts } from '../model/ports';
import { describeNode } from '../model/summary';
import { nodeSummaryText } from '../labels';
import { NodeShell } from './NodeShell';

export type RuleFlowNodeType = Node<FlowNodeData, string>;

/** Loop 的四个结构化端口图例；handle 名是 compiler 术语，不翻译。 */
function LoopPortLegend() {
  const m = useMessages();
  const entries = [
    { handle: 'collection', text: m.rules_loop_legend_collection() },
    { handle: 'body(item,index)', text: m.rules_loop_legend_body() },
    { handle: 'yield(value)', text: m.rules_loop_legend_yield() },
    { handle: 'done(collected)', text: m.rules_loop_legend_done() },
  ];
  return (
    <dl className="grid grid-cols-2 gap-x-3 gap-y-1 text-[9px] leading-3 text-ink-subtle">
      {entries.map((entry) => (
        <div key={entry.handle} className="min-w-0">
          <dt className="font-mono text-ink-muted">{entry.handle}</dt>
          <dd>{entry.text}</dd>
        </div>
      ))}
    </dl>
  );
}

export function RuleFlowNode({ id, data, selected }: NodeProps<RuleFlowNodeType>) {
  const m = useMessages();
  const ports = getNodePorts(data.kind, data.config);
  return (
    <NodeShell
      nodeId={id}
      kind={data.kind}
      summary={nodeSummaryText(m, describeNode(data.kind, data.config))}
      inputs={ports.inputs}
      outputs={ports.outputs}
      diagnostics={data.diagnostics}
      entryIntents={data.entryIntents}
      focused={data.focused}
      dimmed={data.dimmed}
      selectedPort={data.selectedPort}
      onPortSelect={data.onPortSelect ?? undefined}
      selected={selected ?? false}
      extra={data.kind === 'loop' ? <LoopPortLegend /> : undefined}
    />
  );
}
