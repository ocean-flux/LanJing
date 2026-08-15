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
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Icon, type IconName } from '@/components/Icon';
import { PageToolbar } from '@/components/PageToolbar';
import { Button } from '@/components/ui/button';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select';
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

/** 节点数超过该阈值才渲染缩略图，小图不占画布空间。 */
const MINIMAP_NODE_THRESHOLD = 12;

const NODE_ICONS: Record<FlowNodeKind, IconName> = {
  http: 'broadcast',
  js: 'code',
  extract: 'tree-structure',
  mapper: 'translate',
  merge: 'git-merge',
  condition: 'compass',
  loop: 'arrow-counter-clockwise',
};

function nodeLabels(m: Messages): Record<FlowNodeKind, string> {
  return {
    http: m.rules_node_inspector_type_http(),
    js: m.rules_node_inspector_type_js(),
    extract: m.rules_node_inspector_type_extract(),
    mapper: m.rules_node_inspector_type_mapper(),
    merge: m.rules_node_inspector_type_merge(),
    condition: m.rules_node_inspector_type_condition(),
    loop: m.rules_node_inspector_type_loop(),
  };
}

interface RuleNodeData {
  [key: string]: unknown;
  kind: FlowNodeKind;
  label: string;
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
  return (
    <div
      aria-label={`${data.label}: ${data.summary}`}
      className={`w-56 border bg-surface-1 ${selected ? 'border-lantern-strong ring-1 ring-lantern-strong/40' : 'border-hairline-strong'}`}
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
      <div className="flex h-(--density-row) items-center gap-2 border-b border-hairline px-2">
        <Icon name={NODE_ICONS[data.kind]} className="text-base text-lantern-strong" />
        <span className="truncate font-medium">{data.label}</span>
        <span className="ml-auto shrink-0 font-mono text-ui-sm text-ink-subtle">{data.kind}</span>
      </div>
      <p className="px-2 py-1.5 font-mono text-ui-sm break-words text-ink-muted">{data.summary}</p>
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
  return nodeLabels(m)[kind];
}

function createGraph(definition: RuleDefinition, layout: RuleLayout, m: Messages) {
  const labels = nodeLabels(m);
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
      x: 48 + (index % 3) * 288,
      y: 48 + Math.floor(index / 3) * 160,
    },
    data: {
      kind: node.config.kind,
      label: labels[node.config.kind],
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

/**
 * 规则工作区。当前是语义图的只读投影 + 布局保存；
 * 节点面板、检查器、连接校验与撤销重做仍在迁移（阶段 C/D）。
 */
export function RuleWorkspace({ documentId }: { documentId?: string }) {
  const m = useMessages();
  const navigate = useNavigate();
  const { resolvedTheme } = useTheme();
  const [documents, setDocuments] = useState<NativeRuleDocumentSummary[]>([]);
  const [selectedDocumentId, setSelectedDocumentId] = useState(documentId ?? '');
  const [detail, setDetail] = useState<NativeRuleDocumentDetail>();
  const [edges, setEdges] = useState<Edge[]>([]);
  const [nodes, setNodes, onNodesChange] = useNodesState<RuleFlowNode>([]);
  const [status, setStatus] = useState<'loading' | 'ready' | 'saving' | 'error'>('loading');
  const [message, setMessage] = useState('');
  const [isLayoutDirty, setIsLayoutDirty] = useState(false);
  const currentDocumentId = useRef(documentId ?? '');

  const loadDocument = useCallback(
    async (id: string, signal?: AbortSignal) => {
      setStatus('loading');
      setMessage('');
      try {
        const nextDetail = await getNativeRuleDocument(id);
        if (signal?.aborted || currentDocumentId.current !== id) return;
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
        if (controller.signal.aborted) return;
        setDocuments(items);
        const requested =
          documentId && items.some((item) => item.document_id === documentId)
            ? documentId
            : (items[0]?.document_id ?? '');
        currentDocumentId.current = requested;
        setSelectedDocumentId(requested);
        if (requested) void loadDocument(requested, controller.signal);
        else setStatus('ready');
      })
      .catch((error: unknown) => {
        if (controller.signal.aborted) return;
        setStatus('error');
        setMessage(error instanceof Error ? error.message : String(error));
      });
    return () => controller.abort();
  }, [documentId, loadDocument]);

  const handleDocumentChange = (nextId: string) => {
    if (isLayoutDirty && !window.confirm(m.rules_switch_document_confirm())) return;
    currentDocumentId.current = nextId;
    setSelectedDocumentId(nextId);
    navigate(`/sources/rules/${encodeURIComponent(nextId)}`);
  };

  const saveLayout = async () => {
    if (!detail) return;
    const targetId = detail.summary.document_id;
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
      const result = await saveRuleLayout(targetId, detail.layout_revision, layout);
      if (currentDocumentId.current !== targetId) return;
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

  const minimapColor = useMemo(() => () => 'var(--lantern-strong)', []);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <PageToolbar
        meta={
          documents.length > 0 ? m.rules_document_count({ count: documents.length }) : undefined
        }
        actions={
          <Button
            size="sm"
            onClick={() => void saveLayout()}
            disabled={!detail || !isLayoutDirty || status === 'saving'}
          >
            <Icon name="floppy-disk" className="text-base" />
            {status === 'saving' ? m.rules_save_layout_saving() : m.rules_save_layout()}
          </Button>
        }
      >
        <NativeSelect
          size="sm"
          className="ml-2 min-w-56"
          aria-label={m.rules_document_label()}
          value={selectedDocumentId}
          onChange={(event) => handleDocumentChange(event.target.value)}
          disabled={status === 'loading' || documents.length === 0}
        >
          {documents.length === 0 ? (
            <NativeSelectOption value="">{m.rules_no_documents_option()}</NativeSelectOption>
          ) : null}
          {documents.map((document) => (
            <NativeSelectOption key={document.document_id} value={document.document_id}>
              {document.title}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      </PageToolbar>

      {message ? (
        <p
          role={status === 'error' ? 'alert' : 'status'}
          className={`border-b border-hairline px-(--page-gutter) py-1.5 text-ui-sm ${status === 'error' ? 'text-danger' : 'text-ink-muted'}`}
        >
          {message}
        </p>
      ) : null}

      <div className="min-h-0 flex-1 bg-canvas">
        {nodes.length === 0 ? (
          <Empty className="h-full">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <Icon name="tree-structure" className="text-base" />
              </EmptyMedia>
              <EmptyTitle>
                {status === 'loading' ? m.rules_canvas_loading() : m.rules_canvas_empty()}
              </EmptyTitle>
              <EmptyDescription>{m.rules_canvas_empty_hint()}</EmptyDescription>
            </EmptyHeader>
          </Empty>
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
            snapGrid={[16, 16]}
            snapToGrid
            fitView
            minZoom={0.2}
            maxZoom={2}
            colorMode={resolvedTheme}
            proOptions={{ hideAttribution: true }}
            aria-label={m.rules_canvas_aria()}
          >
            {nodes.length > MINIMAP_NODE_THRESHOLD ? (
              <MiniMap pannable zoomable nodeColor={minimapColor} />
            ) : null}
            <Controls showInteractive={false} />
            <Background gap={16} size={1} />
          </ReactFlow>
        )}
      </div>
    </div>
  );
}
