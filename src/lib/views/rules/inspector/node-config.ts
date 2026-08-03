//! 节点 Inspector 的 canonical config 辅助。

import type { FlowNodeKind } from '$lib/rules/native-authoring/wire';

export type JsonObject = Record<string, unknown>;

export type ConditionPredicate = {
  operator: string;
  pointer: string;
  value?: unknown;
};

export type ConditionExpression =
  | {
      mode: 'typed';
      predicate: ConditionPredicate;
      true_branch: string;
      false_branch: string;
    }
  | { mode: 'js'; code: string };

export type ConditionExpressionPatch = {
  mode?: 'typed' | 'js';
  predicate?: ConditionPredicate;
  true_branch?: string;
  false_branch?: string;
  code?: string;
  branches?: string[];
};

export type CollectionSelector = { mode: 'typed'; pointer: string } | { mode: 'js'; code: string };

export type MergeInputValue = {
  input_id: string;
  handle: string;
  order: number;
  activation: 'required' | 'optional';
};

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
  return JSON.parse(JSON.stringify(value)) as T;
}

/** 空白节点也展示可编辑的 canonical 初始字段。 */
export function canonicalConfig(kind: FlowNodeKind, config: JsonObject | null): JsonObject {
  return { ...clone(DEFAULTS[kind]), ...(config ?? {}) };
}

export function stringValue(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback;
}

export function numberValue(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

export function recordValue(value: unknown): Record<string, string> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return {};
  return Object.fromEntries(
    Object.entries(value).filter(
      ([key, item]) => key.length > 0 && typeof item === 'string',
    ) as Array<[string, string]>,
  );
}

export function stringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : [];
}

export function mergeInputValues(value: unknown): MergeInputValue[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter((item): item is JsonObject => typeof item === 'object' && item !== null)
    .map((item, index) => ({
      input_id: stringValue(item.input_id, `input_${index + 1}`),
      handle: stringValue(item.handle, `in:${index}`),
      order: numberValue(item.order, index),
      activation: item.activation === 'optional' ? ('optional' as const) : ('required' as const),
    }))
    .sort((left, right) => left.order - right.order)
    .map((item, order) => ({ ...item, order }));
}

export function conditionExpression(config: JsonObject): ConditionExpression {
  const value = config.expression;
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return clone(DEFAULTS.condition.expression) as ConditionExpression;
  }
  const expression = value as JsonObject;
  if (expression.mode === 'js') {
    return { mode: 'js', code: stringValue(expression.code) };
  }
  const predicate = expression.predicate;
  return {
    mode: 'typed',
    predicate:
      typeof predicate === 'object' && predicate !== null && !Array.isArray(predicate)
        ? {
            operator: stringValue((predicate as JsonObject).operator, 'exists'),
            pointer: stringValue((predicate as JsonObject).pointer),
            ...(Object.prototype.hasOwnProperty.call(predicate, 'value')
              ? { value: (predicate as JsonObject).value }
              : {}),
          }
        : { operator: 'exists', pointer: '' },
    true_branch: stringValue(expression.true_branch, stringArray(config.branches)[0] ?? 'true'),
    false_branch: stringValue(expression.false_branch, stringArray(config.branches)[1] ?? 'false'),
  };
}

export function collectionSelector(config: JsonObject): CollectionSelector {
  const value = config.collection;
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return { mode: 'typed', pointer: '' };
  }
  const selector = value as JsonObject;
  return selector.mode === 'js'
    ? { mode: 'js', code: stringValue(selector.code) }
    : { mode: 'typed', pointer: stringValue(selector.pointer) };
}

export function literalText(value: unknown): string {
  if (value === undefined) return '';
  if (typeof value === 'string') return value;
  return JSON.stringify(value);
}

export function parseLiteral(value: string): unknown {
  const trimmed = value.trim();
  if (!trimmed) return '';
  try {
    return JSON.parse(trimmed) as unknown;
  } catch {
    return value;
  }
}

export function conditionPatch(config: JsonObject, patch: ConditionExpressionPatch): JsonObject {
  const current = conditionExpression(config);
  const expression =
    patch.mode === 'js'
      ? { mode: 'js', code: patch.code ?? '' }
      : {
          mode: 'typed',
          predicate:
            patch.predicate ??
            ('predicate' in current ? current.predicate : { operator: 'exists', pointer: '' }),
          true_branch:
            patch.true_branch ?? ('true_branch' in current ? current.true_branch : 'true'),
          false_branch:
            patch.false_branch ?? ('false_branch' in current ? current.false_branch : 'false'),
        };
  return { ...config, ...(patch.branches ? { branches: patch.branches } : {}), expression };
}

export function collectionPatch(config: JsonObject, selector: CollectionSelector): JsonObject {
  return { ...config, collection: selector };
}
