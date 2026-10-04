//! 节点能力 descriptor 注册表：编辑器唯一的数据来源。
//!
//! 声明由 Rust 的 `lj-rule-model::descriptor` 出，经 `list_rule_node_descriptors`
//! IPC 取回。模块加载时先用同源生成的 fixture（`__fixtures__/node-descriptors.json`，
//! 由 Rust 测试比对）填充，浏览器 dev 与单测因此不需要 IPC；Tauri 下
//! `loadNodeDescriptors()` 会用真实声明替换它。
//!
//! `kind` 是 wire 字符串而不是闭集：未安装能力只会查不到声明，不会走新的分支。

import {
  listRuleNodeDescriptors,
  type NodeDescriptor,
  type NodePortDescriptor,
  type NodePortHandleSource,
  type NodePortRole,
  type NodePortSide,
  type NodePortValueSource,
  type NodePortLabelDescriptor,
  type PortValueKind,
} from '@/shared/tauri/rules';

import descriptorFixture from './__fixtures__/node-descriptors.json';

/** 节点 config 的 wire 形态（`{ kind, value }` 里的 `value`）。 */
export type NodeConfig = Record<string, unknown>;

/** 已解析的 port：handle 与 value kind 已落定。 */
export type ResolvedDescriptorPort = {
  handle: string;
  /** Closed value kinds；输出端口恰有一个。 */
  kinds: PortValueKind[];
  role: NodePortRole;
  side: NodePortSide;
  /** `messages/*\/rules.json` 的稳定 key；`literal` 为真时是原样显示的术语。 */
  labelKey: string;
  literal: boolean;
  /** 1-based 序号（`numbered` 标签用）。 */
  index?: number;
};

/** 一个节点已解析的 port 集合。 */
export type ResolvedDescriptorPorts = {
  inputs: ResolvedDescriptorPort[];
  outputs: ResolvedDescriptorPort[];
};

const fixture = descriptorFixture as { descriptors: NodeDescriptor[] };

let registry: readonly NodeDescriptor[] = fixture.descriptors;
const byKind = new Map<string, NodeDescriptor>(
  registry.map((descriptor) => [descriptor.kind, descriptor]),
);
let loadInFlight: Promise<void> | undefined;
let loaded = false;

/** 替换注册表（测试与 IPC 加载都经这里）。 */
export function registerNodeDescriptors(descriptors: readonly NodeDescriptor[]): void {
  registry = descriptors;
  byKind.clear();
  for (const descriptor of descriptors) byKind.set(descriptor.kind, descriptor);
}

/** 全部声明，顺序与 Rust 声明表一致（palette 顺序即此）。 */
export function nodeDescriptors(): readonly NodeDescriptor[] {
  return registry;
}

/** 按 wire kind 查声明；未安装能力返回 `undefined`。 */
export function nodeDescriptor(kind: string): NodeDescriptor | undefined {
  return byKind.get(kind);
}

/** 新节点的默认 config；未安装能力返回 `undefined`。 */
export function descriptorDefaultConfig(kind: string): NodeConfig | undefined {
  return byKind.get(kind)?.default_config;
}

/** 从 Rust 取回声明；失败时保留已有声明（不把编辑器降级成空表）。 */
export function loadNodeDescriptors(): Promise<void> {
  if (loaded) return Promise.resolve();
  loadInFlight ??= listRuleNodeDescriptors()
    .then((set) => {
      if (set.descriptors.length > 0) registerNodeDescriptors(set.descriptors);
      loaded = true;
    })
    .catch(() => {
      loadInFlight = undefined;
    });
  return loadInFlight;
}

/** 解析一个 port 的 handle 列表；`null` 表示固定 handle。 */
function resolveHandles(source: NodePortHandleSource, config: NodeConfig): string[] | null {
  switch (source.source) {
    case 'fixed': {
      return null;
    }
    case 'items': {
      const items = config[source.field];
      return Array.isArray(items) ? items.filter((item) => typeof item === 'string') : [];
    }
    case 'item_field': {
      const items = config[source.field];
      if (!Array.isArray(items)) return [];
      return items
        .map((item) =>
          typeof item === 'object' && item !== null
            ? (item as Record<string, unknown>)[source.handle_field]
            : undefined,
        )
        .filter((handle): handle is string => typeof handle === 'string');
    }
  }
}

/** 解析一个 port 的 value kinds，以及 `field_variant` 带来的标签覆盖。 */
function resolveValue(
  source: NodePortValueSource,
  config: NodeConfig,
): { kinds: PortValueKind[]; labelKey?: string } {
  switch (source.source) {
    case 'kind': {
      return { kinds: [source.kind] };
    }
    case 'union': {
      return { kinds: [...source.kinds] };
    }
    case 'field_variant': {
      const selected = config[source.field];
      const variant =
        source.variants.find((candidate) => candidate.value === selected) ?? source.variants[0];
      if (variant === undefined) return { kinds: ['json'] };
      return { kinds: [variant.kind], labelKey: variant.label_key };
    }
  }
}

function fixedHandle(source: NodePortHandleSource): string {
  return source.source === 'fixed' ? source.handle : '';
}

/** 展开一条声明为若干已解析 port；空 handle 列表表示该 port 当前不存在。 */
function resolvePort(
  port: NodePortDescriptor,
  config: NodeConfig,
  position: number,
): ResolvedDescriptorPort[] {
  const { kinds, labelKey } = resolveValue(port.value, config);
  const label: NodePortLabelDescriptor = port.label;
  const withIndex = (index: number | undefined): number | undefined =>
    label.numbered ? (index ?? position + 1) : undefined;
  const handles = resolveHandles(port.handle, config);
  if (handles === null) {
    return [
      {
        handle: fixedHandle(port.handle),
        kinds,
        role: port.role,
        side: port.side,
        labelKey: labelKey ?? label.key,
        literal: label.literal,
        index: withIndex(undefined),
      },
    ];
  }
  return handles.map((handle, offset) => ({
    handle,
    kinds,
    role: port.role,
    side: port.side,
    labelKey: labelKey ?? label.key,
    literal: label.literal,
    index: withIndex(position + offset + 1),
  }));
}

/** 按 config 解析一个节点的全部 port。 */
export function resolveDescriptorPorts(
  descriptor: NodeDescriptor,
  config: NodeConfig,
): ResolvedDescriptorPorts {
  // 缺省字段用 descriptor 的默认 config 补齐：调用方（palette 推荐、端口检查）
  // 常常只有部分 config，port 语义仍应与完整节点一致。
  const effective = { ...descriptor.default_config, ...config };
  const resolve = (ports: NodePortDescriptor[]): ResolvedDescriptorPort[] =>
    ports.flatMap((port, position) => resolvePort(port, effective, position));
  return { inputs: resolve(descriptor.inputs), outputs: resolve(descriptor.outputs) };
}
