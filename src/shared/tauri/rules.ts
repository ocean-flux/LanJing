import { invoke, isTauri } from '@tauri-apps/api/core';

export type StandardIntent =
  | 'Search'
  | 'Discover'
  | 'ResolveItem'
  | 'ListUnits'
  | 'ResolveAsset'
  | 'ContinueAction';

export type FlowNodeKind = 'http' | 'js' | 'extract' | 'mapper' | 'merge' | 'condition' | 'loop';

export interface FlowNode {
  id: string;
  config: { kind: FlowNodeKind; value: Record<string, unknown> };
}

export interface FlowEdge {
  from: { node_id: string; handle: string };
  to: { node_id: string; handle: string };
}

export interface RuleDefinition {
  contract: 'rule_definition';
  schema_version: 1;
  source_identity: { id: string } | string;
  base_url: string;
  intent_exports: Partial<Record<StandardIntent, { flow_entry: string; mapper_output: string }>>;
  flow: { nodes: FlowNode[]; edges: FlowEdge[] };
}

export interface NativeRuleDocumentSummary {
  document_id: string;
  format: string;
  title: string;
  source_identity: string;
  state: 'draft' | 'linked';
  semantic_revision: number;
  layout_revision: number;
  link_revision: number;
  created_at_ms: number;
  updated_at_ms: number;
}

export interface NativeRuleDocumentDetail {
  summary: NativeRuleDocumentSummary;
  semantic_revision: number;
  layout_revision: number;
  definition: RuleDefinition | null;
  layout_json: string | null;
}

export interface RuleLayout {
  nodes: Record<string, { position: { x: number; y: number }; collapsed: boolean }>;
  loopRegions?: Record<string, { collapsed: boolean }>;
}

export function listNativeRuleDocuments(): Promise<NativeRuleDocumentSummary[]> {
  if (!isTauri()) return Promise.resolve([]);
  return invoke<NativeRuleDocumentSummary[]>('list_native_rule_documents', { request: {} });
}

export function getNativeRuleDocument(
  documentId: string,
): Promise<NativeRuleDocumentDetail | null> {
  if (!isTauri()) return Promise.resolve(null);
  return invoke<NativeRuleDocumentDetail | null>('get_native_rule_document', {
    request: { document_id: documentId },
  });
}

export function saveRuleLayout(
  documentId: string,
  expectedRevision: number,
  layout: RuleLayout,
): Promise<{ layout?: { revision: number; conflict?: { expected: number; current: number } } }> {
  return invoke('save_native_rule_document', {
    request: {
      document_id: documentId,
      semantic: null,
      layout: {
        expected_revision: expectedRevision,
        layout_json: JSON.stringify(layout),
      },
    },
  });
}

export function parseRuleLayout(raw: string | null): RuleLayout {
  if (!raw) return { nodes: {} };
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== 'object') return { nodes: {} };
    const { nodes } = parsed as { nodes?: unknown };
    return nodes && typeof nodes === 'object' ? (parsed as RuleLayout) : { nodes: {} };
  } catch {
    return { nodes: {} };
  }
}
