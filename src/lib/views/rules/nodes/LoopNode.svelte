<script lang="ts">
  //! 循环节点：collection → body(yield) → done。
  //! 只读展示 kind/config 摘要/诊断标记；连接校验由画布 gate 负责。

  import type { FlowNodeData } from './types';
  import NodeShell from './NodeShell.svelte';
  import { getNodePorts } from './ports';
  import { summarizeNode } from './summary';

  type Props = {
    data: FlowNodeData;
    selected?: boolean;
  };

  let { data, selected = false }: Props = $props();

  const summary = $derived(summarizeNode(data.kind, data.config));
  const ports = $derived(getNodePorts(data.kind, data.config));
</script>

<NodeShell
  kind={data.kind}
  {summary}
  inputs={ports.inputs}
  outputs={ports.outputs}
  diagnostics={data.diagnostics}
  entryIntents={data.entryIntents}
  focused={data.focused}
  dimmed={data.dimmed}
  selectedPort={data.selectedPort}
  onPortSelect={data.onPortSelect}
  {selected}
>
  {#snippet extra()}
    <dl class="grid grid-cols-2 gap-x-3 gap-y-1 text-[9px] leading-3 text-ink-subtle">
      <div class="min-w-0">
        <dt class="font-mono text-ink-muted">collection</dt>
        <dd>集合</dd>
      </div>
      <div class="min-w-0">
        <dt class="font-mono text-ink-muted">body(item,index)</dt>
        <dd>逐项</dd>
      </div>
      <div class="min-w-0">
        <dt class="font-mono text-ink-muted">yield(value)</dt>
        <dd>产出</dd>
      </div>
      <div class="min-w-0">
        <dt class="font-mono text-ink-muted">done(collected)</dt>
        <dd>完成</dd>
      </div>
    </dl>
  {/snippet}
</NodeShell>
