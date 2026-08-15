//! 端口合同测试：七类节点的 handle 集合与类型兼容矩阵。
//!
//! 合同（与 FlowPortRef.handle 一致）：
//! Http 规则入口 + http_response 输出；Js 输入 + 配置选择的 json/raw 输出；
//! Extract http_response→json；Mapper json→delta；Merge named inputs→json；
//! Condition json→json 分支；Loop collection→body(yield)→done。

import { describe, expect, it } from 'vitest';
import {
  PORT_CONTRACT,
  findInputPort,
  findOutputPort,
  getNodePorts,
  handleCompatible,
  portPositionPercent,
  portPositionStyle,
  portCompatible,
  type OutputPortDef,
  type InputPortDef,
} from './ports';

/** 取某类节点的输出端口。 */
function out(kind: Parameters<typeof findOutputPort>[0], id: string): OutputPortDef {
  const port = findOutputPort(kind, id);
  expect(port, `缺少输出端口 ${kind}#${id}`).toBeDefined();
  return port as OutputPortDef;
}

/** 取某类节点的输入端口。 */
function input(kind: Parameters<typeof findInputPort>[0], id: string): InputPortDef {
  const port = findInputPort(kind, id);
  expect(port, `缺少输入端口 ${kind}#${id}`).toBeDefined();
  return port as InputPortDef;
}

describe('PORT_CONTRACT：七类节点 handle 集合', () => {
  it('覆盖全部七类节点类型', () => {
    expect(Object.keys(PORT_CONTRACT).sort()).toEqual(
      ['condition', 'extract', 'http', 'js', 'loop', 'mapper', 'merge'].sort(),
    );
  });

  it('Http：只显示 http_response 输出，规则入口由入口标识表达', () => {
    const contract = PORT_CONTRACT.http;
    expect(contract.inputs).toEqual([]);
    expect(contract.outputs.map((p) => p.id)).toEqual(['http_response']);
    expect(out('http', 'http_response').emits).toBe('http_response');
    expect(out('http', 'http_response').side).toBe('right');
  });

  it('Js：input 输入 + 默认 JSON 输出', () => {
    const contract = PORT_CONTRACT.js;
    expect(contract.inputs.map((p) => p.id)).toEqual(['in']);
    expect(contract.outputs.map((p) => p.id)).toEqual(['json']);
    expect(out('js', 'json').emits).toBe('json');
    expect(contract.inputs[0].side).toBe('left');
    expect(contract.outputs[0].side).toBe('right');
    expect(getNodePorts('js', { output: 'raw' }).outputs.map((p) => p.id)).toEqual(['raw']);
    expect(getNodePorts('js', { output: 'raw' }).outputs[0].side).toBe('right');
  });

  it('Extract：source 输入 + json 输出（http_response→json）', () => {
    const contract = PORT_CONTRACT.extract;
    expect(contract.inputs.map((p) => p.id)).toEqual(['source']);
    expect(contract.outputs.map((p) => p.id)).toEqual(['json']);
    expect(contract.inputs[0].side).toBe('left');
    expect(contract.outputs[0].side).toBe('right');
    expect(handleCompatible('http', 'http_response', 'extract', 'source')).toBe(true);
  });

  it('Mapper：in 输入 + delta 输出（json→delta）', () => {
    const contract = PORT_CONTRACT.mapper;
    expect(contract.inputs.map((p) => p.id)).toEqual(['in']);
    expect(contract.outputs.map((p) => p.id)).toEqual(['delta']);
    expect(contract.inputs[0].side).toBe('left');
    expect(contract.outputs[0].side).toBe('right');
    expect(handleCompatible('js', 'json', 'mapper', 'in')).toBe(true);
  });

  it('Merge：命名多输入 + json 输出', () => {
    const contract = PORT_CONTRACT.merge;
    expect(contract.inputs.length).toBeGreaterThanOrEqual(2);
    expect(contract.inputs.every((p) => p.id.startsWith('in:'))).toBe(true);
    expect(contract.inputs.every((p) => p.side === 'left')).toBe(true);
    expect(contract.outputs.map((p) => p.id)).toEqual(['json']);
    expect(contract.outputs[0].side).toBe('right');
    expect(handleCompatible('js', 'json', 'merge', 'in:0')).toBe(true);
  });

  it('Condition：in 输入 + 多分支输出（json→branch outputs）', () => {
    const contract = PORT_CONTRACT.condition;
    expect(contract.inputs.map((p) => p.id)).toEqual(['in']);
    expect(contract.outputs.length).toBeGreaterThanOrEqual(2);
    expect(contract.outputs.every((p) => p.id.startsWith('branch:'))).toBe(true);
    expect(contract.inputs[0].side).toBe('left');
    expect(contract.outputs.every((p) => p.side === 'right')).toBe(true);
    expect(handleCompatible('js', 'json', 'condition', 'in')).toBe(true);
  });

  it('Loop：collection 输入 + body(yield)|done 输出', () => {
    const contract = PORT_CONTRACT.loop;
    expect(contract.inputs.map((p) => p.id)).toEqual(['in', 'yield']);
    expect(contract.outputs.map((p) => p.id)).toEqual(['body', 'done']);
    expect(contract.inputs.map((p) => p.side)).toEqual(['left', 'top']);
    expect(contract.outputs.map((p) => p.side)).toEqual(['bottom', 'right']);
    expect(out('loop', 'body').emits).toBe('loop_binding');
    expect(out('loop', 'done').emits).toBe('json');
    expect(handleCompatible('js', 'json', 'loop', 'in')).toBe(true);
  });
});

describe('端口类型兼容矩阵', () => {
  it('Http.http_response 只可接入 Extract.source，不可接入 JS/Mapper', () => {
    expect(handleCompatible('http', 'http_response', 'extract', 'source')).toBe(true);
    expect(handleCompatible('http', 'http_response', 'js', 'in')).toBe(false);
    expect(handleCompatible('http', 'http_response', 'mapper', 'in')).toBe(false);
  });

  it('Js.json 可接入 JSON 节点，Js.raw 只能接入 JS 输入', () => {
    expect(handleCompatible('js', 'json', 'mapper', 'in')).toBe(true);
    expect(handleCompatible('js', 'json', 'condition', 'in')).toBe(true);
    expect(handleCompatible('js', 'json', 'loop', 'in')).toBe(true);
    expect(handleCompatible('js', 'json', 'merge', 'in:0')).toBe(true);
    expect(handleCompatible('js', 'raw', 'extract', 'source')).toBe(false);
    expect(handleCompatible('js', 'raw', 'mapper', 'in')).toBe(false);
  });

  it('Mapper.delta 不冒充 JSON，不能接入 Merge/JS/Condition', () => {
    expect(handleCompatible('mapper', 'delta', 'merge', 'in:1')).toBe(false);
    expect(handleCompatible('mapper', 'delta', 'js', 'in')).toBe(false);
    expect(handleCompatible('mapper', 'delta', 'condition', 'in')).toBe(false);
  });

  it('Condition 分支与 Loop body/done 可接入 Js.in', () => {
    expect(handleCompatible('condition', 'branch:0', 'js', 'in')).toBe(true);
    expect(handleCompatible('loop', 'body', 'js', 'in')).toBe(true);
    expect(handleCompatible('loop', 'done', 'js', 'in')).toBe(true);
    expect(handleCompatible('loop', 'done', 'condition', 'in')).toBe(true);
  });

  it('不存在或错配的 handle 一律不兼容', () => {
    expect(handleCompatible('http', 'nope', 'js', 'in')).toBe(false);
    expect(handleCompatible('js', 'json', 'extract', 'in')).toBe(false); // extract 没有 in
    expect(handleCompatible('condition', 'branch:0', 'mapper', 'in')).toBe(true);
    expect(findInputPort('http', 'in')).toBeUndefined();
    expect(findOutputPort('loop', 'done')).toBeDefined();
    expect(findInputPort('condition', 'branch:0')).toBeUndefined();
  });

  it('portCompatible 按输入端口 accepts 矩阵判定', () => {
    const jsIn = input('js', 'in');
    const jsonOut = out('js', 'json');
    expect(portCompatible(jsonOut, jsIn)).toBe(true);
    // 输入端口不声明该类型则不兼容
    const conditionIn = input('condition', 'in');
    expect(portCompatible(out('loop', 'done'), conditionIn)).toBe(true);
  });
});

describe('动态端口投影', () => {
  it('Merge 使用 canonical inputs 的真实 handle 与顺序', () => {
    const ports = getNodePorts('merge', {
      inputs: [
        { input_id: 'primary', handle: 'source:primary', order: 1 },
        { input_id: 'fallback', handle: 'source:fallback', order: 0 },
      ],
    });
    expect(ports.inputs.map((port) => [port.id, port.label])).toEqual([
      ['source:fallback', 'fallback'],
      ['source:primary', 'primary'],
    ]);
  });

  it('Condition 按 branches 生成真实分支端口', () => {
    const ports = getNodePorts('condition', { branches: ['match', 'miss', 'unknown'] });
    expect(ports.outputs.map((port) => [port.id, port.label])).toEqual([
      ['branch:0', 'match'],
      ['branch:1', 'miss'],
      ['branch:2', 'unknown'],
    ]);
    expect(ports.outputs.every((port) => port.role === 'control')).toBe(true);
  });

  it('Loop 端口保留 collection/body/yield/done 结构角色', () => {
    const ports = getNodePorts('loop', {});
    expect(ports.inputs.map((port) => [port.id, port.role])).toEqual([
      ['in', 'data'],
      ['yield', 'control'],
    ]);
    expect(ports.outputs.map((port) => [port.id, port.role])).toEqual([
      ['body', 'binding'],
      ['done', 'control'],
    ]);
    expect(ports.inputs.map((port) => [port.id, port.side])).toEqual([
      ['in', 'left'],
      ['yield', 'top'],
    ]);
    expect(ports.outputs.map((port) => [port.id, port.side])).toEqual([
      ['body', 'bottom'],
      ['done', 'right'],
    ]);
  });
});

describe('端口相对位置', () => {
  it('单端口位于 50%', () => {
    expect(portPositionPercent(0, 1)).toBe(50);
    expect(portPositionStyle('left', 0, 1)).toBe('top: 50%;');
    expect(portPositionStyle('top', 0, 1)).toBe('left: 50%;');
  });

  it('多端口按相对比例分布，不使用固定像素间距', () => {
    expect([0, 1, 2].map((index) => portPositionPercent(index, 3))).toEqual([25, 50, 75]);
    expect(portPositionStyle('right', 0, 3)).toBe('top: 25%;');
    expect(portPositionStyle('right', 2, 3)).toBe('top: 75%;');
    expect(portPositionStyle('bottom', 1, 3)).toBe('left: 50%;');
  });
});
