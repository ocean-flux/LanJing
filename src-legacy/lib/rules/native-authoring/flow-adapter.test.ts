//! flow-adapter 映射正确性测试。
//!
//! 覆盖：七类 kind 映射、edge id 确定性、intentFocus 不复制节点、move→layout action、
//! connect→semantic action、reconnect→双 action、reachableNodes BFS。

import { describe, expect, it } from 'vitest';
import {
  defaultNodePosition,
  edgeId,
  flowToSemantic,
  projectLoopRegion,
  projectLoopRegions,
  parseEditorSelection,
  portSelectionToken,
  reachableNodes,
  semanticToFlow,
  type FlowNodeData,
  type FlowUIChange,
} from './flow-adapter';
import { createBlankDefinition } from './core';
import type { FlowEdge, FlowNodeKind, RuleDefinition, StandardIntent } from './wire';

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

/** 七类节点 type → kind 映射 fixture。 */
function sevenNodeDefinition(overrides?: Partial<RuleDefinition>): RuleDefinition {
  const definition = createBlankDefinition('source:seven');
  const kinds: FlowNodeKind[] = ['http', 'js', 'extract', 'mapper', 'merge', 'condition', 'loop'];
  definition.flow = {
    nodes: kinds.map((kind) => ({
      id: `node:${kind}`,
      config: { kind, value: {} },
    })),
    edges: [],
  };
  return { ...definition, ...overrides };
}

/** 带边 fixture。 */
function connectedDefinition(): RuleDefinition {
  const definition = createBlankDefinition('source:edges');
  definition.flow = {
    nodes: [
      { id: 'node:http', config: { kind: 'http' as const, value: {} } },
      { id: 'node:extract', config: { kind: 'extract' as const, value: {} } },
      { id: 'node:mapper', config: { kind: 'mapper' as const, value: {} } },
    ],
    edges: [
      {
        from: { node_id: 'node:http', handle: 'out' },
        to: { node_id: 'node:extract', handle: 'in' },
      },
      {
        from: { node_id: 'node:extract', handle: 'out' },
        to: { node_id: 'node:mapper', handle: 'in' },
      },
    ],
  };
  definition.intent_exports = {
    Search: { flow_entry: 'node:http', mapper_output: 'node:mapper' },
  };
  return definition;
}

function loopRegionDefinition(): RuleDefinition {
  const definition = createBlankDefinition('source:loop-region');
  definition.flow = {
    nodes: [
      { id: 'collection', config: { kind: 'http', value: {} } },
      { id: 'loop', config: { kind: 'loop', value: {} } },
      { id: 'body-js', config: { kind: 'js', value: { output: 'json' } } },
      { id: 'body-extract', config: { kind: 'extract', value: {} } },
      { id: 'done', config: { kind: 'mapper', value: {} } },
    ],
    edges: [
      {
        from: { node_id: 'collection', handle: 'output' },
        to: { node_id: 'loop', handle: 'collection' },
      },
      {
        from: { node_id: 'loop', handle: 'body' },
        to: { node_id: 'body-js', handle: 'input' },
      },
      {
        from: { node_id: 'body-js', handle: 'output' },
        to: { node_id: 'body-extract', handle: 'input' },
      },
      {
        from: { node_id: 'body-extract', handle: 'output' },
        to: { node_id: 'loop', handle: 'yield' },
      },
      {
        from: { node_id: 'loop', handle: 'done' },
        to: { node_id: 'done', handle: 'input' },
      },
    ],
  };
  return definition;
}

// ---------------------------------------------------------------------------
// Loop region projection
// ---------------------------------------------------------------------------

describe('projectLoopRegion', () => {
  it('projects valid collection/body/yield/done boundaries and body nodes', () => {
    const definition = loopRegionDefinition();
    const region = projectLoopRegion(definition, 'loop');

    expect(region.status).toBe('valid');
    expect(region.bodyEntry).toEqual({ node_id: 'body-js', handle: 'input' });
    expect(region.yieldSource).toEqual({ node_id: 'body-extract', handle: 'output' });
    expect(region.bodyNodes).toEqual(['body-extract', 'body-js']);
    expect(region.collectionEdgeIds).toEqual(['collection:output->loop:collection']);
    expect(region.bodyEdgeIds).toEqual(['loop:body->body-js:input']);
    expect(region.yieldEdgeIds).toEqual(['body-extract:output->loop:yield']);
    expect(region.doneEdgeIds).toEqual(['loop:done->done:input']);
    expect(region.crossRegionEdgeIds).toEqual([]);
    expect(region.boundaryEdgeIds).toHaveLength(4);
    expect(region.diagnostics).toEqual([]);
  });

  it('reports missing body without inventing body entry or nodes', () => {
    const definition = loopRegionDefinition();
    definition.flow.edges = definition.flow.edges.filter(
      (edge) => !(edge.from.node_id === 'loop' && edge.from.handle === 'body'),
    );

    const region = projectLoopRegion(definition, 'loop');

    expect(region.status).toBe('invalid');
    expect(region.bodyEntry).toBeNull();
    expect(region.bodyNodes).toEqual([]);
    expect(region.diagnostics.map((diagnostic) => diagnostic.code)).toEqual(['LOOP_BODY_INVALID']);
  });

  it('reports multiple yield edges and does not choose an arbitrary source', () => {
    const definition = loopRegionDefinition();
    definition.flow.edges.push({
      from: { node_id: 'body-js', handle: 'output' },
      to: { node_id: 'loop', handle: 'yield' },
    });

    const region = projectLoopRegion(definition, 'loop');

    expect(region.status).toBe('invalid');
    expect(region.yieldSource).toBeNull();
    expect(region.yieldEdgeIds).toHaveLength(2);
    expect(region.diagnostics.map((diagnostic) => diagnostic.code)).toEqual(['LOOP_YIELD_INVALID']);
  });

  it('reports body bypass and exposes bypass edge as a cross-region boundary', () => {
    const definition = loopRegionDefinition();
    definition.flow.edges.push({
      from: { node_id: 'body-js', handle: 'output' },
      to: { node_id: 'done', handle: 'input' },
    });

    const region = projectLoopRegion(definition, 'loop');
    const bypassEdgeId = 'body-js:output->done:input';

    expect(region.status).toBe('invalid');
    expect(region.bodyNodes).toEqual(['body-extract', 'body-js']);
    expect(region.crossRegionEdgeIds).toEqual([bypassEdgeId]);
    expect(region.boundaryEdgeIds).toContain(bypassEdgeId);
    expect(region.diagnostics.map((diagnostic) => diagnostic.code)).toEqual([
      'LOOP_BODY_BYPASS',
      'LOOP_CROSS_REGION_EDGE',
    ]);
  });

  it('reports incoming edge crossing into body region', () => {
    const definition = loopRegionDefinition();
    definition.flow.edges.push({
      from: { node_id: 'collection', handle: 'output' },
      to: { node_id: 'body-extract', handle: 'input' },
    });

    const region = projectLoopRegion(definition, 'loop');

    expect(region.status).toBe('invalid');
    expect(region.crossRegionEdgeIds).toEqual(['collection:output->body-extract:input']);
    expect(region.diagnostics.map((diagnostic) => diagnostic.code)).toEqual([
      'LOOP_CROSS_REGION_EDGE',
    ]);
  });

  it('projects loops in stable node-id order and marks overlapping regions', () => {
    const definition = loopRegionDefinition();
    definition.flow.nodes.push({ id: 'loop-2', config: { kind: 'loop', value: {} } });
    definition.flow.edges.push(
      {
        from: { node_id: 'collection', handle: 'output' },
        to: { node_id: 'loop-2', handle: 'collection' },
      },
      {
        from: { node_id: 'loop-2', handle: 'body' },
        to: { node_id: 'body-js', handle: 'input' },
      },
      {
        from: { node_id: 'body-extract', handle: 'output' },
        to: { node_id: 'loop-2', handle: 'yield' },
      },
      {
        from: { node_id: 'loop-2', handle: 'done' },
        to: { node_id: 'done', handle: 'input' },
      },
    );

    const regions = projectLoopRegions(definition);

    expect(regions.map((region) => region.loopNodeId)).toEqual(['loop', 'loop-2']);
    expect(
      regions.every((region) =>
        region.diagnostics.some((diagnostic) => diagnostic.code === 'LOOP_REGION_OVERLAP'),
      ),
    ).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// semanticToFlow
// ---------------------------------------------------------------------------

describe('semanticToFlow', () => {
  it('map all 7 kinds to FlowViewNode type', () => {
    const def = sevenNodeDefinition();
    const { nodes } = semanticToFlow(def, null, null);
    const expected = new Set<string>([
      'http',
      'js',
      'extract',
      'mapper',
      'merge',
      'condition',
      'loop',
    ]);
    expect(nodes.length).toBe(7);
    for (const node of nodes) {
      expect(expected.has(node.type)).toBe(true);
    }
    // 验证 kind → type 同名
    for (const node of nodes) {
      expect(node.type).toBe((node.data as FlowNodeData).kind);
    }
  });

  it('node position falls back to default grid for un-laid-out nodes', () => {
    const def = sevenNodeDefinition();
    const { nodes } = semanticToFlow(def, null, null);
    expect(nodes[0].position).toEqual(defaultNodePosition('http', 0));
    expect(nodes[3].position).toEqual(defaultNodePosition('mapper', 0));
    // 同类第二个节点 row 递增
    const mixedDef = createBlankDefinition('source:multi');
    mixedDef.flow = {
      nodes: [
        { id: 'a', config: { kind: 'http' as const, value: {} } },
        { id: 'b', config: { kind: 'http' as const, value: {} } },
        { id: 'c', config: { kind: 'mapper' as const, value: {} } },
      ],
      edges: [],
    };
    const { nodes: multiNodes } = semanticToFlow(mixedDef, null, null);
    expect(multiNodes[0].position).toEqual(defaultNodePosition('http', 0));
    expect(multiNodes[1].position).toEqual(defaultNodePosition('http', 1));
    expect(multiNodes[2].position).toEqual(defaultNodePosition('mapper', 0));
  });

  it('edge id is deterministic from semantic identity', () => {
    const def = connectedDefinition();
    const { edges } = semanticToFlow(def, null, null);
    expect(edges).toHaveLength(2);
    expect(edges[0].id).toBe('node:http:out->node:extract:in');
    expect(edges[1].id).toBe('node:extract:out->node:mapper:in');

    // 同一 definition 两次调用产生相同 id
    const { edges: edges2 } = semanticToFlow(def, null, null);
    expect(edges[0].id).toBe(edges2[0].id);
  });

  it('maps compiler handles to visible node handles without changing semantic edge data', () => {
    const definition = createBlankDefinition('source:template');
    definition.flow = {
      nodes: [
        { id: 'node:http', config: { kind: 'http', value: {} } },
        { id: 'node:extract', config: { kind: 'extract', value: {} } },
      ],
      edges: [
        {
          from: { node_id: 'node:http', handle: 'output' },
          to: { node_id: 'node:extract', handle: 'input' },
        },
      ],
    };

    const { edges } = semanticToFlow(definition, null, null);
    expect(edges[0]).toMatchObject({ sourceHandle: 'http_response', targetHandle: 'source' });
    expect(edges[0].data.edge).toEqual(definition.flow.edges[0]);
  });

  it('keeps Loop yield as a labeled control backedge', () => {
    const { edges } = semanticToFlow(loopRegionDefinition(), null, null);
    const bodyEdge = edges.find((edge) => edge.id === 'loop:body->body-js:input');
    const yieldEdge = edges.find((edge) => edge.id === 'body-extract:output->loop:yield');

    expect(bodyEdge?.data).toMatchObject({
      role: 'binding',
      label: 'body(item,index)',
      lane: 'auxiliary',
    });
    expect(yieldEdge?.data).toMatchObject({
      role: 'control',
      label: 'yield(value)',
      lane: 'loop',
      route: 'loop-back',
    });
  });

  it('intentFocus does NOT duplicate nodes/edges', () => {
    const def = connectedDefinition();
    const without = semanticToFlow(def, null, null);
    const withFocus = semanticToFlow(def, 'Search' as StandardIntent, null);
    expect(withFocus.nodes.length).toBe(without.nodes.length);
    expect(withFocus.edges.length).toBe(without.edges.length);
    // 所有节点 ID 相同
    expect(withFocus.nodes.map((n) => n.id).sort()).toEqual(without.nodes.map((n) => n.id).sort());
  });

  it('intentFocus sets focused/dimmed flags', () => {
    const def = connectedDefinition();
    const { nodes } = semanticToFlow(def, 'Search' as StandardIntent, null);
    const http = nodes.find((n) => n.id === 'node:http');
    expect(http?.data.focused).toBe(true);
    expect(http?.data.dimmed).toBe(false);
    // mapper 也在可达子图中
    const mapper = nodes.find((n) => n.id === 'node:mapper');
    expect(mapper?.data.focused).toBe(true);
    expect(mapper?.data.dimmed).toBe(false);
  });

  it('intentFocus dims nodes outside focus subgraph', () => {
    const def = createBlankDefinition('source:branch');
    def.flow = {
      nodes: [
        { id: 'a', config: { kind: 'http' as const, value: {} } },
        { id: 'b', config: { kind: 'mapper' as const, value: {} } },
        { id: 'c', config: { kind: 'mapper' as const, value: {} } },
      ],
      edges: [{ from: { node_id: 'a', handle: 'out' }, to: { node_id: 'b', handle: 'in' } }],
    };
    def.intent_exports = { Search: { flow_entry: 'a', mapper_output: 'b' } };
    const { nodes } = semanticToFlow(def, 'Search' as StandardIntent, null);
    // a, b in focus; c outside → dimmed
    expect(nodes.find((n) => n.id === 'a')?.data.focused).toBe(true);
    expect(nodes.find((n) => n.id === 'b')?.data.focused).toBe(true);
    expect(nodes.find((n) => n.id === 'c')?.data.focused).toBe(false);
    expect(nodes.find((n) => n.id === 'c')?.data.dimmed).toBe(true);
  });

  it('null intentFocus has no dimmed markers', () => {
    const def = connectedDefinition();
    const { nodes } = semanticToFlow(def, null, null);
    for (const node of nodes) {
      expect(node.data.focused).toBe(false);
      expect(node.data.dimmed).toBe(false);
    }
  });

  it('selection marks the selected node', () => {
    const def = sevenNodeDefinition();
    const { nodes } = semanticToFlow(def, null, 'node:http');
    expect(nodes.find((n) => n.id === 'node:http')?.selected).toBe(true);
    expect(nodes.find((n) => n.id === 'node:js')?.selected).toBe(false);
  });

  it('selection marks the selected edge without selecting either node', () => {
    const def = connectedDefinition();
    const selectedId = edgeId(def.flow.edges[0]);
    const { nodes, edges } = semanticToFlow(def, null, `edge:${selectedId}`);
    expect(edges.find((edge) => edge.id === selectedId)?.selected).toBe(true);
    expect(nodes.every((node) => node.selected !== true)).toBe(true);
  });

  it('port selection keeps node unselected and marks the visible handle', () => {
    const def = sevenNodeDefinition();
    const selection = portSelectionToken('node:js', 'source', 'output');
    const { nodes } = semanticToFlow(def, null, selection);
    const node = nodes.find((candidate) => candidate.id === 'node:js');
    expect(node?.selected).toBe(false);
    expect(node?.data.selectedPort).toEqual({ direction: 'source', id: 'json' });
  });

  it('collapsed Loop region hides only internal nodes and edges', () => {
    const definition = loopRegionDefinition();
    const layout = {
      nodes: {},
      loopRegions: { loop: { collapsed: true } },
    };
    const { nodes, edges, loopRegions } = semanticToFlow(definition, layout, null, null);
    expect(loopRegions[0]?.collapsed).toBe(true);
    expect(nodes.find((node) => node.id === 'body-js')?.hidden).toBe(true);
    expect(nodes.find((node) => node.id === 'loop')?.hidden).toBe(false);
    expect(edges.find((edge) => edge.id === 'body-js:output->body-extract:input')?.hidden).toBe(
      true,
    );
    expect(edges.find((edge) => edge.id === 'loop:body->body-js:input')?.hidden).toBe(false);
  });
});

describe('EditorSelection', () => {
  it('encodes node and handle delimiters without ambiguity', () => {
    const token = portSelectionToken('node:one', 'target', 'in:primary');
    expect(parseEditorSelection(token)).toEqual({
      kind: 'port',
      nodeId: 'node:one',
      direction: 'target',
      handle: 'in:primary',
    });
  });
});

// ---------------------------------------------------------------------------
// edgeId 确定性
// ---------------------------------------------------------------------------

describe('edgeId', () => {
  it('derives deterministic id from semantic identity', () => {
    const e1: FlowEdge = {
      from: { node_id: 'a', handle: 'out' },
      to: { node_id: 'b', handle: 'in' },
    };
    const e2: FlowEdge = {
      from: { node_id: 'a', handle: 'out' },
      to: { node_id: 'b', handle: 'in' },
    };
    expect(edgeId(e1)).toBe(edgeId(e2));
  });

  it('different edges have different ids', () => {
    const e1: FlowEdge = {
      from: { node_id: 'a', handle: 'out' },
      to: { node_id: 'b', handle: 'in' },
    };
    const e2: FlowEdge = {
      from: { node_id: 'a', handle: 'out' },
      to: { node_id: 'c', handle: 'in' },
    };
    expect(edgeId(e1)).not.toBe(edgeId(e2));
  });
});

// ---------------------------------------------------------------------------
// reachableNodes BFS
// ---------------------------------------------------------------------------

describe('reachableNodes', () => {
  it('returns entry and all downstream nodes', () => {
    const edges = [
      { from: { node_id: 'a', handle: 'out' }, to: { node_id: 'b', handle: 'in' } },
      { from: { node_id: 'b', handle: 'out' }, to: { node_id: 'c', handle: 'in' } },
    ];
    const result = reachableNodes('a', edges);
    expect([...result].sort()).toEqual(['a', 'b', 'c']);
  });

  it('does not traverse into disconnected branches', () => {
    const edges = [
      { from: { node_id: 'a', handle: 'out' }, to: { node_id: 'b', handle: 'in' } },
      { from: { node_id: 'c', handle: 'out' }, to: { node_id: 'd', handle: 'in' } },
    ];
    const result = reachableNodes('a', edges);
    expect([...result]).toEqual(['a', 'b']);
  });

  it('handles empty edge set', () => {
    expect([...reachableNodes('a', [])]).toEqual(['a']);
  });
});

// ---------------------------------------------------------------------------
// flowToSemantic 映射
// ---------------------------------------------------------------------------

describe('flowToSemantic', () => {
  it('connect → edgeConnect(connected: true)', () => {
    const edge: FlowEdge = {
      from: { node_id: 'a', handle: 'out' },
      to: { node_id: 'b', handle: 'in' },
    };
    const change: FlowUIChange = { kind: 'connect', edge };
    const action = flowToSemantic(change);
    expect(action).not.toBeInstanceOf(Array);
    expect('kind' in action && action.kind === 'edgeConnect').toBe(true);
    if ('kind' in action && action.kind === 'edgeConnect') {
      expect(action.edge).toEqual(edge);
      expect(action.connected).toBe(true);
    }
  });

  it('disconnect → edgeConnect(connected: false)', () => {
    const edge: FlowEdge = {
      from: { node_id: 'a', handle: 'out' },
      to: { node_id: 'b', handle: 'in' },
    };
    const action = flowToSemantic({ kind: 'disconnect', edge });
    expect('kind' in action && action.kind === 'edgeConnect').toBe(true);
    if ('kind' in action && action.kind === 'edgeConnect') {
      expect(action.connected).toBe(false);
    }
  });

  it('reconnect → array of two actions [disconnect, connect]', () => {
    const from: FlowEdge = {
      from: { node_id: 'a', handle: 'out' },
      to: { node_id: 'b', handle: 'in' },
    };
    const to: FlowEdge = {
      from: { node_id: 'a', handle: 'out' },
      to: { node_id: 'c', handle: 'in' },
    };
    const actions = flowToSemantic({ kind: 'reconnect', from, to });
    expect(actions).toBeInstanceOf(Array);
    expect(actions).toHaveLength(2);
    if (Array.isArray(actions)) {
      expect(actions[0]).toEqual({ kind: 'edgeConnect', edge: from, connected: false });
      expect(actions[1]).toEqual({ kind: 'edgeConnect', edge: to, connected: true });
    }
  });

  it('move → moveNode (`layout` domain)', () => {
    const action = flowToSemantic({ kind: 'move', nodeId: 'a', position: { x: 100, y: 200 } });
    expect('kind' in action && action.kind === 'moveNode').toBe(true);
    if ('kind' in action && action.kind === 'moveNode') {
      expect(action.position).toEqual({ x: 100, y: 200 });
    }
  });

  it('select → selection (transient, no history)', () => {
    const action = flowToSemantic({ kind: 'select', nodeId: 'a' });
    expect(action).toEqual({ kind: 'selection', nodeId: 'a' });
  });
});
