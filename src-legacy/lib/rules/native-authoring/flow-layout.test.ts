import { describe, expect, it } from 'vitest';
import { createBlankDefinition } from './core';
import { fallbackFlowLayout, layoutNativeRuleFlow } from './flow-layout';
import type { RuleDefinition } from './wire';

function branchingDefinition(): RuleDefinition {
  const definition = createBlankDefinition('source:layout');
  definition.intent_exports = {
    Search: { flow_entry: 'http', mapper_output: 'merge' },
  };
  definition.flow = {
    nodes: [
      { id: 'http', config: { kind: 'http', value: {} } },
      { id: 'extract', config: { kind: 'extract', value: {} } },
      {
        id: 'condition',
        config: { kind: 'condition', value: { branches: ['yes', 'no'] } },
      },
      { id: 'yes', config: { kind: 'js', value: { output: 'json' } } },
      { id: 'no', config: { kind: 'js', value: { output: 'json' } } },
      {
        id: 'merge',
        config: {
          kind: 'merge',
          value: {
            inputs: [
              { input_id: 'yes', handle: 'in:0', order: 0 },
              { input_id: 'no', handle: 'in:1', order: 1 },
            ],
          },
        },
      },
    ],
    edges: [
      { from: { node_id: 'http', handle: 'output' }, to: { node_id: 'extract', handle: 'input' } },
      {
        from: { node_id: 'extract', handle: 'output' },
        to: { node_id: 'condition', handle: 'input' },
      },
      { from: { node_id: 'condition', handle: 'yes' }, to: { node_id: 'yes', handle: 'input' } },
      { from: { node_id: 'condition', handle: 'no' }, to: { node_id: 'no', handle: 'input' } },
      { from: { node_id: 'yes', handle: 'output' }, to: { node_id: 'merge', handle: 'in:0' } },
      { from: { node_id: 'no', handle: 'output' }, to: { node_id: 'merge', handle: 'in:1' } },
    ],
  };
  return definition;
}

function loopDefinition(): RuleDefinition {
  const definition = createBlankDefinition('source:loop-layout');
  definition.flow = {
    nodes: [
      { id: 'source', config: { kind: 'http', value: {} } },
      { id: 'loop', config: { kind: 'loop', value: {} } },
      { id: 'body', config: { kind: 'js', value: {} } },
      { id: 'done', config: { kind: 'mapper', value: {} } },
    ],
    edges: [
      {
        from: { node_id: 'source', handle: 'output' },
        to: { node_id: 'loop', handle: 'collection' },
      },
      { from: { node_id: 'loop', handle: 'body' }, to: { node_id: 'body', handle: 'input' } },
      { from: { node_id: 'body', handle: 'output' }, to: { node_id: 'loop', handle: 'yield' } },
      { from: { node_id: 'loop', handle: 'done' }, to: { node_id: 'done', handle: 'input' } },
    ],
  };
  return definition;
}

describe('flow layout', () => {
  it('fallback layout creates readable layers for a branching graph', () => {
    const positions = fallbackFlowLayout(branchingDefinition());
    expect(positions.http.x).toBeLessThan(positions.extract.x);
    expect(positions.extract.x).toBeLessThan(positions.condition.x);
    expect(positions.condition.x).toBeLessThan(positions.yes.x);
    expect(positions.condition.x).toBeLessThan(positions.no.x);
    expect(positions.yes.y).not.toBe(positions.no.y);
    expect(positions.merge.x).toBeGreaterThan(positions.yes.x);
  });

  it('fallback layout uses measured node height when a node has expanded content', () => {
    const positions = fallbackFlowLayout(branchingDefinition(), {
      yes: { width: 252, height: 420 },
    });

    expect(positions.no.y - positions.yes.y).toBe(492);
  });

  it('fallback layout aligns single-node layers by node center', () => {
    const definition = createBlankDefinition('source:center');
    definition.intent_exports = { Search: { flow_entry: 'http', mapper_output: 'extract' } };
    definition.flow = {
      nodes: [
        { id: 'http', config: { kind: 'http', value: {} } },
        { id: 'extract', config: { kind: 'extract', value: {} } },
      ],
      edges: [
        {
          from: { node_id: 'http', handle: 'output' },
          to: { node_id: 'extract', handle: 'input' },
        },
      ],
    };
    const positions = fallbackFlowLayout(definition, {
      http: { width: 252, height: 180 },
      extract: { width: 252, height: 320 },
    });

    expect(positions.http.y + 90).toBe(positions.extract.y + 160);
  });

  it('ELK layout returns one position for every node without mutating definition', async () => {
    const definition = branchingDefinition();
    const before = JSON.stringify(definition);
    const positions = await layoutNativeRuleFlow(definition);

    expect(Object.keys(positions).sort()).toEqual(
      definition.flow.nodes.map((node) => node.id).sort(),
    );
    expect(JSON.stringify(definition)).toBe(before);
    expect(positions.http.x).toBeLessThan(positions.merge.x);
    expect(positions.yes.y).not.toBe(positions.no.y);
  });

  it('ELK layout keeps Loop body after the Loop node without using yield as a feedback constraint', async () => {
    const definition = loopDefinition();
    const positions = await layoutNativeRuleFlow(definition);

    expect(positions.source.x).toBeLessThan(positions.loop.x);
    expect(positions.loop.x).toBeLessThan(positions.body.x);
    expect(positions.loop.x).toBeLessThan(positions.done.x);
  });
});
