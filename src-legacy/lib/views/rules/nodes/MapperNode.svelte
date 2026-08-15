<script lang="ts">
  //! 映射节点：json → delta。
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
></NodeShell>
