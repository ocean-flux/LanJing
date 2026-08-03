import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import EdgeInspector from './EdgeInspector.svelte';
import type { FlowViewEdge } from '../nodes/types';
import type { FlowNode } from '$lib/rules/native-authoring/wire';

const sourceNode: FlowNode = {
  id: 'node:http',
  config: { kind: 'http', value: {} },
};

const targetNode: FlowNode = {
  id: 'node:extract',
  config: { kind: 'extract', value: {} },
};

const edge: FlowViewEdge = {
  id: 'node:http:output->node:extract:input',
  source: 'node:http',
  target: 'node:extract',
  sourceHandle: 'http_response',
  targetHandle: 'source',
  selected: true,
  data: {
    edge: {
      from: { node_id: 'node:http', handle: 'output' },
      to: { node_id: 'node:extract', handle: 'input' },
    },
    dimmed: false,
    selected: true,
    role: 'data',
    lane: 'main',
    valueKind: 'http_response',
    route: 'normal',
  },
};

describe('EdgeInspector', () => {
  it('shows semantic endpoints and derived value kind', () => {
    render(EdgeInspector, {
      props: { edge, sourceNode, targetNode, onDelete: vi.fn() },
    });

    expect(screen.getByText('连线语义')).toBeTruthy();
    expect(screen.getByText('HTTP 请求')).toBeTruthy();
    expect(screen.getByText('提取')).toBeTruthy();
    expect(screen.getByText('http_response')).toBeTruthy();
    expect(screen.getAllByText('HTTP 响应')).toHaveLength(2);
  });

  it('deletes the selected edge through the typed callback', async () => {
    const onDelete = vi.fn();
    render(EdgeInspector, {
      props: { edge, sourceNode, targetNode, onDelete },
    });

    await fireEvent.click(screen.getByRole('button', { name: '删除连线' }));
    expect(onDelete).toHaveBeenCalledTimes(1);
  });

  it('shows an empty selection state without semantic data', () => {
    render(EdgeInspector, {
      props: { edge: null, sourceNode: null, targetNode: null, onDelete: vi.fn() },
    });
    expect(screen.getByText('选择一条连线查看来源、目标和语义')).toBeTruthy();
  });
});
