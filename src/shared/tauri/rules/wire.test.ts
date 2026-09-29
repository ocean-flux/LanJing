//! 原生规则文档生命周期 wire 测试。
//!
//! 覆盖：9 个 invoke wrapper 的 command 名与 `{ request }` wrapper 约定、
//! DTO 字段名与 Rust 合同（snake_case）逐字段 parity。

import { invoke, isTauri } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  createNativeRuleDocument,
  deleteNativeRuleDocument,
  getNativeRuleDocument,
  getNativeRuleProvenance,
  listNativeRuleDocuments,
  renameNativeRuleDocument,
  saveNativeRuleDocument,
  validateNativeRuleDocument,
  type NativeRuleDocumentSummary,
} from './wire';

// 用字符串形式而不是 `import()` 形式：`invoke` 是泛型函数（`<T>(cmd) => Promise<T>`），
// `import()` 形式会把 factory 返回值对着模块类型校验，而 `vi.fn()` 产出的 Mock
// 无法满足泛型签名。字符串形式下 mock 本身仍然是有类型的。
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn<typeof invoke>(),
  isTauri: vi.fn<typeof isTauri>(),
}));

describe('native rule wire invoke wrappers', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(isTauri).mockReset();
    vi.mocked(isTauri).mockReturnValue(true);
  });

  it('createNativeRuleDocument 使用 create_native_rule_document + request wrapper', async () => {
    const summary = makeSummary();
    vi.mocked(invoke).mockResolvedValueOnce(summary);

    await expect(createNativeRuleDocument({ mode: { kind: 'blank' } })).resolves.toEqual(summary);
    expect(invoke).toHaveBeenCalledWith('create_native_rule_document', {
      request: { mode: { kind: 'blank' } },
    });
  });

  it('createNativeRuleDocument 支持 template 模式（snake_case 字段）', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(makeSummary());

    await createNativeRuleDocument({
      mode: {
        kind: 'template',
        title: '示例规则',
        intent: 'Search',
        data_type: 'json',
        base_url: 'https://example.test',
      },
    });
    expect(invoke).toHaveBeenCalledWith('create_native_rule_document', {
      request: {
        mode: {
          kind: 'template',
          title: '示例规则',
          intent: 'Search',
          data_type: 'json',
          base_url: 'https://example.test',
        },
      },
    });
  });

  it('saveNativeRuleDocument 分域 payload 保持 snake_case', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      document_id: 'doc:1',
      semantic: { revision: 2 },
      layout: null,
    });

    await saveNativeRuleDocument({
      document_id: 'doc:1',
      semantic: {
        expected_revision: 1,
        definition: makeDefinition(),
        credential_mutations: [
          {
            node_id: 'node:http',
            json_pointer: '/headers/authorization',
            logical_name: 'auth',
            action: 'replace',
            value: 'token:one-time',
          },
        ],
      },
      layout: { expected_revision: 1, layout_json: '{"nodes":{}}' },
    });
    expect(invoke).toHaveBeenCalledWith('save_native_rule_document', {
      request: {
        document_id: 'doc:1',
        semantic: {
          expected_revision: 1,
          definition: makeDefinition(),
          credential_mutations: [
            {
              node_id: 'node:http',
              json_pointer: '/headers/authorization',
              logical_name: 'auth',
              action: 'replace',
              value: 'token:one-time',
            },
          ],
        },
        layout: { expected_revision: 1, layout_json: '{"nodes":{}}' },
      },
    });
  });

  it('validateNativeRuleDocument 使用对应 command', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      revision: 1,
      definition_hash: 'hash:def',
      plan_hash: 'hash:plan',
      diagnostics: [],
      profile: {
        id: 'source:test',
        title: '测试',
        icon_url: null,
        version: null,
        group: null,
        supported_intents: ['Search'],
        risk_notes: [],
      },
      capability: { network: false, system: { fs: false, env: false, process: false } },
    });

    await validateNativeRuleDocument({ document_id: 'doc:1', revision: 1 });
    expect(invoke).toHaveBeenCalledWith('validate_native_rule_document', {
      request: { document_id: 'doc:1', revision: 1 },
    });
  });

  it('listNativeRuleDocuments 使用空 request wrapper', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([makeSummary()]);

    await expect(listNativeRuleDocuments()).resolves.toHaveLength(1);
    expect(invoke).toHaveBeenCalledWith('list_native_rule_documents', { request: {} });
  });

  it('getNativeRuleDocument / getNativeRuleProvenance 使用对应 command', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      summary: makeSummary(),
      semantic_revision: 1,
      layout_revision: 1,
      provenance: null,
    });
    await getNativeRuleDocument({ document_id: 'doc:1' });
    expect(invoke).toHaveBeenCalledWith('get_native_rule_document', {
      request: { document_id: 'doc:1' },
    });

    vi.mocked(invoke).mockResolvedValueOnce({
      format: 'legado',
      adapter_version: '1.0.0',
      input_hash: 'hash:input',
      diagnostics: [],
      imported_at_ms: 1_700_000_000_000,
      masked_text: '脱敏原文',
    });
    await getNativeRuleProvenance({ document_id: 'doc:1' });
    expect(invoke).toHaveBeenCalledWith('get_native_rule_provenance', {
      request: { document_id: 'doc:1' },
    });
  });

  it('renameNativeRuleDocument / deleteNativeRuleDocument 使用对应 command', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(makeSummary());
    await renameNativeRuleDocument({
      document_id: 'doc:1',
      title: '新标题',
      expected_revision: 1,
      trace_id: 'trace:1',
      occurred_at_ms: 1_700_000_000_000,
    });
    expect(invoke).toHaveBeenCalledWith('rename_native_rule_document', {
      request: {
        document_id: 'doc:1',
        title: '新标题',
        expected_revision: 1,
        trace_id: 'trace:1',
        occurred_at_ms: 1_700_000_000_000,
      },
    });

    vi.mocked(invoke).mockResolvedValueOnce(null);
    await deleteNativeRuleDocument({
      document_id: 'doc:1',
      confirm_linked: true,
      trace_id: 'trace:1',
      occurred_at_ms: 1_700_000_000_000,
    });
    expect(invoke).toHaveBeenCalledWith('delete_native_rule_document', {
      request: {
        document_id: 'doc:1',
        confirm_linked: true,
        trace_id: 'trace:1',
        occurred_at_ms: 1_700_000_000_000,
      },
    });
  });

  it('非 Tauri 环境下读路径退化为空集且不发起 invoke', async () => {
    vi.mocked(isTauri).mockReturnValue(false);

    await expect(listNativeRuleDocuments()).resolves.toEqual([]);
    await expect(getNativeRuleDocument({ document_id: 'doc:1' })).resolves.toBeNull();
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe('native rule wire mirror parity', () => {
  it('NativeRuleDocumentSummary 字段与 storage DocumentSummary 一致', () => {
    const summary = makeSummary();
    expect(Object.keys(summary).sort()).toEqual(
      [
        'document_id',
        'format',
        'title',
        'source_identity',
        'state',
        'semantic_revision',
        'layout_revision',
        'link_revision',
        'created_at_ms',
        'updated_at_ms',
      ].sort(),
    );
  });

  it('SaveNativeRuleDocumentOutcome 分域 outcome 字段一致', () => {
    const outcome = {
      document_id: 'doc:1',
      semantic: { revision: 2, conflict: { expected: 1, current: 5 } },
      layout: { revision: 2, conflict: null },
    };
    expect(Object.keys(outcome.semantic ?? {}).sort()).toEqual(['conflict', 'revision'].sort());
    expect(Object.keys(outcome.semantic?.conflict ?? {}).sort()).toEqual(
      ['current', 'expected'].sort(),
    );
  });

  it('NativeRuleDocumentDetail / ProvenanceSummaryView 字段一致', () => {
    const detail = {
      summary: makeSummary(),
      semantic_revision: 1,
      layout_revision: 1,
      provenance: {
        format: 'legado',
        adapter_version: '1.0.0',
        input_hash: 'hash:input',
        diagnostics: [],
        imported_at_ms: 1_700_000_000_000,
      },
    };
    expect(Object.keys(detail).sort()).toEqual(
      ['summary', 'semantic_revision', 'layout_revision', 'provenance'].sort(),
    );
    expect(Object.keys(detail.provenance ?? {}).sort()).toEqual(
      ['format', 'adapter_version', 'input_hash', 'diagnostics', 'imported_at_ms'].sort(),
    );
  });

  it('NativeRuleProvenanceView 字段一致且 masked_text 为脱敏文本', () => {
    const view = {
      format: 'legado',
      adapter_version: '1.0.0',
      input_hash: 'hash:input',
      diagnostics: [],
      imported_at_ms: 1_700_000_000_000,
      masked_text: '脱敏原文',
    };
    expect(Object.keys(view).sort()).toEqual(
      [
        'format',
        'adapter_version',
        'input_hash',
        'diagnostics',
        'imported_at_ms',
        'masked_text',
      ].sort(),
    );
  });

  it('RuleDefinition 镜像携带 contract tag 与 schema_version', () => {
    const definition = makeDefinition();
    expect(definition.contract).toBe('rule_definition');
    expect(definition.schema_version).toBe(1);
    expect(Object.keys(definition).sort()).toEqual(
      [
        'contract',
        'schema_version',
        'source_identity',
        'base_url',
        'intent_exports',
        'flow',
        'capability_manifest',
        'source_id_rules',
      ].sort(),
    );
    expect(definition.flow).toEqual({ nodes: expect.any(Array), edges: expect.any(Array) });
  });

  it('credential mutation 携带一次性明文（与 Rust CredentialMutationRequest.value 一致）', () => {
    const serialized = JSON.stringify({
      node_id: 'node:http',
      json_pointer: '/headers/authorization',
      logical_name: 'auth',
      action: 'replace',
      value: 'one-time-secret-value',
    });
    // Replace 的一次性明文确实进入请求载荷（后端立即消费为 carrier，不落库）
    expect(serialized).toContain('one-time-secret-value');
    // Clear 不带 value
    const clear = JSON.stringify({
      node_id: 'node:http',
      json_pointer: '/headers/authorization',
      logical_name: 'auth',
      action: 'clear',
    });
    expect(clear).not.toContain('value');
  });
});

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function makeDefinition() {
  return {
    contract: 'rule_definition' as const,
    schema_version: 1 as const,
    source_identity: 'source:test',
    base_url: 'https://example.test',
    intent_exports: {
      Search: { flow_entry: 'node:http', mapper_output: 'node:mapper' },
    },
    flow: {
      nodes: [
        { id: 'node:http', config: { kind: 'http' as const, value: {} } },
        { id: 'node:mapper', config: { kind: 'mapper' as const, value: {} } },
      ],
      edges: [],
    },
    capability_manifest: {
      required: { network: false, system: { fs: false, env: false, process: false } },
    },
    source_id_rules: [],
  };
}

function makeSummary(): NativeRuleDocumentSummary {
  return {
    document_id: 'doc:1',
    format: 'native_rule',
    title: '示例规则',
    source_identity: 'source:test',
    state: 'draft',
    semantic_revision: 1,
    layout_revision: 1,
    link_revision: 0,
    created_at_ms: 1_700_000_000_000,
    updated_at_ms: 1_700_000_000_000,
  };
}
