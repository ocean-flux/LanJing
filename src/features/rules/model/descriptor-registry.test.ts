//! descriptor 注册表测试：字段、端口与默认值只有 descriptor 一个来源。
//!
//! 这里同时是 #61 验收面的证据：新增规则能力只需在 Rust descriptor 表里加一条声明，
//! 前端不新增 React 分支、不新增核心 enum 分支。

import { afterEach, describe, expect, it } from 'vitest';

import type { NodeDescriptor } from '@/shared/tauri/rules';

import {
  descriptorDefaultConfig,
  descriptorHasCodeField,
  nodeDescriptor,
  nodeDescriptors,
  registerNodeDescriptors,
  resolveDescriptorPorts,
} from './descriptor-registry';
import { canonicalConfig } from './node-defaults';
import { getNodePorts, portCompatible } from './ports';

const BUILT_IN_KINDS = ['http', 'js', 'extract', 'mapper', 'merge', 'condition', 'loop'];

/** 未安装能力没有声明，因此没有可运行的源码字段。 */
function requireDescriptor(kind: string): NodeDescriptor {
  const descriptor = nodeDescriptor(kind);
  if (descriptor === undefined) throw new Error(`缺少 descriptor: ${kind}`);
  return descriptor;
}

/** 模块加载时的声明表快照，用例改动注册表后复位用。 */
const ORIGINAL_DESCRIPTORS = nodeDescriptors();

/** 没有任何字段的声明（未安装能力的等价物；只用于 code 字段判定）。 */
const unknownDescriptor: NodeDescriptor = {
  kind: 'custom_reader',
  label_key: 'rules_node_inspector_type_custom_reader',
  description_key: 'rules_node_inspector_type_custom_reader',
  icon: 'brackets-curly',
  inputs: [],
  outputs: [],
  fields: [],
  default_config: {},
};

describe('descriptor 注册表', () => {
  afterEach(() => {
    registerNodeDescriptors(ORIGINAL_DESCRIPTORS);
  });

  it('every_installed_capability_resolves_fields_ports_and_defaults_from_its_descriptor', () => {
    for (const kind of BUILT_IN_KINDS) {
      const descriptor = nodeDescriptor(kind);
      if (descriptor === undefined) throw new Error(`未安装能力缺少 descriptor: ${kind}`);
      expect(descriptor.fields.length).toBeGreaterThan(0);

      const ports = getNodePorts(kind, canonicalConfig(kind, null));
      expect(ports.outputs.length).toBeGreaterThan(0);
      for (const output of ports.outputs) {
        expect(output.emits).toBeTypeOf('string');
      }
    }
  });

  it('dynamic_ports_come_from_config_not_from_a_per_kind_branch', () => {
    expect(getNodePorts('js', { output: 'json' }).outputs[0]?.emits).toBe('json');
    expect(getNodePorts('js', { output: 'raw' }).outputs[0]?.emits).toBe('raw');

    const merge = getNodePorts('merge', {
      inputs: [
        { input_id: 'a', handle: 'in:0', order: 0, activation: 'required' },
        { input_id: 'b', handle: 'in:1', order: 1, activation: 'optional' },
        { input_id: 'c', handle: 'in:2', order: 2, activation: 'optional' },
      ],
    });
    expect(merge.inputs.map((port) => port.id)).toEqual(['in:0', 'in:1', 'in:2']);

    const condition = getNodePorts('condition', { branches: ['alpha', 'beta'] });
    expect(condition.outputs.map((port) => port.id)).toEqual(['alpha', 'beta']);
  });

  it('uninstalled_capability_has_no_declaration_and_no_ports', () => {
    expect(nodeDescriptor('not-installed')).toBeUndefined();
    expect(getNodePorts('not-installed', {})).toEqual({ inputs: [], outputs: [] });
    // 未安装能力没有默认值：空 config 仍可展示、保存与 round-trip。
    expect(canonicalConfig('not-installed', null)).toEqual({});
  });

  it('a_new_capability_needs_only_a_descriptor_declaration', () => {
    // 这条声明模拟"新规则能力"：没有新的 React 分支、没有核心 enum 分支，
    // 只有一条 descriptor 声明。
    const declared: NodeDescriptor = {
      kind: 'importer',
      label_key: 'rules_node_inspector_type_http',
      description_key: 'rules_node_desc_http',
      icon: 'broadcast',
      inputs: [
        {
          handle: { source: 'fixed', handle: 'input' },
          value: { source: 'kind', kind: 'json' },
          role: 'data',
          side: 'left',
          label: { key: 'input', literal: true, numbered: false },
        },
      ],
      outputs: [
        {
          handle: { source: 'fixed', handle: 'output' },
          value: { source: 'kind', kind: 'json' },
          role: 'data',
          side: 'right',
          label: { key: 'json_output', literal: false, numbered: false },
        },
      ],
      fields: [
        {
          name: 'sources',
          label_key: 'rules_node_inspector_field_inputs',
          editor: {
            editor: 'string_list',
            item_label_key: 'rules_node_inspector_field_inputs',
            add_label_key: 'rules_node_inspector_identity_field_add',
            remove_label_key: 'rules_node_inspector_identity_field_remove',
            min_items: 0,
          },
          required: true,
        },
      ],
      default_config: { sources: [] },
    };

    registerNodeDescriptors([...ORIGINAL_DESCRIPTORS, declared]);

    expect(nodeDescriptor('importer')?.fields.map((field) => field.name)).toEqual(['sources']);
    expect(descriptorDefaultConfig('importer')).toEqual({ sources: [] });
    expect(getNodePorts('importer', { sources: [] }).outputs[0]?.emits).toBe('json');
    expect(getNodePorts('importer', { sources: [] }).inputs[0]?.side).toBe('left');
    expect(canonicalConfig('importer', null)).toEqual({ sources: [] });

    // 已安装能力不受影响。
    const [mapperInput] = getNodePorts('mapper', {}).inputs;
    if (mapperInput === undefined) throw new Error('mapper 缺少输入端口');
    expect(mapperInput.side).toBe('left');
    expect(
      portCompatible(
        { id: 'x', label: mapperInput.label, emits: 'json', side: 'right' },
        mapperInput,
      ),
    ).toBe(true);
  });

  it('resolved_variant_ports_change_label_with_the_config_value', () => {
    const descriptor = nodeDescriptor('js');
    if (descriptor === undefined) throw new Error('js 缺少 descriptor');
    const json = resolveDescriptorPorts(descriptor, { output: 'json' });
    const raw = resolveDescriptorPorts(descriptor, { output: 'raw' });
    expect(json.outputs[0]?.labelKey).not.toBe(raw.outputs[0]?.labelKey);
  });

  it('code_field_presence_is_read_from_the_descriptor', () => {
    // 预览入口的问法是「这个节点有没有作者手写源码」，答案只能来自声明。
    for (const kind of BUILT_IN_KINDS) {
      const descriptor = nodeDescriptor(kind);
      if (descriptor === undefined) throw new Error(`未安装能力缺少 descriptor: ${kind}`);
      const expected = descriptor.fields.some((field) => field.editor.editor === 'code');
      expect(descriptorHasCodeField(descriptor)).toBe(expected);
    }
    expect(descriptorHasCodeField(requireDescriptor('js'))).toBe(true);
    expect(descriptorHasCodeField(requireDescriptor('mapper'))).toBe(false);
    // 未安装能力没有声明，因此没有声明可读的源码字段。
    expect(descriptorHasCodeField(unknownDescriptor)).toBe(false);
  });
});
