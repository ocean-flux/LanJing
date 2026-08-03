//! NativeRuleEditorSession 行为测试。
//!
//! 覆盖：loadDocument skeleton、createTemplate、save双域merge、undo/redo、
//! dirty分域、credential不进history、flowProjection响应式、cleanup。

import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

import { NativeRuleEditorSession } from './session.svelte';
import type { NativeRuleDocumentSummary, SaveNativeRuleDocumentOutcome } from './wire';

// ---------------------------------------------------------------------------
// helper
// ---------------------------------------------------------------------------

function mockSummary(
  overrides: Partial<NativeRuleDocumentSummary> = {},
): NativeRuleDocumentSummary {
  return {
    document_id: 'doc:1',
    format: 'native_rule',
    title: '测试规则',
    source_identity: 'source:test',
    state: 'draft',
    semantic_revision: 1,
    layout_revision: 1,
    link_revision: 0,
    created_at_ms: 1_700_000_000_000,
    updated_at_ms: 1_700_000_000_000,
    ...overrides,
  };
}

/** 模拟后端 save 返回的 outcome。 */
function mockOutcome(
  overrides: Partial<SaveNativeRuleDocumentOutcome> = {},
): SaveNativeRuleDocumentOutcome {
  return {
    document_id: 'doc:1',
    semantic: { revision: 2, conflict: null },
    layout: { revision: 2, conflict: null },
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// NativeRuleEditorSession
// ---------------------------------------------------------------------------

describe('NativeRuleEditorSession', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  // -----------------------------------------------------------------------
  // loadDocument
  // -----------------------------------------------------------------------

  it('loadDocument reads summary from wire and builds skeleton state', async () => {
    const summary = mockSummary();
    invoke.mockResolvedValueOnce({ summary });

    const session = new NativeRuleEditorSession();
    await session.loadDocument('doc:1');

    expect(invoke).toHaveBeenCalledWith('get_native_rule_document', {
      request: { document_id: 'doc:1' },
    });
    expect(session.documentId).toBe('doc:1');
    expect(session.title).toBe('测试规则');
    // skeleton: blank definition with source_identity
    expect(session.definition.source_identity).toBe('source:test');
    expect(session.dirty).toEqual({ semantic: false, layout: false });
  });

  it('loadDocument on missing doc throws', async () => {
    invoke.mockResolvedValueOnce(null);
    const session = new NativeRuleEditorSession();
    await expect(session.loadDocument('doc:missing')).rejects.toThrow('不存在');
  });

  // -----------------------------------------------------------------------
  // createTemplate
  // -----------------------------------------------------------------------

  it('createTemplate calls wire and builds skeleton state', async () => {
    const summary = mockSummary();
    invoke.mockResolvedValueOnce(summary);
    invoke.mockResolvedValueOnce({
      summary,
      semantic_revision: summary.semantic_revision,
      layout_revision: summary.layout_revision,
      definition: null,
      layout_json: null,
      provenance: null,
    });

    const session = new NativeRuleEditorSession();
    await session.createTemplate({
      title: 'My Rule',
      intent: 'Search' as const,
      dataType: 'html' as const,
      baseUrl: 'https://example.test',
    });

    expect(invoke).toHaveBeenCalledWith('create_native_rule_document', {
      request: {
        mode: {
          kind: 'template',
          title: 'My Rule',
          intent: 'Search',
          data_type: 'html',
          base_url: 'https://example.test',
        },
      },
    });
    expect(session.title).toBe('测试规则');
    expect(session.definition.source_identity).toBe('source:test');
    expect(session.definition.base_url).toBe('https://example.test');
    expect(session.dirty).toEqual({ semantic: false, layout: false });
  });

  // -----------------------------------------------------------------------
  // createBlank
  // -----------------------------------------------------------------------

  it('createBlank calls wire with blank mode', async () => {
    const summary = mockSummary();
    invoke.mockResolvedValueOnce(summary);
    invoke.mockResolvedValueOnce({
      summary,
      semantic_revision: summary.semantic_revision,
      layout_revision: summary.layout_revision,
      definition: null,
      layout_json: null,
      provenance: null,
    });

    const session = new NativeRuleEditorSession();
    await session.createBlank();

    expect(invoke).toHaveBeenCalledWith('create_native_rule_document', {
      request: { mode: { kind: 'blank' } },
    });
    expect(session.documentId).toBe('doc:1');
  });

  // -----------------------------------------------------------------------
  // dispatch & undo/redo
  // -----------------------------------------------------------------------

  it('dispatch applies action via core reduce', () => {
    const session = new NativeRuleEditorSession();
    // 先 load 一个文档
    invoke.mockResolvedValueOnce({ summary: mockSummary() });
    invoke.mockResolvedValueOnce(mockOutcome());

    // 用 loadDocument 跳过 first load
    session.dispatch({ kind: 'setField', field: 'base_url', value: 'https://changed.test' });
    expect(session.definition.base_url).toBe('https://changed.test');
    expect(session.dirtySemantic).toBe(true);
  });

  it('undo/redo work', () => {
    const session = new NativeRuleEditorSession();
    session.dispatch({ kind: 'setField', field: 'base_url', value: 'https://a.test' });
    session.dispatch({ kind: 'setField', field: 'base_url', value: 'https://b.test' });

    expect(session.definition.base_url).toBe('https://b.test');
    expect(session.canUndo).toBe(true);

    session.undo();
    expect(session.definition.base_url).toBe('https://a.test');

    session.undo();
    expect(session.definition.base_url).toBe('');
    expect(session.canUndo).toBe(false);

    session.redo();
    expect(session.definition.base_url).toBe('https://a.test');
    expect(session.canRedo).toBe(true);
  });

  it('redo stacks empty after new semantic action', () => {
    const session = new NativeRuleEditorSession();
    session.dispatch({ kind: 'setField', field: 'base_url', value: 'https://a.test' });
    session.undo();
    expect(session.canRedo).toBe(true);

    // 新 action 清空 redo
    session.dispatch({ kind: 'setField', field: 'base_url', value: 'https://c.test' });
    expect(session.canRedo).toBe(false);
  });

  // -----------------------------------------------------------------------
  // save: 双域 merge
  // -----------------------------------------------------------------------

  it('save calls wire with semantic+layout payloads', async () => {
    const session = new NativeRuleEditorSession();
    invoke.mockResolvedValueOnce({
      summary: mockSummary({ semantic_revision: 1, layout_revision: 1 }),
    });
    invoke.mockResolvedValueOnce(mockOutcome());
    await session.loadDocument('doc:1'); // getNativeRuleDocument

    // 修改 semantic（dirtySemantic → true）
    session.dispatch({ kind: 'setField', field: 'base_url', value: 'https://new.test' });

    // save 触发 saveRequest + wire + saveResponse
    // 先 mock save 调用
    invoke.mockClear();
    invoke.mockResolvedValueOnce(mockOutcome());
    const outcome = await session.save();

    expect(invoke).toHaveBeenCalledWith('save_native_rule_document', {
      request: {
        document_id: 'doc:1',
        semantic: {
          expected_revision: 1,
          definition: expect.any(Object),
          credential_mutations: [],
        },
        layout: null,
      },
    });

    expect(outcome.semantic?.revision).toBe(2);
    // saveResponse 后 dirty 被清理（编辑后无新 drift）
    expect(session.dirtySemantic).toBe(false);
  });

  it('save is idempotent when nothing dirty', async () => {
    const session = new NativeRuleEditorSession();
    invoke.mockResolvedValueOnce({ summary: mockSummary() });
    await session.loadDocument('doc:1');

    invoke.mockClear();
    const outcome = await session.save();
    expect(invoke).not.toHaveBeenCalledWith('save_native_rule_document', expect.anything());
    expect(outcome.semantic?.revision).toBe(1);
  });

  it('saveResponse conflict: keeps dirty', async () => {
    const session = new NativeRuleEditorSession();
    invoke.mockResolvedValueOnce({ summary: mockSummary() });
    await session.loadDocument('doc:1');

    session.dispatch({ kind: 'setField', field: 'base_url', value: 'https://new.test' });
    invoke.mockClear();
    invoke.mockResolvedValueOnce({
      document_id: 'doc:1',
      semantic: { revision: 1, conflict: { expected: 1, current: 5 } },
      layout: null,
    });

    await session.save();
    // conflict 后 semantic dirty 保持
    expect(session.dirtySemantic).toBe(true);
    // conflict 快照
    expect(session.conflict.semantic).toBeTruthy();
  });

  describe('dirty分域', () => {
    it('layout action only sets layout dirty', () => {
      const session = new NativeRuleEditorSession();
      session.dispatch({ kind: 'moveNode', nodeId: 'node:http', position: { x: 10, y: 20 } });
      expect(session.dirtySemantic).toBe(false);
      expect(session.dirtyLayout).toBe(true);
    });

    it('semantic action only sets semantic dirty', () => {
      const session = new NativeRuleEditorSession();
      session.dispatch({ kind: 'setField', field: 'base_url', value: 'https://x.test' });
      expect(session.dirtySemantic).toBe(true);
      expect(session.dirtyLayout).toBe(false);
    });

    it('mixed actions set both dirty', () => {
      const session = new NativeRuleEditorSession();
      session.dispatch({ kind: 'setField', field: 'base_url', value: 'https://x.test' });
      session.dispatch({ kind: 'moveNode', nodeId: 'node:http', position: { x: 10, y: 20 } });
      expect(session.dirtySemantic).toBe(true);
      expect(session.dirtyLayout).toBe(true);
    });
  });

  // -----------------------------------------------------------------------
  // credential 不进 dirty 变化外的 history
  // -----------------------------------------------------------------------

  it('credentialReplace does not enter history', () => {
    const session = new NativeRuleEditorSession();
    session.dispatch({
      kind: 'credentialReplace',
      nodeId: 'node:http',
      jsonPointer: '/headers/authorization',
      logicalName: 'auth',
      value: 'sk-secret',
    });
    // credential 不产生 history 条目（但在 pendingCredentialMutations 中）
    expect(session.canUndo).toBe(false);
    expect(session.pendingCredentialMutations).toHaveLength(1);
  });

  // -----------------------------------------------------------------------
  // flowProjection 派生
  // -----------------------------------------------------------------------

  it('flowProjection returns current nodes/edges', () => {
    const session = new NativeRuleEditorSession();
    // blank → 0 nodes
    const p0 = session.flowProjection;
    expect(p0.nodes).toHaveLength(0);

    // add a node
    session.addNode('http');
    const p1 = session.flowProjection;
    expect(p1.nodes).toHaveLength(1);
    expect(p1.nodes[0].type).toBe('http');
  });

  it('flowProjectionSimple works without layout', () => {
    const session = new NativeRuleEditorSession();
    session.addNode('mapper');
    const p = session.flowProjectionSimple;
    expect(p.nodes).toHaveLength(1);
    expect(p.nodes[0].type).toBe('mapper');
  });

  // -----------------------------------------------------------------------
  // validate & prepare
  // -----------------------------------------------------------------------

  it('validate calls wire and sets diagnostics', async () => {
    const session = new NativeRuleEditorSession();
    invoke.mockResolvedValueOnce({ summary: mockSummary() });
    await session.loadDocument('doc:1');

    invoke.mockClear();
    invoke.mockResolvedValueOnce({
      revision: 1,
      definition_hash: 'hash:def',
      plan_hash: 'hash:plan',
      diagnostics: [],
      profile: {
        id: 'source:test',
        title: '',
        icon_url: null,
        version: null,
        group: null,
        supported_intents: [],
        risk_notes: [],
      },
      capability: { network: false, system: { fs: false, env: false, process: false } },
    });

    const preview = await session.validate();
    expect(invoke).toHaveBeenCalledWith('validate_native_rule_document', {
      request: { document_id: 'doc:1', revision: 1 },
    });
    expect(preview.revision).toBe(1);
    expect(session.diagnostics).toEqual([]);
  });

  it('prepare calls wire after auto-save', async () => {
    const session = new NativeRuleEditorSession();
    invoke.mockResolvedValueOnce({ summary: mockSummary() });
    await session.loadDocument('doc:1');

    // make dirty, prepare should auto-save first
    session.dispatch({ kind: 'setField', field: 'base_url', value: 'https://new.test' });

    invoke.mockClear();
    invoke.mockResolvedValueOnce(mockOutcome()); // save
    invoke.mockResolvedValueOnce({
      id: 'candidate:1',
      expected_installed_revision: 0,
      profile: {
        id: 'source:test',
        title: '',
        icon_url: null,
        version: null,
        group: null,
        supported_intents: [],
        risk_notes: [],
      },
      required_grant: { network: false, system: { fs: false, env: false, process: false } },
      diagnostics: [],
      definition_hash: 'hash:def',
      plan_hash: 'hash:plan',
      expires_at_ms: 1_700_000_000_000,
    });

    const candidate = await session.prepare();
    expect(invoke).toHaveBeenCalledWith('save_native_rule_document', expect.anything());
    expect(invoke).toHaveBeenCalledWith('prepare_native_rule_document', {
      request: { document_id: 'doc:1', revision: 2 },
    });
    expect(candidate.id).toBe('candidate:1');
  });

  // -----------------------------------------------------------------------
  // hasUnsavedChanges
  // -----------------------------------------------------------------------

  it('hasUnsavedChanges reflects dirty state', () => {
    const session = new NativeRuleEditorSession();
    expect(session.hasUnsavedChanges).toBe(false);
    session.dispatch({ kind: 'setField', field: 'base_url', value: 'https://x.test' });
    expect(session.hasUnsavedChanges).toBe(true);
    // undo 后 core 的 markSemanticDirty 仍保持 dirty=true（设计如此，参见 core.ts undo 函数）
    session.undo();
    expect(session.hasUnsavedChanges).toBe(true);
  });

  // -----------------------------------------------------------------------
  // addNode / deleteNode
  // -----------------------------------------------------------------------

  it('addNode returns new id and alters projection', () => {
    const session = new NativeRuleEditorSession();
    const id = session.addNode('http');
    expect(id).toBeTruthy();
    expect(typeof id).toBe('string');
    expect(session.definition.flow.nodes).toHaveLength(1);
    expect(session.dirtySemantic).toBe(true);
  });

  it('deleteNode removes node and incident edges', () => {
    const session = new NativeRuleEditorSession();
    const id = session.addNode('http');
    session.addNode('mapper');
    session.connect({
      from: { node_id: id, handle: 'out' },
      to: { node_id: session.definition.flow.nodes[1].id, handle: 'in' },
    });
    expect(session.definition.flow.nodes).toHaveLength(2);
    expect(session.definition.flow.edges).toHaveLength(1);

    session.deleteNode(id);
    expect(session.definition.flow.nodes).toHaveLength(1);
    expect(session.definition.flow.edges).toHaveLength(0);
  });

  it('node add/delete and reconnect are undoable as semantic commands', () => {
    const session = new NativeRuleEditorSession();
    const first = session.addNode('http');
    const second = session.addNode('mapper');
    const original: FlowEdge = {
      from: { node_id: first, handle: 'out' },
      to: { node_id: second, handle: 'in' },
    };
    const replacement: FlowEdge = {
      from: { node_id: first, handle: 'out' },
      to: { node_id: second, handle: 'input' },
    };

    session.connect(original);
    session.reconnect(original, replacement);
    expect(session.definition.flow.edges).toEqual([replacement]);
    session.undo();
    expect(session.definition.flow.edges).toEqual([original]);

    session.deleteNode(first);
    expect(session.definition.flow.nodes).toHaveLength(1);
    session.undo();
    expect(session.definition.flow.nodes).toHaveLength(2);
    expect(session.definition.flow.edges).toEqual([original]);
  });

  // -----------------------------------------------------------------------
  // select / focus / connect utility
  // -----------------------------------------------------------------------

  it('select/focus do not affect dirty', () => {
    const session = new NativeRuleEditorSession();
    session.selectNode('node:http');
    expect(session.selection).toBe('node:http');
    expect(session.dirtySemantic).toBe(false);

    session.focusIntent('Search' as const);
    expect(session.intentFocus).toBe('Search');
    expect(session.dirtySemantic).toBe(false);
  });

  it('selectEdge uses transient edge selection token', () => {
    const session = new NativeRuleEditorSession();
    session.selectEdge('node:a:output->node:b:input');
    expect(session.selection).toBe('edge:node:a:output->node:b:input');
    expect(session.canUndo).toBe(false);
  });

  it('connect adds edge', () => {
    const session = new NativeRuleEditorSession();
    const id1 = session.addNode('http');
    const id2 = session.addNode('mapper');
    session.connect({ from: { node_id: id1, handle: 'out' }, to: { node_id: id2, handle: 'in' } });
    expect(session.definition.flow.edges).toHaveLength(1);
  });

  it('disconnect removes edge', () => {
    const session = new NativeRuleEditorSession();
    const id1 = session.addNode('http');
    const id2 = session.addNode('mapper');
    const edge: FlowEdge = {
      from: { node_id: id1, handle: 'out' },
      to: { node_id: id2, handle: 'in' },
    };
    session.connect(edge);
    expect(session.definition.flow.edges).toHaveLength(1);
    session.disconnect(edge);
    expect(session.definition.flow.edges).toHaveLength(0);
  });
});

// 需要重用引入 FlowEdge 类型。
import type { FlowEdge } from './wire';
