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
  RuleErrorWire,
  SaveNativeRuleDocumentOutcome,
  SaveNativeRuleDocumentRequest,
} from '@/shared/tauri/rules';

const invoke = vi.hoisted(() => vi.fn<(command: string, args?: unknown) => Promise<unknown>>());
const listen = vi.hoisted(() => vi.fn<(event: string, handler: unknown) => Promise<() => void>>());

// 读路径带 isTauri 守卫；测试里固定为运行在 Tauri 中。
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => true }));

// 预览执行订阅 execution event 流；这里注入假 listen，事件由测试主动投递。
vi.mock('@tauri-apps/api/event', () => ({ listen }));

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

  it('未安装能力节点：校验出稳定诊断且不产出 Plan，展示与保存仍成功', async () => {
    const payloadText = '{"selector":".entry"}';
    const session = createRuleEditorSession();
    const state = () => session.getState();

    invoke.mockResolvedValueOnce(
      mockDetail({
        definition: definitionWithOpaqueNode(payloadText),
        summary: mockSummary({ semantic_revision: 1, layout_revision: 1 }),
        effective_semantic_revision: 1,
      }),
    );
    await state().loadDocument('doc:1');

    // 展示：未安装能力节点仍在投影里（descriptor 查不到, 但可展示）。
    expect(selectFlowProjection(state()).nodes.map((node) => node.type)).toContain('custom_reader');

    // 校验：后端给出稳定诊断且不产出 Plan（plan_hash 为 null），
    // 诊断面必须原样保留后端 code, 不换一个通用错误。
    invoke.mockClear();
    invoke.mockResolvedValueOnce({
      valid: false,
      revision: 1,
      definition_hash: 'hash:def',
      plan_hash: null,
      diagnostics: [
        {
          code: 'NODE_CAPABILITY_UNAVAILABLE',
          severity: 'error',
          message: '节点引用了未安装的规则能力 custom_reader',
          span: { path: '/flow/nodes/node:custom/config', start: 0, end: 0 },
        },
      ],
      profile: null,
      capability: { network: false, system: { fs: false, env: false, process: false } },
    });

    const preview = await state().validate();
    expect(preview.valid).toBe(false);
    expect(preview.plan_hash).toBeNull();
    expect(state().core.validation.status).toBe('invalid');
    expect(state().core.validation.diagnostics).toEqual([
      expect.objectContaining({
        code: 'NODE_CAPABILITY_UNAVAILABLE',
        severity: 'error',
        message: '节点引用了未安装的规则能力 custom_reader',
      }),
    ]);

    // 保存仍成功, 但只作为 Draft: Effective Rule Revision 不被替换。
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://draft.test' });
    invoke.mockClear();
    invoke.mockResolvedValueOnce(
      mockOutcome({ semantic: { revision: 2, conflict: null, activation: 'draft' }, layout: null }),
    );
    const outcome = await state().save();
    expect(outcome.semantic?.activation).toBe('draft');
    expect(state().effectiveSemanticRevision).toBe(1);
  });

  // -----------------------------------------------------------------------
  // 保存失败：稳定诊断 + 保留 Effective Rule Revision
  // -----------------------------------------------------------------------

  it('保存失败产生稳定诊断，保留 Effective Rule Revision 与草稿', async () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    invoke.mockResolvedValueOnce(
      mockDetail({
        summary: mockSummary({ semantic_revision: 1, layout_revision: 1 }),
        effective_semantic_revision: 1,
      }),
    );
    await state().loadDocument('doc:1');
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://draft.test' });

    invoke.mockClear();
    invoke.mockRejectedValueOnce({
      stage: 'persistence',
      code: 'version_conflict',
      message: '文档已被其他写入者更新',
      trace_id: 'trace:1',
      retryable: true,
      diagnostics: [],
    } satisfies RuleErrorWire);

    // 稳定 code：session 只抛 SessionError('save_failed')，message 保留后端安全摘要。
    await expect(state().save()).rejects.toMatchObject({ code: 'save_failed' });

    // (a) 稳定诊断出现在既有诊断表面（DiagnosticList 的输入）。
    expect(state().core.validation.status).toBe('error');
    expect(state().core.validation.diagnostics).toEqual([
      expect.objectContaining({
        code: 'version_conflict',
        severity: 'error',
        message: '文档已被其他写入者更新',
      }),
    ]);

    // (b) Effective Rule Revision / savedSemanticRevision 不被替换。
    expect(state().effectiveSemanticRevision).toBe(1);
    expect(state().core.savedSemanticRevision).toBe(1);

    // (c) 草稿仍在且仍可编辑。
    expect(state().core.dirty.semantic).toBe(true);
    expect(selectIsSaving(state())).toBe(false);
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://draft-2.test' });
    expect(state().core.definition.base_url).toBe('https://draft-2.test');
  });

  it('保存失败透传后端 compiler 诊断而不丢弃它们', async () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    invoke.mockResolvedValueOnce(mockDetail());
    await state().loadDocument('doc:1');
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://draft.test' });

    invoke.mockClear();
    invoke.mockRejectedValueOnce({
      stage: 'compile',
      code: 'compile_failed',
      message: 'Definition 无法编译为 immutable Plan',
      trace_id: 'trace:2',
      retryable: false,
      diagnostics: [
        {
          code: 'NODE_CAPABILITY_UNAVAILABLE',
          severity: 'error',
          message: '节点引用了未安装的规则能力 custom_reader',
          span: { path: '/flow/nodes/node:custom/config', start: 0, end: 0 },
        },
      ],
    } satisfies RuleErrorWire);

    await expect(state().save()).rejects.toThrow('Definition 无法编译为 immutable Plan');
    expect(state().core.validation.diagnostics).toEqual([
      expect.objectContaining({ code: 'NODE_CAPABILITY_UNAVAILABLE', severity: 'error' }),
    ]);
  });

  it('非 IPC 错误也产生稳定 SAVE_FAILED 诊断', async () => {
    const session = createRuleEditorSession();
    const state = () => session.getState();
    invoke.mockResolvedValueOnce(mockDetail());
    await state().loadDocument('doc:1');
    state().dispatch({ kind: 'setField', field: 'base_url', value: 'https://draft.test' });

    invoke.mockClear();
    invoke.mockRejectedValueOnce(new Error('storage unavailable'));

    await expect(state().save()).rejects.toThrow('storage unavailable');
    expect(state().core.validation.diagnostics).toEqual([
      expect.objectContaining({ code: 'SAVE_FAILED', severity: 'error' }),
    ]);
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

// ---------------------------------------------------------------------------
// 预览执行（startPreviewRun / cancelPreviewRun）
//
// 事件订阅用假 listen：这一层要证明的是「订阅先于启动、按 execution id 折叠、
// 终态收订阅」，不是 Tauri event API 本身。
// ---------------------------------------------------------------------------

/** Execute 尚未发出时的占位 resolver；调用它本来就是测试错误，不静默跳过。 */
const unresolvedExecute = (): void => undefined;

/** 假事件流：记录 handler，返回可断言的退订函数。 */
function mockEventStream() {
  let deliver: unknown = null;
  const unlisten = vi.fn<() => void>();
  listen.mockImplementation((_event, handler) => {
    deliver = handler;
    return Promise.resolve(unlisten);
  });
  return {
    emit: (payload: unknown) => (deliver as (message: { payload: unknown }) => void)({ payload }),
    unlisten,
    subscribed: () => deliver !== null,
  };
}

function previewEvent(sequence: number, kind: unknown, executionId = 'exec:1') {
  return {
    execution_id: executionId,
    sequence,
    trace_id: 'trace:1',
    occurred_at_ms: 1_700_000_000_000,
    kind,
  };
}

/** 一次预览请求；测试只关心这组固定的目标与输入。 */
const PREVIEW_REQUEST = {
  sourceId: 'source:test',
  intent: 'Search',
  input: { type: 'Query', value: '关键词' },
} as const;

describe('createRuleEditorSession 预览执行', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('先订阅再 execute，并把 started/诊断/终态折叠成运行态', async () => {
    const stream = mockEventStream();
    let resolveExecute: (value: { execution_id: string }) => void = unresolvedExecute;
    const executeResponse = new Promise<{ execution_id: string }>((resolve) => {
      resolveExecute = resolve;
    });
    invoke.mockImplementation((command) =>
      command === 'execute'
        ? executeResponse
        : Promise.reject(new Error(`unexpected command ${command}`)),
    );

    const session = createRuleEditorSession();
    const pending = session.getState().startPreviewRun(PREVIEW_REQUEST);

    // Execute 还没返回：订阅已经在，启动阶段的事件先被缓冲。
    expect(stream.subscribed()).toBe(true);
    expect(session.getState().execution.status).toBe('running');
    stream.emit(previewEvent(1, { kind: 'started' }));
    expect(session.getState().execution.traceId).toBeNull();

    resolveExecute({ execution_id: 'exec:1' });
    await pending;

    expect(invoke).toHaveBeenCalledWith('execute', {
      request: {
        source_id: 'source:test',
        intent: 'Search',
        input: { type: 'Query', value: '关键词' },
        mode: { mode: 'live' },
      },
    });
    expect(session.getState().execution.executionId).toBe('exec:1');
    expect(session.getState().execution.traceId).toBe('trace:1');

    stream.emit(
      previewEvent(2, { kind: 'diagnostic', code: 'js_timeout', message: 'JS 执行超时' }),
    );
    stream.emit(previewEvent(3, { kind: 'completed' }));

    const { execution } = session.getState();
    expect(execution.status).toBe('succeeded');
    expect(execution.diagnostics).toEqual([
      { code: 'js_timeout', severity: 'error', message: 'JS 执行超时' },
    ]);
    // 终态后订阅被收掉：事件流不再需要为这次运行服务。
    expect(stream.unlisten).toHaveBeenCalledWith();

    // 其他 execution 的事件不影响本次状态。
    stream.emit(previewEvent(4, { kind: 'failed' }, 'exec:other'));
    expect(session.getState().execution.status).toBe('succeeded');
  });

  it('启动失败进 failed 诊断面，且不向调用方抛错', async () => {
    mockEventStream();
    invoke.mockRejectedValueOnce({
      stage: 'execution',
      code: 'source_not_installed',
      message: '来源未安装',
      trace_id: 'trace:1',
      retryable: false,
      diagnostics: [],
    });

    const session = createRuleEditorSession();
    await expect(session.getState().startPreviewRun(PREVIEW_REQUEST)).resolves.toBeUndefined();

    const { execution } = session.getState();
    expect(execution.status).toBe('failed');
    expect(execution.failureCode).toBe('source_not_installed');
    expect(execution.diagnostics).toEqual([
      { code: 'source_not_installed', severity: 'error', message: '来源未安装' },
    ]);
  });

  it('cancelPreviewRun 只对进行中的运行发 cancel_execution', async () => {
    mockEventStream();
    invoke.mockImplementation((command) => {
      if (command === 'execute') return Promise.resolve({ execution_id: 'exec:1' });
      if (command === 'cancel_execution') return Promise.resolve({ changed: true });
      return Promise.reject(new Error(`unexpected command ${command}`));
    });

    const session = createRuleEditorSession();
    // 没有执行 id 时取消是空操作。
    await session.getState().cancelPreviewRun();
    expect(invoke).not.toHaveBeenCalledWith('cancel_execution', expect.anything());

    await session.getState().startPreviewRun(PREVIEW_REQUEST);
    await session.getState().cancelPreviewRun();

    expect(invoke).toHaveBeenCalledWith('cancel_execution', {
      request: { execution_id: 'exec:1' },
    });
    // 取消是请求：状态要等 runtime 的 cancelled 事件落定，这里仍是 running。
    expect(session.getState().execution.status).toBe('running');
  });
});

// 订阅不上时不能假装运行：没有事件流就没有运行诊断，如实失败。
describe('createRuleEditorSession 预览执行订阅失败', () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockReset();
  });

  it('listen 失败直接进 failed，不发 execute', async () => {
    listen.mockRejectedValue(new Error('event API 不可用'));

    const session = createRuleEditorSession();
    await session.getState().startPreviewRun(PREVIEW_REQUEST);

    const { execution } = session.getState();
    expect(execution.status).toBe('failed');
    expect(execution.failureCode).toBe('EXECUTION_FAILED');
    expect(invoke).not.toHaveBeenCalled();
  });
});
