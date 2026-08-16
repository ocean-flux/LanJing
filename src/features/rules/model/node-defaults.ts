import type { FlowNodeKind } from '@/shared/tauri/rules';

type JsonObject = Record<string, unknown>;

const DEFAULTS: Record<FlowNodeKind, JsonObject> = {
  http: {
    method: 'Get',
    url: '',
    headers: {},
    body: null,
    charset: null,
    expected_type: 'Html',
  },
  js: { code: '', output: 'json' },
  extract: { rules: [], field_rules: {}, expected_type: 'Html', output_target: 'Media' },
  mapper: { output: 'items', identity_fields: [] },
  merge: {
    inputs: [
      { input_id: 'input_1', handle: 'in:0', order: 0, activation: 'required' },
      { input_id: 'input_2', handle: 'in:1', order: 1, activation: 'optional' },
    ],
    strategy: 'single_active',
  },
  condition: {
    branches: ['true', 'false'],
    expression: {
      mode: 'typed',
      predicate: { operator: 'exists', pointer: '' },
      true_branch: 'true',
      false_branch: 'false',
    },
  },
  loop: {
    collection: { mode: 'typed', pointer: '' },
    item_binding: 'item',
    index_binding: 'index',
    max_iterations: 64,
  },
};

function clone<T>(value: T): T {
  try {
    return structuredClone(value);
  } catch {
    throw new Error('无法复制节点配置');
  }
}

/** 为新节点补齐 current Rust closed config 的最小字段。 */
export function canonicalConfig(kind: FlowNodeKind, config: JsonObject | null): JsonObject {
  return { ...clone(DEFAULTS[kind]), ...config };
}
