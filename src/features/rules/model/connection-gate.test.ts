//! 连接 gate 测试：端口类型 / 意图兼容 / 回边拒绝，palette 推荐与可达性。

import { describe, expect, it } from 'vitest';
import type { FlowEdge } from '@/shared/tauri/rules';
import {
  canConnect,
  reachableNodes,
  recommendKinds,
  semanticEdgeFromConnection,
  validateConnection,
  type FlowConnection,
  type GateGraph,
} from './connection-gate';

/** 构造 gate 图。 */
function graph(
  nodes: GateGraph['nodes'],
  edges: FlowEdge[] = [],
  focusEntry?: string | null,
): GateGraph {
  return { nodes, edges, focusEntry };
}

const httpA = { id: 'http-a', kind: 'http' as const };
const httpB = { id: 'http-b', kind: 'http' as const };
const extractA = { id: 'extract-a', kind: 'extract' as const };
const mapperA = { id: 'mapper-a', kind: 'mapper' as const };
const jsA = { id: 'js-a', kind: 'js' as const };
const loopA = { id: 'loop-a', kind: 'loop' as const };

/** 语义边快捷构造。 */
function edge(from: string, fromHandle: string, to: string, toHandle: string): FlowEdge {
  return { from: { node_id: from, handle: fromHandle }, to: { node_id: to, handle: toHandle } };
}

/** 连接快捷构造。 */
function conn(
  source: string,
  sourceHandle: string,
  target: string,
  targetHandle: string,
): FlowConnection {
  return { source, target, sourceHandle, targetHandle };
}

describe('validateConnection：端口类型', () => {
  it('接受 http_response → extract.source 的合法连接', () => {
    const result = validateConnection(
      conn('http-a', 'http_response', 'extract-a', 'source'),
      graph([httpA, extractA]),
    );
    expect(result).toEqual({ ok: true });
  });

  it('拒绝端口类型不兼容（http_response → mapper.in）', () => {
    const result = validateConnection(
      conn('http-a', 'http_response', 'mapper-a', 'in'),
      graph([httpA, mapperA]),
    );
    expect(result).toEqual({ ok: false, reason: 'incompatible-ports' });
  });

  it('拒绝自环（source === target）', () => {
    const result = validateConnection(conn('js-a', 'json', 'js-a', 'in'), graph([jsA]));
    expect(result).toEqual({ ok: false, reason: 'self-loop' });
  });

  it('拒绝未知节点 / 未知 handle / 缺失 handle', () => {
    expect(validateConnection(conn('nope', 'json', 'js-a', 'in'), graph([jsA]))).toEqual({
      ok: false,
      reason: 'unknown-node',
    });
    expect(
      validateConnection(conn('js-a', 'wat', 'extract-a', 'source'), graph([jsA, extractA])),
    ).toEqual({
      ok: false,
      reason: 'unknown-handle',
    });
    expect(
      validateConnection(conn('js-a', '', 'extract-a', 'source'), graph([jsA, extractA])),
    ).toEqual({
      ok: false,
      reason: 'missing-handle',
    });
  });

  it('重连排除正在替换的旧边，不把原位置判为 duplicate', () => {
    const existing = edge('http-a', 'http_response', 'extract-a', 'source');
    const result = validateConnection(conn('http-a', 'http_response', 'extract-a', 'source'), {
      ...graph([httpA, extractA], [existing]),
      excludeEdgeId: 'http-a:http_response->extract-a:source',
    });
    expect(result).toEqual({ ok: true });
  });
});

describe('validateConnection：规则入口', () => {
  it('HTTP 入口不是可连接的输入端口', () => {
    expect(
      validateConnection(conn('http-a', 'http_response', 'http-b', 'in'), graph([httpA, httpB])),
    ).toEqual({ ok: false, reason: 'unknown-handle' });
  });
});

describe('validateConnection：回边拒绝', () => {
  it('拒绝直接回环（A→B 后再连 B→A）', () => {
    const conditionA = { id: 'condition-a', kind: 'condition' as const };
    const existing = edge('condition-a', 'branch:0', 'js-a', 'in');
    const result = validateConnection(
      conn('js-a', 'json', 'condition-a', 'in'),
      graph([conditionA, jsA], [existing]),
    );
    expect(result).toEqual({ ok: false, reason: 'back-edge' });
  });

  it('拒绝间接回环（A→B→C 后再连 C→A）', () => {
    const conditionA = { id: 'condition-a', kind: 'condition' as const };
    const conditionB = { id: 'condition-b', kind: 'condition' as const };
    const edges = [
      edge('condition-a', 'branch:0', 'js-a', 'in'),
      edge('js-a', 'json', 'condition-b', 'in'),
    ];
    const result = validateConnection(
      conn('condition-b', 'branch:0', 'condition-a', 'in'),
      graph([conditionA, conditionB, jsA], edges),
    );
    expect(result).toEqual({ ok: false, reason: 'back-edge' });
  });

  it('允许无环的链路延伸（A→B→C 后再连 B→D）', () => {
    const edges = [
      edge('http-a', 'http_response', 'extract-a', 'source'),
      edge('extract-a', 'json', 'mapper-a', 'in'),
    ];
    const result = validateConnection(
      conn('extract-a', 'json', 'js-a', 'in'),
      graph([httpA, extractA, mapperA, jsA], edges),
    );
    expect(result).toEqual({ ok: true });
  });

  it('允许 Loop body 内节点唯一回接 yield', () => {
    const existing = edge('loop-a', 'body', 'js-a', 'in');
    const result = validateConnection(
      conn('js-a', 'json', 'loop-a', 'yield'),
      graph([loopA, jsA], [existing]),
    );
    expect(result).toEqual({ ok: true });
  });

  it('拒绝 Loop 第二条 yield 回边', () => {
    const jsB = { id: 'js-b', kind: 'js' as const };
    const existing = [
      edge('loop-a', 'body', 'js-a', 'in'),
      edge('js-a', 'json', 'loop-a', 'yield'),
    ];
    const result = validateConnection(
      conn('js-b', 'json', 'loop-a', 'yield'),
      graph([loopA, jsA, jsB], existing),
    );
    expect(result).toEqual({ ok: false, reason: 'loop-yield-occupied' });
  });
});

describe('validateConnection：意图兼容', () => {
  it('焦点激活时，源节点必须在焦点子图内（intent-mismatch）', () => {
    // Http-a 为焦点入口：extract-a / mapper-a 可达，但 js-a 不在子图内
    const g = graph(
      [httpA, extractA, mapperA, jsA],
      [
        edge('http-a', 'http_response', 'extract-a', 'source'),
        edge('extract-a', 'json', 'mapper-a', 'in'),
      ],
      'http-a',
    );
    const result = validateConnection(conn('js-a', 'json', 'mapper-a', 'in'), g);
    expect(result).toEqual({ ok: false, reason: 'intent-mismatch' });
  });

  it('焦点子图内的源节点可正常外扩连接', () => {
    const g = graph(
      [httpA, extractA, mapperA, jsA],
      [
        edge('http-a', 'http_response', 'extract-a', 'source'),
        edge('extract-a', 'json', 'mapper-a', 'in'),
      ],
      'http-a',
    );
    expect(validateConnection(conn('extract-a', 'json', 'js-a', 'in'), g)).toEqual({
      ok: true,
    });
  });

  it('焦点为空（全部视图）时不校验意图', () => {
    const g = graph(
      [httpA, extractA, mapperA, jsA],
      [edge('http-a', 'http_response', 'extract-a', 'source')],
      null,
    );
    expect(validateConnection(conn('js-a', 'json', 'extract-a', 'source'), g)).toEqual({
      ok: false,
      reason: 'incompatible-ports',
    });
    // 同一图内无焦点时，js-a → mapper-a 合法
    expect(validateConnection(conn('js-a', 'json', 'mapper-a', 'in'), g)).toEqual({
      ok: true,
    });
  });
});

describe('reachableNodes', () => {
  it('从入口出发 BFS 可达集合（含入口自身）', () => {
    const edges = [
      edge('http-a', 'http_response', 'extract-a', 'source'),
      edge('extract-a', 'json', 'mapper-a', 'in'),
    ];
    expect([...reachableNodes('http-a', edges)].sort()).toEqual([
      'extract-a',
      'http-a',
      'mapper-a',
    ]);
    expect(reachableNodes('mapper-a', edges)).toEqual(new Set(['mapper-a']));
  });

  it('环路图不无限循环', () => {
    const edges = [
      edge('http-a', 'http_response', 'extract-a', 'source'),
      edge('extract-a', 'json', 'http-a', 'in'),
    ];
    expect([...reachableNodes('http-a', edges)].sort()).toEqual(['extract-a', 'http-a']);
  });
});

describe('semanticEdgeFromConnection', () => {
  it('把 Svelte Flow Connection 映射为语义边', () => {
    expect(
      semanticEdgeFromConnection(conn('http-a', 'http_response', 'extract-a', 'source')),
    ).toEqual(edge('http-a', 'http_response', 'extract-a', 'source'));
  });

  it('把 custom node 可视端口还原为 compiler semantic handle', () => {
    expect(
      semanticEdgeFromConnection(
        conn('http-a', 'http_response', 'extract-a', 'source'),
        { id: 'http-a', kind: 'http', config: {} },
        { id: 'extract-a', kind: 'extract', config: {} },
      ),
    ).toEqual(edge('http-a', 'output', 'extract-a', 'input'));
  });
});

describe('recommendKinds：palette 推荐', () => {
  it('无选中节点时全部七类可添加', () => {
    const recs = recommendKinds({ nodes: [], edges: [] });
    expect(recs).toHaveLength(7);
    expect(recs.every((r) => r.compatible)).toBe(true);
    expect(recs.every((r) => r.blockers.length === 0)).toBe(true);
  });

  it('选中 Extract 后：Mapper/Merge/Condition/Loop/Js 兼容，Http 不兼容并给出原因', () => {
    const recs = recommendKinds({
      nodes: [extractA],
      edges: [],
      selection: 'extract-a',
    });
    const byKind = Object.fromEntries(recs.map((r) => [r.kind, r]));
    // Extract.json 可接入 mapper.in 与 merge.in:0
    expect(byKind.mapper.compatible).toBe(true);
    expect(byKind.merge.compatible).toBe(true);
    expect(byKind.condition.compatible).toBe(true);
    expect(byKind.loop.compatible).toBe(true);
    expect(byKind.js.compatible).toBe(true);
    // Json 与 extract.source / http.in 类型不符
    expect(byKind.extract.compatible).toBe(false);
    expect(byKind.http.compatible).toBe(false);
    expect(byKind.http.blockers).toEqual([
      { code: 'incompatible-ports', sourceKind: 'extract', targetKind: 'http' },
    ]);
  });

  it('意图焦点激活时，选中节点不在焦点子图内 → 全部不兼容并解释', () => {
    const recs = recommendKinds({
      nodes: [httpA, extractA, jsA],
      edges: [edge('http-a', 'http_response', 'extract-a', 'source')],
      selection: 'js-a',
      focusEntry: 'http-a',
    });
    const byKind = Object.fromEntries(recs.map((r) => [r.kind, r]));
    expect(byKind.http.compatible).toBe(false);
    expect(byKind.http.blockers).toContainEqual({ code: 'outside-intent-focus' });
  });

  it('选中焦点子图内节点时可外扩推荐', () => {
    const recs = recommendKinds({
      nodes: [httpA, extractA],
      edges: [edge('http-a', 'http_response', 'extract-a', 'source')],
      selection: 'extract-a',
      focusEntry: 'http-a',
    });
    const byKind = Object.fromEntries(recs.map((r) => [r.kind, r]));
    expect(byKind.mapper.compatible).toBe(true);
    expect(byKind.loop.compatible).toBe(true);
  });
});

describe('canConnect', () => {
  it('是 validateConnection 的布尔投影', () => {
    const g = graph([httpA, extractA]);
    expect(canConnect(conn('http-a', 'http_response', 'extract-a', 'source'), g)).toBe(true);
    expect(canConnect(conn('http-a', 'http_response', 'extract-a', 'in'), g)).toBe(false);
  });
});
