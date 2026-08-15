import {
  Background,
  Controls,
  Handle,
  MiniMap,
  Position,
  ReactFlow,
  type Edge,
  type Node,
  type NodeProps,
  type NodeTypes,
  useNodesState,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { AlertCircle, Braces, Check, Cloud, Code2, GitMerge, RefreshCw, Save } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button } from '@/components/ui';
import { useMessages } from '@/shared/i18n/messages';
import { useTheme } from '@/shared/theme/use-theme';
import {
  getNativeRuleDocument,
  listNativeRuleDocuments,
  parseRuleLayout,
  saveRuleLayout,
  type FlowNodeKind,
  type NativeRuleDocumentDetail,
  type NativeRuleDocumentSummary,
  type RuleDefinition,
  type RuleLayout,
} from '@/shared/tauri/rules';

type Messages = ReturnType<typeof useMessages>;

function getNodeMeta(
  m: Messages,
): Record<FlowNodeKind, { label: string; description: string; icon: typeof Cloud; color: string }> {
  return {
    http: {
      label: m.rules_node_inspector_type_http(),
      description: m.rules_node_inspector_type_http(),
      icon: Cloud,
      color: 'var(--lantern-strong)',
    },
    js: {
      label: m.rules_node_inspector_type_js(),
      description: m.rules_node_inspector_type_js(),
      icon: Code2,
      color: 'var(--lantern)',
    },
    extract: {
      label: m.rules_node_inspector_type_extract(),
      description: m.rules_node_inspector_type_extract(),
      icon: Braces,
      color: 'var(--lantern-hover)',
    },
    mapper: {
      label: m.rules_node_inspector_type_mapper(),
      description: m.rules_node_inspector_type_mapper(),
      icon: RefreshCw,
      color: 'var(--accent)',
    },
    merge: {
      label: m.rules_node_inspector_type_merge(),
      description: m.rules_node_inspector_type_merge(),
      icon: GitMerge,
      color: 'var(--lantern-strong)',
    },
    condition: {
      label: m.rules_node_inspector_type_condition(),
      description: m.rules_node_inspector_type_condition(),
      icon: AlertCircle,
      color: 'var(--lantern)',
    },
    loop: {
      label: m.rules_node_inspector_type_loop(),
      description: m.rules_node_inspector_type_loop(),
      icon: RefreshCw,
      color: 'var(--lantern-hover)',
    },
  };
}

interface RuleNodeData {
  [key: string]: unknown;
  kind: FlowNodeKind;
  summary: string;
  inputs: string[];
  outputs: string[];
}

type RuleFlowNode = Node<RuleNodeData, 'rule'>;

function portTop(index: number, count: number) {
  return `${((index + 1) / (count + 1)) * 100}%`;
}

function RuleNode({ data, selected }: NodeProps<RuleFlowNode>) {
  const m = useMessages();
  const meta = getNodeMeta(m)[data.kind];
  const Icon = meta.icon;
  return (
    <div
      className={`w-60 border bg-(--surface) shadow-sm ${selected ? 'border-(--accent) ring-2 ring-(--ring)/30' : 'border-(--border)'}`}
      aria-label={`${meta.label}：${data.summary}`}
    >
      {data.inputs.map((handle, index) => (
        <Handle
          key={`target:${handle}`}
          id={handle}
          type="target"
          position={Position.Left}
          style={{ top: portTop(index, data.inputs.length) }}
          aria-label={m.rules_port_input({ handle })}
        />
      ))}
      <div className="flex items-center gap-3 border-b border-(--border) px-4 py-3">
        <span
          className="grid h-8 w-8 place-items-center rounded-md text-(--on-lantern)"
          style={{ backgroundColor: meta.color }}
        >
          <Icon size={16} aria-hidden="true" />
        </span>
        <div className="min-w-0">
          <p className="text-[11px] font-semibold text-(--muted-text) uppercase">{data.kind}</p>
          <p className="truncate text-sm font-semibold">{meta.label}</p>
        </div>
      </div>
      <p className="px-4 py-3 text-xs leading-5 text-(--muted-text)">{data.summary}</p>
      {data.outputs.map((handle, index) => (
        <Handle
          key={`source:${handle}`}
          id={handle}
          type="source"
          position={Position.Right}
          style={{ top: portTop(index, data.outputs.length) }}
          aria-label={m.rules_port_output({ handle })}
        />
      ))}
    </div>
  );
}

const nodeTypes: NodeTypes = { rule: RuleNode };

function summarizeNode(kind: FlowNodeKind, config: Record<string, unknown>, m: Messages) {
  if (kind === 'http' && typeof config.url === 'string') {
    const method = typeof config.method === 'string' ? config.method.toUpperCase() : 'GET';
    return `${method} ${config.url}`;
  }
  if (kind === 'condition' && Array.isArray(config.branches)) {
    return m.rules_condition_branches({ count: config.branches.length });
  }
  return getNodeMeta(m)[kind].description;
}

function createGraph(definition: RuleDefinition, layout: RuleLayout, m: Messages) {
  const inputHandles = new Map<string, Set<string>>();
  const outputHandles = new Map<string, Set<string>>();
  for (const edge of definition.flow.edges) {
    const inputs = inputHandles.get(edge.to.node_id) ?? new Set<string>();
    inputs.add(edge.to.handle);
    inputHandles.set(edge.to.node_id, inputs);
    const outputs = outputHandles.get(edge.from.node_id) ?? new Set<string>();
    outputs.add(edge.from.handle);
    outputHandles.set(edge.from.node_id, outputs);
  }

  const nodes: RuleFlowNode[] = definition.flow.nodes.map((node, index) => ({
    id: node.id,
    type: 'rule',
    position: layout.nodes[node.id]?.position ?? {
      x: 48 + (index % 3) * 320,
      y: 48 + Math.floor(index / 3) * 190,
    },
    data: {
      kind: node.config.kind,
      summary: summarizeNode(node.config.kind, node.config.value, m),
      inputs: [...(inputHandles.get(node.id) ?? [])],
      outputs: [...(outputHandles.get(node.id) ?? [])],
    },
  }));
  const edges: Edge[] = definition.flow.edges.map((edge) => ({
    id: `${edge.from.node_id}:${edge.from.handle}->${edge.to.node_id}:${edge.to.handle}`,
    source: edge.from.node_id,
    target: edge.to.node_id,
    sourceHandle: edge.from.handle,
    targetHandle: edge.to.handle,
    type: 'smoothstep',
    animated: false,
  }));
  return { nodes, edges };
}

export function RuleWorkspace() {
  const m = useMessages();
  const { resolvedTheme } = useTheme();
  const [documents, setDocuments] = useState<NativeRuleDocumentSummary[]>([]);
  const [selectedDocumentId, setSelectedDocumentId] = useState('');
  const [detail, setDetail] = useState<NativeRuleDocumentDetail>();
  const [edges, setEdges] = useState<Edge[]>([]);
  const [nodes, setNodes, onNodesChange] = useNodesState<RuleFlowNode>([]);
  const [status, setStatus] = useState<'loading' | 'ready' | 'saving' | 'error'>('loading');
  const [message, setMessage] = useState('');
  const [isLayoutDirty, setIsLayoutDirty] = useState(false);
  const currentDocumentId = useRef('');

  const loadDocument = useCallback(
    async (documentId: string, signal?: AbortSignal) => {
      setStatus('loading');
      setMessage('');
      try {
        const nextDetail = await getNativeRuleDocument(documentId);
        if (signal?.aborted || currentDocumentId.current !== documentId) {
          return;
        }
        setDetail(nextDetail ?? undefined);
        if (!nextDetail?.definition) {
          setNodes([]);
          setEdges([]);
          setStatus('ready');
          return;
        }
        const graph = createGraph(
          nextDetail.definition,
          parseRuleLayout(nextDetail.layout_json),
          m,
        );
        setNodes(graph.nodes);
        setEdges(graph.edges);
        setIsLayoutDirty(false);
        setStatus('ready');
      } catch (error) {
        if (signal?.aborted) return;
        setStatus('error');
        setMessage(error instanceof Error ? error.message : String(error));
      }
    },
    [m, setNodes],
  );

  useEffect(() => {
    const controller = new AbortController();
    void listNativeRuleDocuments()
      .then((items) => {
        if (controller.signal.aborted) {
          return;
        }
        setDocuments(items);
        const first = items[0]?.document_id ?? '';
        currentDocumentId.current = first;
        setSelectedDocumentId(first);
        if (first) {
          void loadDocument(first, controller.signal);
        } else {
          setStatus('ready');
        }
      })
      .catch((error: unknown) => {
        if (controller.signal.aborted) return;
        setStatus('error');
        setMessage(error instanceof Error ? error.message : String(error));
      });
    return () => controller.abort();
  }, [loadDocument]);

  const handleDocumentChange = (documentId: string) => {
    if (isLayoutDirty && !window.confirm(m.rules_switch_document_confirm())) return;
    currentDocumentId.current = documentId;
    setSelectedDocumentId(documentId);
    const controller = new AbortController();
    void loadDocument(documentId, controller.signal);
  };

  const saveLayout = async () => {
    if (!detail) {
      return;
    }
    const documentId = detail.summary.document_id;
    setStatus('saving');
    setMessage('');
    const previousLayout = parseRuleLayout(detail.layout_json);
    const layout: RuleLayout = {
      ...previousLayout,
      nodes: Object.fromEntries(
        nodes.map((node) => [
          node.id,
          {
            position: { x: node.position.x, y: node.position.y },
            collapsed: previousLayout.nodes[node.id]?.collapsed ?? false,
          },
        ]),
      ),
    };
    try {
      const result = await saveRuleLayout(documentId, detail.layout_revision, layout);
      if (currentDocumentId.current !== documentId) return;
      const conflict = result.layout?.conflict;
      if (conflict) {
        setStatus('error');
        setMessage(
          m.rules_layout_conflict_detail({
            expected: conflict.expected,
            current: conflict.current,
          }),
        );
        return;
      }
      setDetail({
        ...detail,
        layout_revision: result.layout?.revision ?? detail.layout_revision,
        layout_json: JSON.stringify(layout),
      });
      setIsLayoutDirty(false);
      setStatus('ready');
      setMessage(m.rules_layout_saved());
    } catch (error) {
      setStatus('error');
      setMessage(error instanceof Error ? error.message : String(error));
    }
  };

  const minimapColor = useMemo(
    () => (node: RuleFlowNode) => getNodeMeta(m)[node.data.kind].color,
    [m],
  );

  return (
    <div className="mx-auto max-w-[1600px] px-5 py-6 sm:px-8 lg:px-10">
      <div className="flex flex-col justify-between gap-4 border-b border-(--border) pb-5 lg:flex-row lg:items-end">
        <div>
          <p className="eyebrow">{m.rules_canvas_eyebrow()}</p>
          <h1 className="font-display mt-2 text-3xl font-semibold">{m.rules_canvas_title()}</h1>
          <p className="mt-2 text-sm text-(--muted-text)">{m.rules_canvas_description()}</p>
        </div>
        <div className="flex flex-wrap items-end gap-3">
          <label className="grid gap-1 text-xs text-(--muted-text)">
            {m.rules_document_label()}
            <select
              className="h-10 min-w-64 rounded-md border border-(--border) bg-(--surface) px-3 text-sm text-(--text) focus-visible:ring-2 focus-visible:ring-(--ring) focus-visible:outline-none"
              value={selectedDocumentId}
              onChange={(event) => handleDocumentChange(event.target.value)}
              disabled={status === 'loading' || documents.length === 0}
            >
              {documents.length === 0 && <option value="">{m.rules_no_documents_option()}</option>}
              {documents.map((document) => (
                <option key={document.document_id} value={document.document_id}>
                  {document.title}
                </option>
              ))}
            </select>
          </label>
          <Button
            onClick={() => void saveLayout()}
            disabled={!detail || !isLayoutDirty || status === 'saving'}
          >
            <Save size={16} aria-hidden="true" />
            {status === 'saving' ? m.rules_save_layout_saving() : m.rules_save_layout()}
          </Button>
        </div>
      </div>

      {message && (
        <p
          className="mt-4 text-sm text-(--muted-text)"
          role={status === 'error' ? 'alert' : 'status'}
        >
          {message}
        </p>
      )}

      <div className="mt-5 h-[min(72vh,760px)] min-h-[480px] overflow-hidden border border-(--border) bg-(--surface-2)">
        {nodes.length === 0 ? (
          <div className="grid h-full place-items-center px-6 text-center">
            <div>
              <Check className="mx-auto text-(--muted-text)" size={24} aria-hidden="true" />
              <p className="mt-3 font-medium">
                {status === 'loading' ? m.rules_canvas_loading() : m.rules_canvas_empty()}
              </p>
              <p className="mt-1 text-sm text-(--muted-text)">{m.rules_canvas_empty_hint()}</p>
            </div>
          </div>
        ) : (
          <ReactFlow
            nodes={nodes}
            edges={edges}
            nodeTypes={nodeTypes}
            onNodesChange={(changes) => {
              onNodesChange(changes);
              if (
                changes.some((change) => change.type === 'position' && change.dragging === false)
              ) {
                setIsLayoutDirty(true);
              }
            }}
            nodesConnectable={false}
            nodesDraggable={status !== 'saving'}
            edgesReconnectable={false}
            fitView
            minZoom={0.2}
            maxZoom={1.8}
            colorMode={resolvedTheme}
            aria-label={m.rules_canvas_aria()}
          >
            <MiniMap pannable zoomable nodeColor={minimapColor} />
            <Controls showInteractive={false} />
            <Background gap={22} size={1} />
          </ReactFlow>
        )}
      </div>
    </div>
  );
}
