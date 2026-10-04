//! createRuleEditorSession 行为测试。
//!
//! 覆盖：loadDocument skeleton、createTemplate、save双域merge、undo/redo、
//! dirty分域、credential不进history、flowProjection响应式、cleanup。

import { beforeEach, describe, expect, it, vi } from 'vitest';

import { cloneJson, createBlankDefinition } from './core';
import { getNodePorts } from './ports';
import {
  createRuleEditorSession,
  selectCanRedo,
  selectCanUndo,
  selectFlowProjection,
  selectHasUnsavedChanges,
  selectIsSaving,
} from './session';
import type {
  FlowEdge,
  FlowNodeKind,
  NativeRuleDocumentDetail,
  NativeRuleDocumentSummary,
  RuleDefinition,
  SaveNativeRuleDocumentOutcome,
  SaveNativeRuleDocumentRequest,
} from '@/shared/tauri/rules';

const invoke = vi.hoisted(() => vi.fn<(command: string, args?: unknown) => Promise<unknown>>());

// 读路径带 isTauri 守卫；测试里固定为运行在 Tauri 中。
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => true }));

// ---------------------------------------------------------------------------
// Helper
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

function mockDefinition(overrides: Partial<RuleDefinition> = {}): RuleDefinition {
  return { ...createBlankDefinition('source:test'), ...overrides };
}

function mockDetail(overrides: Partial<NativeRuleDocumentDetail> = {}): NativeRuleDocumentDetail {
  const summary = overrides.summary ?? mockSummary();
  return {
    summary,
    semantic_revision: summary.semantic_revision,
    effective_semantic_revision: summary.semantic_revision,
    layout_revision: summary.layout_revision,
    definition: mockDefinition(),
    layout_json: null,
    provenance: null,
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

/**
 * 含未安装能力节点的 Definition。
 *
 * `payloadText` 是作者写入的 opaque payload 原文（键序敏感），用于断言保存/重开
 * 之后逐字节一致。
 */
function definitionWithOpaqueNode(payloadText: string): RuleDefinition {
  const definition = cloneJson(mockDefinition());
  definition.flow.nodes = [
    { id: 'node:custom', config: { kind: 'custom_reader', value: JSON.parse(payloadText) } },
    {
      id: 'node:http',
      config: { kind: 'http', value: { method: 'GET', url: 'https://a.test' } },
    },
  ];
  return definition;
}

// ---------------------------------------------------------------------------
// CreateRuleEditorSession
// ---------------------------------------------------------------------------

describe('createRuleEditorSession', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  // -----------------------------------------------------------------------
  // LoadDocument
  // -----------------------------------------------------------------------

  it('loadDocument reads saved Definition and layout from wire', async () => {
    const summary = mockSummary();
    invoke.mockResolvedValueOnce(mockDetail({ summary }));

    const session = createRuleEditorSession();
    const state = () => session.getState();
    await state().loadDocument('doc:1');

    expect(invoke).toHaveBeenCalledWith('get_native_rule_document', {
      request: { document_id: 'doc:1' },
    });
    expect(state().summary?.document_id ?? null).toBe('doc:1');
    expect(state().summary?.title ?? null).toBe('测试规则');
    expect(state().core.definition.source_identity).toBe('source:test');
    expect(state().core.definition.flow.nodes).toEqual([]);
    expect(state().core.dirty).toEqual({ semantic: false, layout: false });
  });

  it('loadDocument rejects missing semantic snapshot instead of creating blank Definition', async () => {
    invoke.mockResolvedValueOnce(mockDetail({ definition: null }));
    const session = createRuleEditorSession();
    const state = () => session.getState();
    await expect(state().loadDocument('doc:1')).rejects.toThrow('document_semantic_missing');
  });

  it('loadDocument on missing doc throws', async () => {
    invoke.mockResolvedValueOnce(null);
    const session = createRuleEditorSession();
    const state = () => session.getState();
    await expect(state().loadDocument('doc:missing')).rejects.toThrow('document_not_found');
  });

  // -----------------------------------------------------------------------
  // CreateTemplate
  // -----------------------------------------------------------------------

  it('createTemplate calls wire and builds saved state', async () => {
    const summary = mockSummary();
    invoke.mockResolvedValueOnce(summary);
    invoke.mockResolvedValueOnce(
      mockDetail({
        summary,
        definition: mockDefinition({ base_url: 'https://example.test' }),
      }),
    );

    const session = createRuleEditorSession();
    const state = () => session.getState();
    await state().createTemplate({
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
    expect(state().summary?.title ?? null).toBe('测试规则');
    expect(state().core.definition.source_identity).toBe('source:test');
    expect(state().core.definition.base_url).toBe('https://example.test');
    expect(state().core.dirty).toEqual({ semantic: false, layout: false });
  });

  // -----------------------------------------------------------------------
  // CreateBlank
  // -----------------------------------------------------------------------

  it('createBlank calls wire with blank mode', async () => {
    const summary = mockSummary();
    invoke.mockResolvedValueOnce(summary);
    invoke.mockResolvedValueOnce(mockDetail({ summary }));

    const session = createRuleEditorSession();
    const state = () => session.getState();
    await state().createBlank();

    expect(invoke).toHaveBeenCalledWith('create_native_rule_document', {
      request: { mode: { kind: 'blank' } },
    });
    expect(state().summary?.document_id ?? null).toBe('doc:1');
  });

  // -----------------------------------------------------------------------
  // Dispatch & undo/redo
  // -----------------------------------------------------------------------

  it('dispatch applies action via core reduce', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    invoke.mockResolvedValueOnce(mockDetail());
    invoke.mockResolvedValueOnce(mockOutcome());

    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://changed.test' });
    expect(state().core.definition.base_url).toBe('https://changed.test');
    expect(state().core.dirty.semantic).toBe(true);
  });

  it('undo/redo work', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://a.test' });
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://b.test' });

    expect(state().core.definition.base_url).toBe('https://b.test');
    expect(selectCanUndo(state())).toBe(true);

    state().undo();
    expect(state().core.definition.base_url).toBe('https://a.test');

    state().undo();
    expect(state().core.definition.base_url).toBe('');
    expect(selectCanUndo(state())).toBe(false);

    state().redo();
    expect(state().core.definition.base_url).toBe('https://a.test');
    expect(selectCanRedo(state())).toBe(true);
  });

  it('redo stacks empty after new semantic action', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://a.test' });
    state().undo();
    expect(selectCanRedo(state())).toBe(true);

    // 新 action 清空 redo
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://c.test' });
    expect(selectCanRedo(state())).toBe(false);
  });

  // -----------------------------------------------------------------------
  // Save: 双域 merge
  // -----------------------------------------------------------------------

  it('save calls wire with semantic+layout payloads', async () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    invoke.mockResolvedValueOnce(
      mockDetail({ summary: mockSummary({ semantic_revision: 1, layout_revision: 1 }) }),
    );
    await state().loadDocument('doc:1');

    // 修改 semantic（dirtySemantic → true）
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://new.test' });

    // Save 触发 saveRequest + wire + saveResponse
    // 先 mock save 调用
    invoke.mockClear();
    invoke.mockResolvedValueOnce(mockOutcome());
    const outcome = await state().save();

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
    // SaveResponse 后 dirty 被清理（编辑后无新 drift）
    expect(state().core.dirty.semantic).toBe(false);
  });

  it('save keeps effective revision when semantic result is a draft', async () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    invoke.mockResolvedValueOnce(mockDetail({ effective_semantic_revision: 1 }));
    await state().loadDocument('doc:1');
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://invalid.test' });

    invoke.mockClear();
    invoke.mockResolvedValueOnce(
      mockOutcome({
        semantic: { revision: 2, conflict: null, activation: 'draft' },
        layout: null,
      }),
    );
    await state().save();

    expect(state().effectiveSemanticRevision).toBe(1);
  });

  it('save is idempotent when nothing dirty', async () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    invoke.mockResolvedValueOnce(mockDetail());
    await state().loadDocument('doc:1');

    invoke.mockClear();
    const outcome = await state().save();
    expect(invoke).not.toHaveBeenCalledWith('save_native_rule_document', expect.anything());
    expect(outcome.semantic?.revision).toBe(1);
  });

  it('save failure clears isSaving but keeps semantic dirty', async () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    invoke.mockResolvedValueOnce(mockDetail());
    await state().loadDocument('doc:1');
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://failed.test' });

    invoke.mockRejectedValueOnce(new Error('storage unavailable'));
    await expect(state().save()).rejects.toThrow('storage unavailable');
    expect(selectIsSaving(state())).toBe(false);
    expect(state().core.dirty.semantic).toBe(true);
  });

  it('saveResponse conflict: keeps dirty', async () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    invoke.mockResolvedValueOnce(mockDetail());
    await state().loadDocument('doc:1');

    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://new.test' });
    invoke.mockClear();
    invoke.mockResolvedValueOnce({
      document_id: 'doc:1',
      semantic: { revision: 1, conflict: { expected: 1, current: 5 } },
      layout: null,
    });

    await state().save();
    // Conflict 后 semantic dirty 保持
    expect(state().core.dirty.semantic).toBe(true);
    // Conflict 快照按 Rust 返回原样保留
    expect(state().core.conflict.semantic).toEqual({ expected: 1, current: 5 });
  });

  // -----------------------------------------------------------------------
  // 未安装能力节点：展示 / 编辑 / 显式保存 / 重新打开的 round-trip
  // -----------------------------------------------------------------------

  it('未安装能力节点的 opaque payload 经编辑与显式保存后原样重新打开', async () => {
    const payloadText = '{"selector":".entry","flags":["a","b"],"nested":{"depth":2}}';
    const session = createRuleEditorSession();
    const state = () => session.getState();

    invoke.mockResolvedValueOnce(
      mockDetail({ definition: definitionWithOpaqueNode(payloadText), semantic_revision: 1 }),
    );
    await state().loadDocument('doc:1');

    // 展示：投影保留未安装能力节点与作者原文；descriptor 查不到 → 空端口而不是崩溃。
    const projected = selectFlowProjection(state()).nodes;
    expect(projected.map((node) => node.type)).toEqual(['custom_reader', 'http']);
    expect(getNodePorts('custom_reader', { selector: '.entry' })).toEqual({
      inputs: [],
      outputs: [],
    });

    // 编辑：移动未知节点、在它旁边新增普通节点；未知节点的 payload 不被改写。
    state().moveNode('node:custom', { x: 120, y: 40 });
    const addedId = state().addNode('mapper');

    invoke.mockClear();
    invoke.mockResolvedValueOnce(
      mockOutcome({
        semantic: { revision: 2, conflict: null, activation: 'draft' },
        layout: { revision: 2, conflict: null },
      }),
    );
    await state().save();

    const [[, saveArgs]] = invoke.mock.calls;
    const { request } = saveArgs as { request: SaveNativeRuleDocumentRequest };
    const savedDefinition = request.semantic?.definition as RuleDefinition;
    const savedNode = savedDefinition.flow.nodes.find((node) => node.id === 'node:custom');
    expect(savedNode?.config.kind).toBe('custom_reader');
    expect(JSON.stringify(savedNode?.config.value)).toBe(payloadText);
    expect(savedDefinition.flow.nodes.map((node) => node.id)).toEqual([
      'node:custom',
      'node:http',
      addedId,
    ]);

    // 重新打开：按后端回存的 definition 重建，opaque payload 逐字节一致。
    invoke.mockResolvedValueOnce(mockDetail({ definition: savedDefinition, semantic_revision: 2 }));
    await state().loadDocument('doc:1');
    const reopened = state().core.definition.flow.nodes.find((node) => node.id === 'node:custom');
    expect(JSON.stringify(reopened?.config.value)).toBe(payloadText);
  });

  describe('dirty分域', () => {
    it('layout action only sets layout dirty', () => {
      const session = createRuleEditorSession();
      const state = () => session.getState();
      state().dispatch({ kind: 'moveNode', nodeId: 'node:http', position: { x: 10, y: 20 } });
      expect(state().core.dirty.semantic).toBe(false);
      expect(state().core.dirty.layout).toBe(true);
    });

    it('semantic action only sets semantic dirty', () => {
      const session = createRuleEditorSession();
      const state = () => session.getState();
      state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://x.test' });
      expect(state().core.dirty.semantic).toBe(true);
      expect(state().core.dirty.layout).toBe(false);
    });

    it('mixed actions set both dirty', () => {
      const session = createRuleEditorSession();
      const state = () => session.getState();
      state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://x.test' });
      state().dispatch({ kind: 'moveNode', nodeId: 'node:http', position: { x: 10, y: 20 } });
      expect(state().core.dirty.semantic).toBe(true);
      expect(state().core.dirty.layout).toBe(true);
    });
  });

  // -----------------------------------------------------------------------
  // Credential 不进 dirty 变化外的 history
  // -----------------------------------------------------------------------

  it('credentialReplace does not enter history', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    state().dispatch({
      kind: 'credentialReplace',
      nodeId: 'node:http',
      jsonPointer: '/headers/authorization',
      logicalName: 'auth',
      value: 'sk-secret',
    });
    // Credential 不产生 history 条目（但在 pendingCredentialMutations 中）
    expect(selectCanUndo(state())).toBe(false);
    expect(state().core.pendingCredentialMutations).toHaveLength(1);
  });

  // -----------------------------------------------------------------------
  // FlowProjection 派生
  // -----------------------------------------------------------------------

  it('flowProjection returns current nodes/edges', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    // Blank → 0 nodes
    const p0 = selectFlowProjection(state());
    expect(p0.nodes).toHaveLength(0);

    // Add a node
    state().addNode('http');
    const p1 = selectFlowProjection(state());
    expect(p1.nodes).toHaveLength(1);
    expect(p1.nodes[0].type).toBe('http');
  });

  it('flowProjectionSimple works without layout', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    state().addNode('mapper');
    const p = selectFlowProjection(state());
    expect(p.nodes).toHaveLength(1);
    expect(p.nodes[0].type).toBe('mapper');
  });

  // -----------------------------------------------------------------------
  // Validate & prepare
  // -----------------------------------------------------------------------

  it('validate calls wire and sets diagnostics', async () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    invoke.mockResolvedValueOnce(mockDetail());
    await state().loadDocument('doc:1');

    invoke.mockClear();
    invoke.mockResolvedValueOnce({
      valid: true,
      revision: 1,
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

    const preview = await state().validate();
    expect(invoke).toHaveBeenCalledWith('validate_native_rule_document', {
      request: { document_id: 'doc:1', revision: 1 },
    });
    expect(preview.revision).toBe(1);
    expect(state().core.validation.diagnostics).toEqual([]);
  });

  it('validation response becomes stale when semantic edit lands while request is pending', async () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    invoke.mockResolvedValueOnce(mockDetail());
    await state().loadDocument('doc:1');

    let resolvePreview: ((preview: unknown) => void) | undefined;
    const previewPromise = new Promise<unknown>((resolve) => {
      resolvePreview = resolve;
    });
    invoke.mockClear();
    invoke.mockReturnValueOnce(previewPromise);
    const pending = state().validate();
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://drift.test' });
    resolvePreview?.({
      valid: true,
      revision: 1,
      definition_hash: 'hash:def',
      plan_hash: 'hash:plan',
      diagnostics: [],
      profile: null,
      capability: { network: false, system: { fs: false, env: false, process: false } },
    });
    await pending;
    expect(state().core.validation.status).toBe('stale');
  });

  // -----------------------------------------------------------------------
  // HasUnsavedChanges
  // -----------------------------------------------------------------------

  it('hasUnsavedChanges reflects dirty state', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    expect(selectHasUnsavedChanges(state())).toBe(false);
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://x.test' });
    expect(selectHasUnsavedChanges(state())).toBe(true);
    // Undo 后 core 的 markSemanticDirty 仍保持 dirty=true（设计如此，参见 core.ts undo 函数）
    state().undo();
    expect(selectHasUnsavedChanges(state())).toBe(true);
  });

  // -----------------------------------------------------------------------
  // AddNode / deleteNode
  // -----------------------------------------------------------------------

  it('addNode emits closed defaults for every current node kind', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    const requiredFields: Record<FlowNodeKind, string[]> = {
      http: ['method', 'url', 'headers', 'body', 'charset', 'expected_type'],
      js: ['code', 'output'],
      extract: ['rules', 'field_rules', 'expected_type', 'output_target'],
      mapper: ['output', 'identity_fields'],
      merge: ['inputs', 'strategy'],
      condition: ['branches', 'expression'],
      loop: ['collection', 'item_binding', 'index_binding', 'max_iterations'],
    };

    for (const [kind, fields] of Object.entries(requiredFields)) {
      state().addNode(kind as FlowNodeKind);
      const node = state().core.definition.flow.nodes.at(-1);
      expect(Object.keys(node?.config.value ?? {})).toEqual(expect.arrayContaining(fields));
    }
  });

  it('deleteNode removes node and incident edges', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    const id = state().addNode('http');
    state().addNode('mapper');
    state().connect({
      from: { node_id: id, handle: 'out' },
      to: { node_id: state().core.definition.flow.nodes[1].id, handle: 'in' },
    });
    expect(state().core.definition.flow.nodes).toHaveLength(2);
    expect(state().core.definition.flow.edges).toHaveLength(1);

    state().deleteNode(id);
    expect(state().core.definition.flow.nodes).toHaveLength(1);
    expect(state().core.definition.flow.edges).toHaveLength(0);
  });

  it('node add/delete and reconnect are undoable as semantic commands', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    const first = state().addNode('http');
    const second = state().addNode('mapper');
    const original: FlowEdge = {
      from: { node_id: first, handle: 'out' },
      to: { node_id: second, handle: 'in' },
    };
    const replacement: FlowEdge = {
      from: { node_id: first, handle: 'out' },
      to: { node_id: second, handle: 'input' },
    };

    state().connect(original);
    state().reconnect(original, replacement);
    expect(state().core.definition.flow.edges).toEqual([replacement]);
    state().undo();
    expect(state().core.definition.flow.edges).toEqual([original]);

    state().deleteNode(first);
    expect(state().core.definition.flow.nodes).toHaveLength(1);
    state().undo();
    expect(state().core.definition.flow.nodes).toHaveLength(2);
    expect(state().core.definition.flow.edges).toEqual([original]);
  });

  // -----------------------------------------------------------------------
  // Select / focus / connect utility
  // -----------------------------------------------------------------------

  it('select/focus do not affect dirty', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    state().selectNode('node:http');
    expect(state().core.selection).toBe('node:http');
    expect(state().core.dirty.semantic).toBe(false);

    state().focusIntent('Search' as const);
    expect(state().core.intentFocus).toBe('Search');
    expect(state().core.dirty.semantic).toBe(false);
  });

  it('selectEdge uses transient edge selection token', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    state().selectEdge('node:a:output->node:b:input');
    expect(state().core.selection).toBe('edge:node:a:output->node:b:input');
    expect(selectCanUndo(state())).toBe(false);
  });

  it('connect adds edge', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    const id1 = state().addNode('http');
    const id2 = state().addNode('mapper');
    state().connect({ from: { node_id: id1, handle: 'out' }, to: { node_id: id2, handle: 'in' } });
    expect(state().core.definition.flow.edges).toHaveLength(1);
  });

  it('disconnect removes edge', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    const id1 = state().addNode('http');
    const id2 = state().addNode('mapper');
    const edge: FlowEdge = {
      from: { node_id: id1, handle: 'out' },
      to: { node_id: id2, handle: 'in' },
    };
    state().connect(edge);
    expect(state().core.definition.flow.edges).toHaveLength(1);
    state().disconnect(edge);
    expect(state().core.definition.flow.edges).toHaveLength(0);
  });

  it('revealNode 选择节点，并为重复定位递增请求序号', () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();

    state().revealNode('node:http');
    expect(state().core.selection).toBe('node:http');
    expect(state().revealRequest).toEqual({ nodeId: 'node:http', sequence: 1 });

    state().revealNode('node:http');
    expect(state().revealRequest).toEqual({ nodeId: 'node:http', sequence: 2 });
  });

  it('pasteSubgraph 重排 id、重写边端点并按偏移落位', () => {
    let seed = 0;
    const session = createRuleEditorSession({ newNodeId: () => `node:new-${(seed += 1)}` });
    const state = () => session.getState();

    state().pasteSubgraph(
      {
        nodes: [
          { id: 'node:src-a', config: { kind: 'http', value: {} } },
          { id: 'node:src-b', config: { kind: 'mapper', value: { fields: [] } } },
        ],
        edges: [
          {
            from: { node_id: 'node:src-a', handle: 'out' },
            to: { node_id: 'node:src-b', handle: 'in' },
          },
          // 另一端不在来源子图内，粘贴时应被丢掉。
          {
            from: { node_id: 'node:src-a', handle: 'out' },
            to: { node_id: 'node:outside', handle: 'in' },
          },
        ],
        positions: { 'node:src-a': { x: 10, y: 20 }, 'node:src-b': { x: 10, y: 200 } },
      },
      { x: 32, y: 32 },
    );

    const { nodes, edges } = state().core.definition.flow;
    expect(nodes.map((node) => node.id)).toEqual(['node:new-1', 'node:new-2']);
    expect(edges).toEqual([
      {
        from: { node_id: 'node:new-1', handle: 'out' },
        to: { node_id: 'node:new-2', handle: 'in' },
      },
    ]);

    const projected = selectFlowProjection(state()).nodes;
    expect(projected.map((node) => node.position)).toEqual([
      { x: 42, y: 52 },
      { x: 42, y: 232 },
    ]);
    expect(state().core.selection).toBe('node:new-1');
    expect(state().core.history).toHaveLength(1);

    state().undo();
    expect(state().core.definition.flow.nodes).toHaveLength(0);
    expect(state().core.definition.flow.edges).toHaveLength(0);

    state().redo();
    expect(state().core.definition.flow.nodes.map((node) => node.id)).toEqual([
      'node:new-1',
      'node:new-2',
    ]);
    expect(selectFlowProjection(state()).nodes.map((node) => node.position)).toEqual([
      { x: 42, y: 52 },
      { x: 42, y: 232 },
    ]);
  });
});
