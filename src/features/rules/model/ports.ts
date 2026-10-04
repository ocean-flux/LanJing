//! 节点端口展示合同：descriptor 声明的 port 与视图层 label/几何辅助。
//!
//! 端口语义（handle、value kind）来自 `model/descriptor-registry`，本模块不再自带
//! kind → port 表。

import {
  nodeDescriptor,
  resolveDescriptorPorts,
  type ResolvedDescriptorPort,
} from './descriptor-registry';

/** 端口数据类型（闭集；与 compiler 的 PortValueKind 对齐）。 */
export type PortType = 'intent_input' | 'http_response' | 'json' | 'raw' | 'delta' | 'loop_binding';

/** 端口在规则图中的结构角色。 */
export type PortRole = 'data' | 'control' | 'binding';

/** 端口在节点边界上的语义侧位。 */
export type PortSide = 'left' | 'right' | 'top' | 'bottom';

/**
 * 端口展示标签。
 *
 * `message` 是需要本地化的静态端口名，视图查 `messages/*\/rules.json`；
 * `literal` 是不翻译的内容 —— 用户配置里的分支名 / input_id，以及 Loop 的
 * `collection` / `yield(value)` 这类 compiler 术语。模型层不持有 locale 文案。
 */
export type PortLabel =
  | { kind: 'message'; key: PortLabelKey; index?: number }
  | { kind: 'literal'; text: string };

/** 静态端口标签的稳定 key。 */
export type PortLabelKey =
  | 'http_response'
  | 'js_input'
  | 'json_output'
  | 'raw_output'
  | 'extract_result'
  | 'json_input'
  | 'delta_output'
  | 'merge_input'
  | 'merge_result'
  | 'condition_input'
  | 'condition_branch';

/** 节点 config 的 wire 形态。 */
type NodeConfig = Record<string, unknown>;

/** Descriptor 出的 port 标签 key 前缀（`messages/*\/rules.json`）。 */
const PORT_LABEL_PREFIX = 'rules_port_label_';

/** Descriptor 标签 → 视图标签；`literal` 是 compiler 术语，原样显示。 */
function descriptorPortLabel(port: ResolvedDescriptorPort): PortLabel {
  if (port.literal) return { kind: 'literal', text: port.labelKey };
  const key = (
    port.labelKey.startsWith(PORT_LABEL_PREFIX)
      ? port.labelKey.slice(PORT_LABEL_PREFIX.length)
      : port.labelKey
  ) as PortLabelKey;
  return port.index === undefined
    ? { kind: 'message', key }
    : { kind: 'message', key, index: port.index };
}

/** 输出端口的 value kind：descriptor 声明恰一个。 */
function descriptorOutputKind(port: ResolvedDescriptorPort): PortType {
  const [kind] = port.kinds;
  if (kind === undefined) throw new Error(`输出端口 ${port.handle} 缺少 value kind`);
  return kind;
}

/**
 * 按节点的 descriptor 声明解析端口（含 handle 与 value kind 的 config 派生）。
 *
 * 端口语义只有 descriptor 一个来源；`kind` 查不到声明时返回空端口集合，也就是
 * 「未安装能力」：可展示、可保存、可往返，但不可校验/编译/执行。
 */
export function getNodePorts(kind: string, config: NodeConfig = {}): NodePorts {
  const descriptor = nodeDescriptor(kind);
  if (descriptor === undefined) return { inputs: [], outputs: [] };
  const resolved = resolveDescriptorPorts(descriptor, config);
  return {
    inputs: resolved.inputs.map((port) => ({
      id: port.handle,
      label: descriptorPortLabel(port),
      accepts: [...port.kinds],
      role: port.role,
      side: port.side,
    })),
    outputs: resolved.outputs.map((port) => ({
      id: port.handle,
      label: descriptorPortLabel(port),
      emits: descriptorOutputKind(port),
      role: port.role,
      side: port.side,
    })),
  };
}
export type InputPortDef = {
  id: string;
  label: PortLabel;
  accepts: PortType[];
  role?: PortRole;
  side: PortSide;
};

/** 语义输出端口。 */
export type OutputPortDef = {
  id: string;
  label: PortLabel;
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
export function portPositionPercent(index: number, count: number): number {
  if (count <= 1) return 50;
  const boundedIndex = Math.min(Math.max(index, 0), count - 1);
  return ((boundedIndex + 1) / (count + 1)) * 100;
}

/** 生成 Handle 的内联侧向坐标；xyflow 负责侧边边界与 transform。 */
export function portPositionStyle(side: PortSide, index: number, count: number): string {
  const percent = `${portPositionPercent(index, count)}%`;
  return side === 'left' || side === 'right' ? `top:${percent};` : `left:${percent};`;
}

/** 查找某类节点的输入端口。 */
export function findInputPort(
  kind: string,
  handleId: string,
  config?: NodeConfig,
): InputPortDef | undefined {
  return getNodePorts(kind, config).inputs.find((port) => port.id === handleId);
}

/** 查找某类节点的输出端口。 */
export function findOutputPort(
  kind: string,
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
  sourceKind: string,
  sourceHandle: string,
  targetKind: string,
  targetHandle: string,
  sourceConfig?: NodeConfig,
  targetConfig?: NodeConfig,
): boolean {
  const out = findOutputPort(sourceKind, sourceHandle, sourceConfig);
  const input = findInputPort(targetKind, targetHandle, targetConfig);
  if (!out || !input) return false;
  return portCompatible(out, input);
}
