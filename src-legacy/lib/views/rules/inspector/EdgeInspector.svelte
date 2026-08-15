<script lang="ts">
  import { m } from '$lib/i18n';
  import Icon from '$lib/components/Icon.svelte';
  import { Badge } from '$lib/components/ui/badge/index.js';
  import { Button } from '$lib/components/ui/button/index.js';
  import { Separator } from '$lib/components/ui/separator/index.js';
  import { uiHandleForSemantic } from '$lib/rules/native-authoring/flow-adapter';
  import type { FlowNode, FlowNodeKind } from '$lib/rules/native-authoring/wire';
  import { getNodePorts } from '../nodes/ports';
  import type { FlowViewEdge } from '../nodes/types';

  type Props = {
    edge: FlowViewEdge | null;
    sourceNode: FlowNode | null;
    targetNode: FlowNode | null;
    onDelete: () => void;
  };

  let { edge, sourceNode, targetNode, onDelete }: Props = $props();

  const typeLabels: Record<FlowNodeKind, () => string> = {
    http: () => m.rules_node_inspector_type_http(),
    js: () => m.rules_node_inspector_type_js(),
    extract: () => m.rules_node_inspector_type_extract(),
    mapper: () => m.rules_node_inspector_type_mapper(),
    merge: () => m.rules_node_inspector_type_merge(),
    condition: () => m.rules_node_inspector_type_condition(),
    loop: () => m.rules_node_inspector_type_loop(),
  };

  const semanticEdge = $derived(edge?.data.edge ?? null);
  const sourcePort = $derived.by(() => {
    if (!semanticEdge || !sourceNode) return null;
    const handle = uiHandleForSemantic(
      sourceNode.config.kind,
      sourceNode.config.value,
      semanticEdge.from.handle,
      'source',
    );
    return (
      getNodePorts(sourceNode.config.kind, sourceNode.config.value).outputs.find(
        (port) => port.id === handle,
      ) ?? null
    );
  });
  const targetPort = $derived.by(() => {
    if (!semanticEdge || !targetNode) return null;
    const handle = uiHandleForSemantic(
      targetNode.config.kind,
      targetNode.config.value,
      semanticEdge.to.handle,
      'target',
    );
    return (
      getNodePorts(targetNode.config.kind, targetNode.config.value).inputs.find(
        (port) => port.id === handle,
      ) ?? null
    );
  });

  const role = $derived(edge?.data.role ?? 'data');
  const valueKind = $derived(edge?.data.valueKind ?? 'unknown');

  function nodeLabel(node: FlowNode | null): string {
    return node ? typeLabels[node.config.kind]() : m.rules_edge_unknown_node();
  }
</script>

{#if !edge || !semanticEdge}
  <div class="flex min-h-40 items-center justify-center px-4">
    <p class="text-center text-sm text-ink-muted">{m.rules_edge_inspector_no_selection()}</p>
  </div>
{:else}
  <div class="flex flex-col gap-3">
    <div class="flex items-start gap-2 px-1">
      <span
        class="flex size-7 shrink-0 items-center justify-center rounded-md bg-lantern-soft text-lantern-strong"
      >
        <Icon name="arrow-right" class="size-4" />
      </span>
      <div class="min-w-0">
        <p class="text-xs font-semibold text-ink">{m.rules_edge_inspector_title()}</p>
        <code class="block truncate text-[10px] text-ink-muted">{edge.id}</code>
      </div>
    </div>

    <Separator />

    <div class="flex flex-col gap-2 px-1">
      <div class="flex items-center justify-between gap-2">
        <span class="text-xs text-ink-muted">{m.rules_edge_role()}</span>
        <Badge variant="outline">{role}</Badge>
      </div>
      <div class="flex items-center justify-between gap-2">
        <span class="text-xs text-ink-muted">{m.rules_edge_value_kind()}</span>
        <code class="text-xs font-semibold text-ink">{valueKind}</code>
      </div>
      {#if edge.data.label}
        <div class="flex items-center justify-between gap-2">
          <span class="text-xs text-ink-muted">{m.rules_edge_semantic_label()}</span>
          <span class="max-w-[11rem] truncate text-right text-xs text-ink">{edge.data.label}</span>
        </div>
      {/if}
    </div>

    <section class="flex flex-col gap-2 px-1" aria-labelledby="edge-source-title">
      <h3 id="edge-source-title" class="text-xs font-semibold text-ink">{m.rules_edge_source()}</h3>
      <div class="border-l-2 border-lantern-strong/50 pl-2">
        <p class="truncate text-xs font-medium text-ink">{nodeLabel(sourceNode)}</p>
        <code class="block truncate text-[10px] text-ink-muted">{semanticEdge.from.node_id}</code>
        <p class="mt-1 text-[11px] text-ink-muted">
          {sourcePort?.label ?? semanticEdge.from.handle}
          <span class="font-mono text-ink-subtle"> · {semanticEdge.from.handle}</span>
        </p>
      </div>
    </section>

    <section class="flex flex-col gap-2 px-1" aria-labelledby="edge-target-title">
      <h3 id="edge-target-title" class="text-xs font-semibold text-ink">{m.rules_edge_target()}</h3>
      <div class="border-l-2 border-hairline-strong pl-2">
        <p class="truncate text-xs font-medium text-ink">{nodeLabel(targetNode)}</p>
        <code class="block truncate text-[10px] text-ink-muted">{semanticEdge.to.node_id}</code>
        <p class="mt-1 text-[11px] text-ink-muted">
          {targetPort?.label ?? semanticEdge.to.handle}
          <span class="font-mono text-ink-subtle"> · {semanticEdge.to.handle}</span>
        </p>
      </div>
    </section>

    <p class="px-1 text-[11px] leading-4 text-ink-subtle">{m.rules_edge_reconnect_hint()}</p>

    <Button type="button" variant="outline" class="w-full" onclick={onDelete}>
      <Icon name="trash" class="size-3.5" />
      <span>{m.rules_edge_delete()}</span>
    </Button>
  </div>
{/if}
