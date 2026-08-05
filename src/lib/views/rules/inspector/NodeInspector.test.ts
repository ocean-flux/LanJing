import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import NodeInspector from './NodeInspector.svelte';

describe('NodeInspector', () => {
  const baseProps = {
    nodeId: null,
    nodeType: null,
    config: null,
    onChange: vi.fn(),
  };

  it('shows placeholder when no node selected', () => {
    render(NodeInspector, { props: baseProps });
    expect(screen.getByText('选择一个节点开始编辑')).toBeTruthy();
  });

  it('shows typed fields for http node', () => {
    render(NodeInspector, {
      props: {
        ...baseProps,
        nodeId: 'node:http-1',
        nodeType: 'http',
        config: { url: '', method: 'Get' },
      },
    });
    expect(screen.getByText('URL')).toBeTruthy();
    expect(screen.getByText('方法')).toBeTruthy();
  });

  it('edits canonical http fields through a typed patch', async () => {
    const onChange = vi.fn();
    render(NodeInspector, {
      props: {
        ...baseProps,
        onChange,
        nodeId: 'node:http-1',
        nodeType: 'http',
        config: { url: '', method: 'Get' },
      },
    });

    const urlInput = screen.getByText('URL').parentElement?.querySelector('input');
    expect(urlInput).toBeTruthy();
    await fireEvent.input(urlInput!, { target: { value: 'https://example.test' } });
    expect(onChange).toHaveBeenCalledWith({ url: 'https://example.test' });
  });

  it('shows typed vs JS mode switch for condition node', () => {
    render(NodeInspector, {
      props: {
        ...baseProps,
        nodeId: 'node:cond-1',
        nodeType: 'condition',
        config: {
          branches: ['true', 'false'],
          expression: {
            mode: 'typed',
            predicate: { operator: 'eq', pointer: '$.x', value: '1' },
            true_branch: 'true',
            false_branch: 'false',
          },
        },
      },
    });
    expect(screen.getByRole('radio', { name: 'Typed' })).toBeTruthy();
    expect(screen.getByRole('radio', { name: 'JS' })).toBeTruthy();
  });

  it('shows canonical merge inputs and reorder controls', () => {
    render(NodeInspector, {
      props: {
        ...baseProps,
        nodeId: 'node:merge-1',
        nodeType: 'merge',
        config: {},
      },
    });
    expect(screen.getByDisplayValue('input_1')).toBeTruthy();
    expect(screen.getByDisplayValue('in:0')).toBeTruthy();
    expect(screen.getAllByRole('button', { name: '上移输入' })).toHaveLength(2);
  });

  it('reorder buttons emit a single inputs command with new order', async () => {
    const onChange = vi.fn();
    render(NodeInspector, {
      props: {
        ...baseProps,
        onChange,
        nodeId: 'node:merge-1',
        nodeType: 'merge',
        config: {},
      },
    });

    await fireEvent.click(screen.getAllByRole('button', { name: '上移输入' })[1]);

    expect(onChange).toHaveBeenCalledTimes(1);
    const patch = onChange.mock.calls[0]?.[0] as { inputs: { input_id: string; order: number }[] };
    expect(patch.inputs[0]?.input_id).toBe('input_2');
    expect(patch.inputs[0]?.order).toBe(0);
    expect(patch.inputs[1]?.input_id).toBe('input_1');
    expect(patch.inputs[1]?.order).toBe(1);
  });

  it('renaming a condition branch updates expression branch identity in same patch', async () => {
    const onChange = vi.fn();
    render(NodeInspector, {
      props: {
        ...baseProps,
        onChange,
        nodeId: 'node:cond-1',
        nodeType: 'condition',
        config: {
          branches: ['true', 'false'],
          expression: {
            mode: 'typed',
            predicate: { operator: 'exists', pointer: '$.title' },
            true_branch: 'true',
            false_branch: 'false',
          },
        },
      },
    });

    await fireEvent.input(screen.getAllByRole('textbox', { name: '分支 handle' })[0]!, {
      target: { value: 'match' },
    });
    expect(onChange).toHaveBeenLastCalledWith({
      branches: ['match', 'false'],
      expression: expect.objectContaining({ true_branch: 'match', false_branch: 'false' }),
    });
  });

  it('edits extract rules and loop bindings through canonical patches', async () => {
    const extractChange = vi.fn();
    render(NodeInspector, {
      props: {
        ...baseProps,
        onChange: extractChange,
        nodeId: 'node:extract-1',
        nodeType: 'extract',
        config: { rules: [], expected_type: 'Html', output_target: 'Media' },
      },
    });
    await fireEvent.click(screen.getByRole('button', { name: '添加提取规则' }));
    expect(extractChange).toHaveBeenCalledWith({
      rules: [{ CssSelector: { selector: '', extract_type: 'Text', regex_clean: null } }],
    });

    const loopChange = vi.fn();
    render(NodeInspector, {
      props: {
        ...baseProps,
        onChange: loopChange,
        nodeId: 'node:loop-1',
        nodeType: 'loop',
        config: {},
      },
    });
    await fireEvent.input(screen.getByDisplayValue('item'), { target: { value: 'entry' } });
    expect(loopChange).toHaveBeenCalledWith({ item_binding: 'entry' });
  });

  it('edits JS output port mode through canonical config', async () => {
    const onChange = vi.fn();
    render(NodeInspector, {
      props: {
        ...baseProps,
        onChange,
        nodeId: 'node:js-1',
        nodeType: 'js',
        config: { output: 'json', code: '' },
      },
    });
    const outputSelect = screen.getByRole('button', { name: '输出类型' });
    outputSelect.focus();
    await fireEvent.keyDown(outputSelect, { key: 'ArrowDown' });
    await screen.findByRole('listbox');
    await fireEvent.keyDown(document.activeElement ?? outputSelect, { key: 'ArrowDown' });
    await fireEvent.keyDown(document.activeElement ?? outputSelect, { key: 'Enter' });
    expect(onChange).toHaveBeenCalledWith({ output: 'raw' });
  });
});
