//! 端口展示合同测试：handle 集合、类型兼容与几何辅助。
//!
//! handle 的唯一定义在 Rust descriptor（`lj-rule-model::descriptor`）：这里只断言
//! descriptor → 视图层的映射，不复制一份合同。handle 一律是 compiler 的 semantic
//! 形式（`input` / `output` / branch 名 / `in:N`），视图不再维护第二套可视命名。

import { describe, expect, it } from 'vitest';

import {
  findInputPort,
  findOutputPort,
  getNodePorts,
  handleCompatible,
  portCompatible,
  portPositionPercent,
  portPositionStyle,
  type OutputPortDef,
  type PortType,
} from './ports';

const LINEAR_KINDS = ['http', 'js', 'extract', 'mapper'];

/** 一个输出端口定义（兼容矩阵的 out 侧）。 */
function output(emits: PortType): OutputPortDef {
  return { id: 'out', label: { kind: 'literal', text: 'out' }, emits, side: 'right' };
}

describe('端口合同', () => {
  it('linear_kinds_expose_exactly_one_typed_input_and_output', () => {
    for (const kind of LINEAR_KINDS) {
      const ports = getNodePorts(kind, {});
      expect(ports.inputs).toHaveLength(1);
      expect(ports.outputs).toHaveLength(1);
      expect(ports.outputs[0]?.emits).toBeTypeOf('string');
    }
  });

  it('js_output_kind_follows_config_output', () => {
    expect(getNodePorts('js', { output: 'json' }).outputs[0]?.emits).toBe('json');
    expect(getNodePorts('js', { output: 'raw' }).outputs[0]?.emits).toBe('raw');
  });

  it('merge_input_handles_follow_config_order', () => {
    const ports = getNodePorts('merge', {
      inputs: [
        { input_id: 'a', handle: 'in:0', order: 0, activation: 'required' },
        { input_id: 'b', handle: 'in:1', order: 1, activation: 'optional' },
      ],
    });
    expect(ports.inputs.map((port) => port.id)).toEqual(['in:0', 'in:1']);
    expect(ports.inputs[0]?.accepts).toContain('json');
    expect(ports.outputs[0]?.emits).toBe('json');
  });

  it('condition_branch_handles_follow_config_branches', () => {
    const ports = getNodePorts('condition', { branches: ['a', 'b', 'c'] });
    expect(ports.outputs.map((port) => port.id)).toEqual(['a', 'b', 'c']);
    expect(ports.inputs[0]?.accepts).toContain('json');
  });

  it('loop_keeps_collection_input_yield_input_and_two_outputs', () => {
    const ports = getNodePorts('loop', {});
    expect(ports.inputs.map((port) => port.id).sort()).toEqual(['collection', 'yield']);
    expect(ports.outputs.map((port) => port.id).sort()).toEqual(['body', 'done']);
    expect(findOutputPort('loop', 'body')?.emits).toBe('loop_binding');
  });

  it('unknown_capability_declares_no_ports', () => {
    expect(getNodePorts('not-installed', {})).toEqual({ inputs: [], outputs: [] });
    expect(findInputPort('not-installed', 'input')).toBeUndefined();
  });

  it('ports_are_compatible_only_when_the_input_accepts_the_output_kind', () => {
    const input = findInputPort('mapper', 'input');
    if (input === undefined) throw new Error('mapper 缺少输入端口');
    expect(portCompatible(output('json'), input)).toBe(true);
    expect(portCompatible(output('http_response'), input)).toBe(false);
    expect(handleCompatible('extract', 'output', 'mapper', 'input')).toBe(true);
    expect(handleCompatible('mapper', 'output', 'extract', 'input')).toBe(false);
  });

  it('port_positions_are_derived_from_physical_side', () => {
    expect(portPositionPercent(0, 1)).toBe(50);
    expect(portPositionPercent(0, 2)).toBeCloseTo(33.33, 1);
    expect(portPositionPercent(1, 2)).toBeCloseTo(66.67, 1);
    expect(portPositionStyle('left', 0, 2)).toBe('top:33.33333333333333%;');
    expect(portPositionStyle('bottom', 0, 1)).toBe('left:50%;');
  });
});
