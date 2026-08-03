<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
  import { Badge } from '$lib/components/ui/badge/index.js';
  import { Button } from '$lib/components/ui/button/index.js';
  import { Separator } from '$lib/components/ui/separator/index.js';
  import {
    uiHandleForSemantic,
    type FlowHandleDirection,
  } from '$lib/rules/native-authoring/flow-adapter';
  import type { FlowNode } from '$lib/rules/native-authoring/wire';
  import { NODE_KIND_META } from '../nodes/meta';
  import { getNodePorts, type InputPortDef, type OutputPortDef } from '../nodes/ports';
  import type { FlowViewEdge } from '../nodes/types';

  type Props = {
    nodeId: string | null;
    node: FlowNode | null;
    direction: FlowHandleDirection | null;
    handle: string | null;
    edges: FlowViewEdge[];
    onEditNode: () => void;
  };

  let { nodeId, node, direction, handle, edges, onEditNode }: Props = $props();

  const uiHandle = $derived.by(() => {
    if (!node || !direction || !handle) return null;
    return uiHandleForSemantic(node.config.kind, node.config.value, handle, direction);
  });
  const port = $derived.by((): InputPortDef | OutputPortDef | null => {
    if (!node || !direction || !uiHandle) return null;
    const ports = getNodePorts(node.config.kind, node.config.value);
    return direction === 'source'
      ? (ports.outputs.find((candidate) => candidate.id === uiHandle) ?? null)
      : (ports.inputs.find((candidate) => candidate.id === uiHandle) ?? null);
  });
  const relatedEdges = $derived(
    nodeId && direction && handle
      ? edges.filter((edge) =>
          direction === 'source'
            ? edge.data.edge.from.node_id === nodeId && edge.data.edge.from.handle === handle
            : edge.data.edge.to.node_id === nodeId && edge.data.edge.to.handle === handle,
        )
      : [],
  );
  const typeLabel = $derived(
    port ? ('accepts' in port ? port.accepts.join(' / ') : port.emits) : 'unknown',
  );
  const role = $derived(port?.role ?? 'data');
</script>

{#if !node || !nodeId || !direction || !handle || !port}
  <div class="flex min-h-40 items-center justify-center px-4">
    <p class="text-center text-sm text-ink-muted">选择一个端口查看其连接合同</p>
  </div>
{:else}
  <div class="flex flex-col gap-3">
    <div class="flex items-start gap-2 px-1">
      <span
        class="flex size-7 shrink-0 items-center justify-center rounded-md bg-lantern-soft text-lantern-strong"
      >
        <Icon name={direction === 'source' ? 'arrow-right' : 'arrow-left'} class="size-4" />
      </span>
      <div class="min-w-0">
        <p class="text-xs font-semibold text-ink">端口属性</p>
        <code class="block truncate text-[10px] text-ink-muted">{nodeId}</code>
      </div>
    </div>

    <Separator />

    <section class="flex flex-col gap-2 px-1" aria-labelledby="port-identity-title">
      <h3 id="port-identity-title" class="text-xs font-semibold text-ink">语义身份</h3>
      <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-2 text-xs">
        <dt class="text-ink-muted">节点</dt>
        <dd class="truncate text-right text-ink">{NODE_KIND_META[node.config.kind].label}</dd>
        <dt class="text-ink-muted">方向</dt>
        <dd class="text-right text-ink">{direction === 'source' ? '输出' : '输入'}</dd>
        <dt class="text-ink-muted">显示句柄</dt>
        <dd class="truncate text-right font-mono text-ink">{uiHandle}</dd>
        <dt class="text-ink-muted">真实句柄</dt>
        <dd class="truncate text-right font-mono text-ink">{handle}</dd>
        <dt class="text-ink-muted">角色</dt>
        <dd class="text-right"><Badge variant="outline">{role}</Badge></dd>
        <dt class="text-ink-muted">数据类型</dt>
        <dd class="text-right font-mono text-ink">{typeLabel}</dd>
      </dl>
    </section>

    <section class="flex flex-col gap-2 px-1" aria-labelledby="port-connections-title">
      <div class="flex items-center justify-between gap-2">
        <h3 id="port-connections-title" class="text-xs font-semibold text-ink">连接</h3>
        <span class="font-mono text-[10px] text-ink-subtle">{relatedEdges.length}</span>
      </div>
      {#if relatedEdges.length === 0}
        <p class="text-[11px] leading-4 text-ink-subtle">
          尚未连接。拖动端口即可创建经过校验的连线。
        </p>
      {:else}
        <ul class="flex flex-col gap-1.5" aria-label="端口连接列表">
          {#each relatedEdges as edge (edge.id)}
            <li class="border-l-2 border-lantern-strong/50 pl-2">
              <code class="block truncate text-[10px] text-ink-muted">{edge.id}</code>
              <p class="text-[11px] text-ink">
                {direction === 'source'
                  ? `${edge.data.edge.to.node_id} · ${edge.data.edge.to.handle}`
                  : `${edge.data.edge.from.node_id} · ${edge.data.edge.from.handle}`}
              </p>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <Button type="button" variant="outline" class="w-full" onclick={onEditNode}>
      <Icon name="pencil-simple" class="size-3.5" />
      <span>编辑节点配置</span>
    </Button>
  </div>
{/if}
