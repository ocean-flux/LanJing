//! 节点端口合同：七类节点的 handle、value kind 与结构角色。

import type { FlowNodeKind } from '$lib/rules/native-authoring/wire';

/** 端口数据类型（闭集；与 compiler 的 PortValueKind 对齐）。 */
export type PortType = 'http_response' | 'json' | 'raw' | 'delta' | 'loop_binding';

/** 端口在规则图中的结构角色。 */
export type PortRole = 'data' | 'control' | 'binding';

/** 端口在节点边界上的语义侧位。 */
export type PortSide = 'left' | 'right' | 'top' | 'bottom';

/** 语义输入端口。 */
export type InputPortDef = {
  id: string;
  label: string;
  accepts: PortType[];
  role?: PortRole;
  side: PortSide;
};

/** 语义输出端口。 */
export type OutputPortDef = {
  id: string;
  label: string;
  emits: PortType;
  role?: PortRole;
  side: PortSide;
};

/** 单类节点的端口集合。 */
export type NodePorts = {
  inputs: InputPortDef[];
  outputs: OutputPortDef[];
};

/** 七类节点的空配置回退合同。动态节点使用 getNodePorts。 */
export const PORT_CONTRACT: Record<FlowNodeKind, NodePorts> = {
  http: {
    inputs: [],
    outputs: [
      {
        id: 'http_response',
        label: 'HTTP 响应',
        emits: 'http_response',
        role: 'data',
        side: 'right',
      },
    ],
  },
  js: {
    inputs: [
      {
        id: 'in',
        label: '脚本输入',
        accepts: ['raw', 'json', 'loop_binding'],
        role: 'data',
        side: 'left',
      },
    ],
    outputs: [{ id: 'json', label: 'JSON 输出', emits: 'json', role: 'data', side: 'right' }],
  },
  extract: {
    inputs: [
      { id: 'source', label: 'HTTP 响应', accepts: ['http_response'], role: 'data', side: 'left' },
    ],
    outputs: [{ id: 'json', label: '提取结果', emits: 'json', role: 'data', side: 'right' }],
  },
  mapper: {
    inputs: [{ id: 'in', label: 'JSON 输入', accepts: ['json'], role: 'data', side: 'left' }],
    outputs: [{ id: 'delta', label: '增量输出', emits: 'delta', role: 'data', side: 'right' }],
  },
  merge: {
    inputs: [
      { id: 'in:0', label: '输入 1', accepts: ['json'], role: 'data', side: 'left' },
      { id: 'in:1', label: '输入 2', accepts: ['json'], role: 'data', side: 'left' },
    ],
    outputs: [{ id: 'json', label: '合并结果', emits: 'json', role: 'data', side: 'right' }],
  },
  condition: {
    inputs: [{ id: 'in', label: '条件输入', accepts: ['json'], role: 'data', side: 'left' }],
    outputs: [
      { id: 'branch:0', label: '分支 1', emits: 'json', role: 'control', side: 'right' },
      { id: 'branch:1', label: '分支 2', emits: 'json', role: 'control', side: 'right' },
      { id: 'branch:2', label: '分支 3', emits: 'json', role: 'control', side: 'right' },
    ],
  },
  loop: {
    inputs: [
      { id: 'in', label: 'collection', accepts: ['json'], role: 'data', side: 'left' },
      { id: 'yield', label: 'yield(value)', accepts: ['json'], role: 'control', side: 'top' },
    ],
    outputs: [
      {
        id: 'body',
        label: 'body(item,index)',
        emits: 'loop_binding',
        role: 'binding',
        side: 'bottom',
      },
      { id: 'done', label: 'done(collected)', emits: 'json', role: 'control', side: 'right' },
    ],
  },
};

type NodeConfig = Record<string, unknown>;

function stringArray(config: NodeConfig, key: string): string[] {
  const value = config[key];
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string' && item.length > 0)
    : [];
}

function mergeInputs(
  config: NodeConfig,
): Array<{ input_id: string; handle: string; order: number; activation?: string }> {
  const value = config.inputs;
  if (!Array.isArray(value)) return [];
  return value
    .filter((item): item is Record<string, unknown> => typeof item === 'object' && item !== null)
    .map((item, index) => ({
      input_id: typeof item.input_id === 'string' ? item.input_id : `input_${index + 1}`,
      handle: typeof item.handle === 'string' ? item.handle : `in:${index}`,
      order: typeof item.order === 'number' ? item.order : index,
      activation: typeof item.activation === 'string' ? item.activation : undefined,
    }))
    .sort((left, right) => left.order - right.order);
}

/** 按 canonical node config 生成动态端口；不修改 Definition。 */
export function getNodePorts(kind: FlowNodeKind, config: NodeConfig = {}): NodePorts {
  if (kind === 'js') {
    const output = typeof config.output === 'string' ? config.output.toLowerCase() : 'json';
    return {
      inputs: PORT_CONTRACT.js.inputs,
      outputs: [
        output === 'raw'
          ? { id: 'raw', label: '原文输出', emits: 'raw', role: 'data', side: 'right' }
          : PORT_CONTRACT.js.outputs[0],
      ],
    };
  }

  if (kind === 'merge') {
    const inputs = mergeInputs(config);
    const declaredInputs =
      inputs.length > 0
        ? inputs
        : PORT_CONTRACT.merge.inputs.map((port, index) => ({
            input_id: port.label,
            handle: port.id,
            order: index,
          }));
    return {
      inputs: declaredInputs.map((input, index) => ({
        id: input.handle,
        label: input.input_id || `输入 ${index + 1}`,
        accepts: ['json'],
        role: 'data',
        side: 'left',
      })),
      outputs: PORT_CONTRACT.merge.outputs,
    };
  }

  if (kind === 'condition') {
    const branches = stringArray(config, 'branches');
    const visibleBranches = branches.length > 0 ? branches : ['true', 'false'];
    return {
      inputs: PORT_CONTRACT.condition.inputs,
      outputs: visibleBranches.map((branch, index) => ({
        id: `branch:${index}`,
        label: branch,
        emits: 'json',
        role: 'control',
        side: 'right',
      })),
    };
  }

  if (kind === 'loop') {
    return {
      inputs: [
        { id: 'in', label: 'collection', accepts: ['json'], role: 'data', side: 'left' },
        { id: 'yield', label: 'yield(value)', accepts: ['json'], role: 'control', side: 'top' },
      ],
      outputs: [
        {
          id: 'body',
          label: 'body(item,index)',
          emits: 'loop_binding',
          role: 'binding',
          side: 'bottom',
        },
        { id: 'done', label: 'done(collected)', emits: 'json', role: 'control', side: 'right' },
      ],
    };
  }

  return PORT_CONTRACT[kind];
}

/** 计算同侧端口的相对位置；不使用固定像素间距，避免节点高度变化后漂移。 */
export function portPositionPercent(index: number, count: number): number {
  if (count <= 1) return 50;
  const boundedIndex = Math.min(Math.max(index, 0), count - 1);
  return ((boundedIndex + 1) / (count + 1)) * 100;
}

/** 生成 Handle 的内联侧向坐标；xyflow 负责侧边边界与 transform。 */
export function portPositionStyle(side: PortSide, index: number, count: number): string {
  const percent = `${portPositionPercent(index, count)}%`;
  return side === 'left' || side === 'right' ? `top: ${percent};` : `left: ${percent};`;
}

/** 查找某类节点的输入端口。 */
export function findInputPort(
  kind: FlowNodeKind,
  handleId: string,
  config?: NodeConfig,
): InputPortDef | undefined {
  return getNodePorts(kind, config).inputs.find((port) => port.id === handleId);
}

/** 查找某类节点的输出端口。 */
export function findOutputPort(
  kind: FlowNodeKind,
  handleId: string,
  config?: NodeConfig,
): OutputPortDef | undefined {
  return getNodePorts(kind, config).outputs.find((port) => port.id === handleId);
}

/** 输出端口能否接入输入端口（按类型矩阵）。 */
export function portCompatible(out: OutputPortDef, input: InputPortDef): boolean {
  return input.accepts.includes(out.emits);
}

/** 按节点类型、配置与 handle id 判端口兼容。 */
export function handleCompatible(
  sourceKind: FlowNodeKind,
  sourceHandle: string,
  targetKind: FlowNodeKind,
  targetHandle: string,
  sourceConfig?: NodeConfig,
  targetConfig?: NodeConfig,
): boolean {
  const out = findOutputPort(sourceKind, sourceHandle, sourceConfig);
  const input = findInputPort(targetKind, targetHandle, targetConfig);
  if (!out || !input) return false;
  return portCompatible(out, input);
}
