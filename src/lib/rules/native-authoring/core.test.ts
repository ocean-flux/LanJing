//! 原生规则作者核心 reducer 行为测试。
//!
//! 覆盖合同列出的不变量：undo/redo、dirty 域分离、epoch stale rejection、
//! edit-during-save 分域 merge、candidate 失效、credential 不进 history、
//! layout coalesce、transient 状态不入历史。

import { describe, expect, it } from 'vitest';
import {
  createBlankDefinition,
  createInitialState,
  reduce,
  type AuthoringAction,
  type NativeRuleAuthoringState,
} from './core';
import type { FlowEdge, RuleDefinition } from './wire';

/** 单节点定义 fixture。 */
function makeDefinition(overrides: Partial<RuleDefinition> = {}): RuleDefinition {
  const definition = createBlankDefinition('source:test');
  definition.flow = {
    nodes: [
      {
        id: 'node:http',
        config: { kind: 'http', value: { url_template: 'https://example.test' } },
      },
      {
        id: 'node:mapper',
        config: { kind: 'mapper', value: { fields: [] } },
      },
    ],
    edges: [],
  };
  return { ...definition, ...overrides };
}

function initialState(
  options: {
    definition?: RuleDefinition;
    semanticRevision?: number | null;
    layoutRevision?: number | null;
  } = {},
): NativeRuleAuthoringState {
  return createInitialState({
    definition: options.definition ?? makeDefinition(),
    savedSemanticRevision: options.semanticRevision ?? 1,
    savedLayoutRevision: options.layoutRevision ?? 1,
  });
}

function run(
  state: NativeRuleAuthoringState,
  ...actions: AuthoringAction[]
): NativeRuleAuthoringState {
  return actions.reduce(reduce, state);
}

const edge: FlowEdge = {
  from: { node_id: 'node:http', handle: 'out' },
  to: { node_id: 'node:mapper', handle: 'in' },
};

describe('NativeRuleAuthoringCore undo/redo', () => {
  it('undo 回退语义命令，redo 重放', () => {
    let state = initialState();
    state = run(state, { kind: 'setField', field: 'base_url', value: 'https://a.test' });
    state = run(state, { kind: 'setField', field: 'base_url', value: 'https://b.test' });

    expect(state.definition.base_url).toBe('https://b.test');

    state = reduce(state, { kind: 'undo' });
    expect(state.definition.base_url).toBe('https://a.test');
    expect(state.history).toHaveLength(1);

    state = reduce(state, { kind: 'undo' });
    expect(state.definition.base_url).toBe('');
    expect(state.history).toHaveLength(0);

    state = reduce(state, { kind: 'redo' });
    expect(state.definition.base_url).toBe('https://a.test');

    state = reduce(state, { kind: 'redo' });
    expect(state.definition.base_url).toBe('https://b.test');
    expect(state.redo).toHaveLength(0);
  });

  it('undo/redo 只回放 semantic/layout 命令；selection/intentFocus 不入历史', () => {
    let state = initialState();
    state = run(
      state,
      { kind: 'selection', nodeId: 'node:http' },
      { kind: 'intentFocus', intent: 'Search' },
      { kind: 'setField', field: 'base_url', value: 'https://c.test' },
    );

    expect(state.history).toHaveLength(1);
    expect(state.history[0]?.domain).toBe('semantic');
    expect(state.selection).toBe('node:http');
    expect(state.intentFocus).toBe('Search');

    // undo 不影响 transient 状态
    state = reduce(state, { kind: 'undo' });
    expect(state.selection).toBe('node:http');
    expect(state.intentFocus).toBe('Search');
  });

  it('undo 后新编辑清空 redo 栈', () => {
    let state = initialState();
    state = run(
      state,
      { kind: 'setField', field: 'base_url', value: 'https://a.test' },
      { kind: 'setField', field: 'base_url', value: 'https://b.test' },
    );
    state = reduce(state, { kind: 'undo' });
    expect(state.redo).toHaveLength(1);

    state = run(state, { kind: 'setField', field: 'base_url', value: 'https://c.test' });
    expect(state.redo).toHaveLength(0);
  });
});

describe('NativeRuleAuthoringCore dirty 域分离', () => {
  it('语义编辑只置 semantic dirty；布局编辑只置 layout dirty', () => {
    let state = initialState();
    state = run(state, { kind: 'setField', field: 'base_url', value: 'https://a.test' });
    expect(state.dirty).toEqual({ semantic: true, layout: false });

    state = run(state, { kind: 'moveNode', nodeId: 'node:http', position: { x: 10, y: 20 } });
    expect(state.dirty).toEqual({ semantic: true, layout: true });

    // 无实际变化的 action 不改 dirty
    state = run(state, { kind: 'moveNode', nodeId: 'node:http', position: { x: 10, y: 20 } });
    expect(state.dirty).toEqual({ semantic: true, layout: true });
  });

  it('布局折叠同样只置 layout dirty', () => {
    const state = run(initialState(), {
      kind: 'collapseNode',
      nodeId: 'node:http',
      collapsed: true,
    });
    expect(state.dirty).toEqual({ semantic: false, layout: true });
  });
});

describe('NativeRuleAuthoringCore epoch stale rejection', () => {
  it('旧 epoch 响应被拒绝，最新响应被接受', () => {
    let state = initialState();
    state = run(state, { kind: 'setField', field: 'base_url', value: 'https://a.test' });

    // 第一次保存
    state = reduce(state, { kind: 'saveRequest' });
    expect(state.epoch).toBe(1);
    // 保存期间继续编辑并再次保存（覆盖在途快照）
    state = run(state, { kind: 'setField', field: 'base_url', value: 'https://b.test' });
    state = reduce(state, { kind: 'saveRequest' });
    expect(state.epoch).toBe(2);

    // 第一个请求的响应（epoch 1）已过期 → 拒绝，revision/dirty 不动
    const staleResponse = reduce(state, {
      kind: 'saveResponse',
      epoch: 1,
      outcome: { document_id: 'doc:1', semantic: { revision: 2 } },
    });
    expect(staleResponse.savedSemanticRevision).toBe(1);
    expect(staleResponse.dirty.semantic).toBe(true);
    expect(staleResponse.epoch).toBe(2);

    // 最新响应（epoch 2）被接受
    const accepted = reduce(staleResponse, {
      kind: 'saveResponse',
      epoch: 2,
      outcome: { document_id: 'doc:1', semantic: { revision: 3 } },
    });
    expect(accepted.savedSemanticRevision).toBe(3);
    expect(accepted.epoch).toBe(3);
    expect(accepted.dirty.semantic).toBe(false);
  });

  it('同一响应重复投递被拒绝（epoch 已递增）', () => {
    let state = initialState();
    state = run(state, { kind: 'setField', field: 'base_url', value: 'https://a.test' });
    state = reduce(state, { kind: 'saveRequest' });

    const outcome = { document_id: 'doc:1', semantic: { revision: 2 } };
    const first = reduce(state, { kind: 'saveResponse', epoch: 1, outcome });
    expect(first.savedSemanticRevision).toBe(2);

    const duplicate = reduce(first, { kind: 'saveResponse', epoch: 1, outcome });
    expect(duplicate.savedSemanticRevision).toBe(2);
    expect(duplicate.epoch).toBe(first.epoch);
  });
});

describe('NativeRuleAuthoringCore edit-during-save merge', () => {
  it('未编辑的域收敛为已保存；编辑中的域保持 dirty', () => {
    let state = initialState();
    // 两个域都脏
    state = run(
      state,
      { kind: 'setField', field: 'base_url', value: 'https://a.test' },
      { kind: 'moveNode', nodeId: 'node:http', position: { x: 5, y: 5 } },
    );
    state = reduce(state, { kind: 'saveRequest' });
    expect(state.inFlightSave?.semantic).not.toBeNull();
    expect(state.inFlightSave?.layout).not.toBeNull();

    // 保存期间只编辑语义域
    state = run(state, { kind: 'setField', field: 'base_url', value: 'https://b.test' });

    const merged = reduce(state, {
      kind: 'saveResponse',
      epoch: 1,
      outcome: {
        document_id: 'doc:1',
        semantic: { revision: 2 },
        layout: { revision: 2 },
      },
    });
    // 语义域有编辑后快照 → 保持 dirty
    expect(merged.savedSemanticRevision).toBe(2);
    expect(merged.dirty.semantic).toBe(true);
    // 布局域无编辑 → 收敛
    expect(merged.savedLayoutRevision).toBe(2);
    expect(merged.dirty.layout).toBe(false);
    expect(merged.conflict).toEqual({ semantic: null, layout: null });
  });

  it('未编辑保存后两域都收敛', () => {
    let state = initialState();
    state = run(
      state,
      { kind: 'setField', field: 'base_url', value: 'https://a.test' },
      { kind: 'collapseNode', nodeId: 'node:mapper', collapsed: true },
    );
    state = reduce(state, { kind: 'saveRequest' });

    const merged = reduce(state, {
      kind: 'saveResponse',
      epoch: 1,
      outcome: {
        document_id: 'doc:1',
        semantic: { revision: 2 },
        layout: { revision: 2 },
      },
    });
    expect(merged.dirty).toEqual({ semantic: false, layout: false });
    expect(merged.savedSemanticRevision).toBe(2);
    expect(merged.savedLayoutRevision).toBe(2);
  });

  it('冲突分域记录且 pending credential 保留', () => {
    let state = initialState();
    state = run(state, { kind: 'setField', field: 'base_url', value: 'https://a.test' });
    state = reduce(state, { kind: 'saveRequest' });

    const conflicted = reduce(state, {
      kind: 'saveResponse',
      epoch: 1,
      outcome: {
        document_id: 'doc:1',
        semantic: { revision: 1, conflict: { expected: 1, current: 5 } },
      },
    });
    expect(conflicted.conflict.semantic).toEqual({ expected: 1, current: 5 });
    expect(conflicted.savedSemanticRevision).toBe(1);
    expect(conflicted.dirty.semantic).toBe(true);
  });
});

describe('NativeRuleAuthoringCore candidate invalidation', () => {
  const candidate = { id: 'candidate:1', expires_at_ms: 4_000_000_000_000 };

  it('semantic action 清 candidate；layout action 不清', () => {
    let state: NativeRuleAuthoringState = { ...initialState(), candidate };
    state = run(state, { kind: 'setField', field: 'base_url', value: 'https://a.test' });
    expect(state.candidate).toBeNull();

    state = { ...initialState(), candidate };
    state = run(state, { kind: 'edgeConnect', edge, connected: true });
    expect(state.candidate).toBeNull();

    state = { ...initialState(), candidate };
    state = run(state, { kind: 'moveNode', nodeId: 'node:http', position: { x: 3, y: 3 } });
    expect(state.candidate).toEqual(candidate);
  });

  it('语义保存成功后 candidate 失效', () => {
    let state: NativeRuleAuthoringState = { ...initialState(), candidate };
    state = run(state, { kind: 'setField', field: 'base_url', value: 'https://a.test' });
    state = run(state, { kind: 'saveRequest' });
    const saved = reduce(state, {
      kind: 'saveResponse',
      epoch: 1,
      outcome: { document_id: 'doc:1', semantic: { revision: 2 } },
    });
    expect(saved.candidate).toBeNull();
  });
});

describe('NativeRuleAuthoringCore credential 不变量', () => {
  it('credentialReplace 携带一次性明文；值进 pending 但不进 history，saveRequest 后清空', () => {
    let state = initialState();
    state = run(
      state,
      { kind: 'setField', field: 'base_url', value: 'https://a.test' },
      {
        kind: 'credentialReplace',
        nodeId: 'node:http',
        jsonPointer: '/headers/authorization',
        logicalName: 'auth',
        value: 'sk-secret-raw-value',
      },
    );

    // 历史只有 setField（语义命令）；credential 命令不产生历史条目
    expect(state.history).toHaveLength(1);
    expect(state.history[0]?.kind).toBe('setField');

    // pending 携带一次性明文（与 Rust CredentialMutationRequest.value 一致）
    expect(state.pendingCredentialMutations).toEqual([
      {
        node_id: 'node:http',
        json_pointer: '/headers/authorization',
        logical_name: 'auth',
        action: 'replace',
        value: 'sk-secret-raw-value',
      },
    ]);

    // 明文不进历史/redo/snapshot；序列化后 history 内无 secret
    const serialized = JSON.stringify({ history: state.history, redo: state.redo });
    expect(serialized).not.toContain('sk-secret-raw-value');

    // saveRequest 快照后立即清空 pending（明文一次性，不跨 save 驻留）
    const afterSave = reduce(state, { kind: 'saveRequest' });
    expect(afterSave.pendingCredentialMutations).toEqual([]);
    expect(afterSave.inFlightSave?.semantic?.credentialMutations).toEqual([
      {
        node_id: 'node:http',
        json_pointer: '/headers/authorization',
        logical_name: 'auth',
        action: 'replace',
        value: 'sk-secret-raw-value',
      },
    ]);
  });

  it('credential 操作置 semantic dirty 并清 candidate；clear 不带 value', () => {
    const candidate = { id: 'candidate:1', expires_at_ms: 4_000_000_000_000 };
    let state: NativeRuleAuthoringState = { ...initialState(), candidate };
    state = run(state, {
      kind: 'credentialClear',
      nodeId: 'node:http',
      jsonPointer: '/headers/authorization',
      logicalName: 'auth',
    });

    expect(state.dirty.semantic).toBe(true);
    expect(state.candidate).toBeNull();
    expect(state.pendingCredentialMutations[0]).toEqual({
      node_id: 'node:http',
      json_pointer: '/headers/authorization',
      logical_name: 'auth',
      action: 'clear',
    });
    expect('value' in (state.pendingCredentialMutations[0] ?? {})).toBe(false);
  });

  it('saveRequest 清空已入队变更；响应期间追加的保留并保持 dirty', () => {
    let state = initialState();
    state = run(state, {
      kind: 'credentialReplace',
      nodeId: 'node:http',
      jsonPointer: '/headers/authorization',
      logicalName: 'auth',
      value: 'sk-secret-1',
    });
    state = reduce(state, { kind: 'saveRequest' });
    expect(state.pendingCredentialMutations).toEqual([]);

    // 保存期间追加第二条变更
    state = run(state, {
      kind: 'credentialReplace',
      nodeId: 'node:http',
      jsonPointer: '/headers/x-api-key',
      logicalName: 'api-key',
      value: 'sk-secret-2',
    });

    const saved = reduce(state, {
      kind: 'saveResponse',
      epoch: 1,
      outcome: { document_id: 'doc:1', semantic: { revision: 2 } },
    });
    // 第一条已随 save 发送并清空；第二条是响应期间新输入 → 保留 → semantic 保持 dirty
    expect(saved.pendingCredentialMutations).toEqual([
      {
        node_id: 'node:http',
        json_pointer: '/headers/x-api-key',
        logical_name: 'api-key',
        action: 'replace',
        value: 'sk-secret-2',
      },
    ]);
    expect(saved.dirty.semantic).toBe(true);
  });
});

describe('NativeRuleAuthoringCore layout coalescing', () => {
  it('连续 moveNode 合并为一条历史；undo 回到拖动前位置', () => {
    let state = initialState();
    state = run(
      state,
      { kind: 'moveNode', nodeId: 'node:http', position: { x: 1, y: 1 } },
      { kind: 'moveNode', nodeId: 'node:http', position: { x: 2, y: 2 } },
      { kind: 'moveNode', nodeId: 'node:http', position: { x: 3, y: 3 } },
    );

    const layout = state.layout as {
      nodes: Record<string, { position: { x: number; y: number } }>;
    };
    expect(layout.nodes['node:http']?.position).toEqual({ x: 3, y: 3 });
    expect(state.history).toHaveLength(1);
    expect(state.history[0]?.kind).toBe('moveNode');

    state = reduce(state, { kind: 'undo' });
    const reverted = state.layout as {
      nodes: Record<string, { position: { x: number; y: number } }>;
    };
    expect(reverted.nodes['node:http']?.position).toEqual({ x: 0, y: 0 });
  });

  it('不同节点移动不合并', () => {
    let state = initialState();
    state = run(
      state,
      { kind: 'moveNode', nodeId: 'node:http', position: { x: 1, y: 1 } },
      { kind: 'moveNode', nodeId: 'node:mapper', position: { x: 2, y: 2 } },
    );
    expect(state.history).toHaveLength(2);
  });

  it('连续 collapse 合并；undo 恢复原折叠状态', () => {
    let state = initialState();
    state = run(
      state,
      { kind: 'collapseNode', nodeId: 'node:http', collapsed: true },
      { kind: 'collapseNode', nodeId: 'node:http', collapsed: false },
    );
    expect(state.history).toHaveLength(1);

    state = reduce(state, { kind: 'undo' });
    const layout = state.layout as { nodes: Record<string, { collapsed: boolean }> };
    expect(layout.nodes['node:http']?.collapsed).toBe(false);
  });
});

describe('NativeRuleAuthoringCore transient 与 reset', () => {
  it('viewport 只改 transient，core state 不变', () => {
    const state = initialState();
    const next = reduce(state, { kind: 'viewport', viewport: { scale: 1.5 } });
    expect(next).toBe(state);
    expect(next.history).toHaveLength(0);
  });

  it('setDiagnostics 更新诊断但不入历史', () => {
    const state = run(initialState(), {
      kind: 'setDiagnostics',
      diagnostics: [{ code: 'missing_intent', severity: 'warning', message: '缺少意图导出' }],
    });
    expect(state.diagnostics).toHaveLength(1);
    expect(state.history).toHaveLength(0);
  });

  it('reset 整体替换 state', () => {
    const original = initialState();
    const fresh = initialState({ semanticRevision: 7 });
    const reset = reduce(original, { kind: 'reset', state: fresh });
    expect(reset.savedSemanticRevision).toBe(7);
    expect(reset).not.toBe(original);
  });

  it('no-op 语义命令不产生历史', () => {
    const state = initialState();
    const next = reduce(state, {
      kind: 'setField',
      field: 'base_url',
      value: state.definition.base_url,
    });
    expect(next).toBe(state);
  });
});

describe('NativeRuleAuthoringCore 语义命令领域', () => {
  it('history 条目全部标记 domain 且 undo/redo 只处理 semantic/layout', () => {
    let state = initialState();
    state = run(
      state,
      { kind: 'setField', field: 'base_url', value: 'https://a.test' },
      { kind: 'edgeConnect', edge, connected: true },
      {
        kind: 'setIntentExport',
        intent: 'Search',
        value: { flow_entry: 'node:http', mapper_output: 'node:mapper' },
      },
      { kind: 'setNodeConfig', nodeId: 'node:http', patch: { url_template: 'https://x.test' } },
      { kind: 'moveNode', nodeId: 'node:http', position: { x: 9, y: 9 } },
    );

    expect(state.history).toHaveLength(5);
    for (const command of state.history) {
      expect(['semantic', 'layout']).toContain(command.domain);
    }
    const semantic = state.history.filter((command) => command.domain === 'semantic');
    expect(semantic).toHaveLength(4);

    // undo 到底：语义与布局都被回退，定义回到初始
    while (state.history.length > 0) {
      state = reduce(state, { kind: 'undo' });
    }
    expect(state.definition.base_url).toBe('');
    expect(state.definition.flow.edges).toHaveLength(0);
    expect(state.definition.intent_exports.Search).toBeUndefined();
    expect(state.history).toHaveLength(0);
    expect(state.redo).toHaveLength(5);

    // 再全部 redo
    while (state.redo.length > 0) {
      state = reduce(state, { kind: 'redo' });
    }
    expect(state.definition.base_url).toBe('https://a.test');
    expect(state.definition.flow.edges).toHaveLength(1);
  });
});

describe('NativeRuleAuthoringCore saveRequest 快照', () => {
  it('无 dirty 域时 saveRequest 为 no-op', () => {
    const state = initialState();
    const next = reduce(state, { kind: 'saveRequest' });
    expect(next).toBe(state);
  });

  it('inFlightSave 只包含 dirty 域；保存期间仍可编辑', () => {
    let state = initialState();
    state = run(state, { kind: 'setField', field: 'base_url', value: 'https://a.test' });
    state = reduce(state, { kind: 'saveRequest' });

    expect(state.inFlightSave?.semantic).not.toBeNull();
    expect(state.inFlightSave?.layout).toBeNull();
    expect(state.inFlightSave?.semantic?.revision).toBe(1);

    // 保存期间继续编辑不被阻塞
    state = run(state, { kind: 'setField', field: 'base_url', value: 'https://b.test' });
    expect(state.definition.base_url).toBe('https://b.test');
  });
});
