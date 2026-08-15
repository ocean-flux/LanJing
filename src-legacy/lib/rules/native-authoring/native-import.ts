//! 原生规则 JSON 导入预检：只接受 current RuleDefinition 合同。

import type { RuleDefinition } from './wire';

export const MAX_NATIVE_RULE_IMPORT_BYTES = 5 * 1024 * 1024;

export type NativeRuleImportIssue =
  | 'file_too_large'
  | 'invalid_json'
  | 'root_not_object'
  | 'contract_mismatch'
  | 'schema_unsupported'
  | 'unknown_root_field'
  | 'invalid_source_identity'
  | 'invalid_intent_exports'
  | 'invalid_flow'
  | 'invalid_capability_manifest'
  | 'invalid_source_id_rules';

export type NativeRuleImportResult =
  | { ok: true; definition: RuleDefinition }
  | { ok: false; issue: NativeRuleImportIssue };

type JsonObject = Record<string, unknown>;

const NODE_KINDS = new Set(['http', 'js', 'extract', 'mapper', 'merge', 'condition', 'loop']);
const STANDARD_INTENTS = new Set([
  'Search',
  'Discover',
  'ResolveItem',
  'ListUnits',
  'ResolveAsset',
  'ContinueAction',
]);
const ROOT_FIELDS = new Set([
  'contract',
  'schema_version',
  'source_identity',
  'base_url',
  'intent_exports',
  'flow',
  'capability_manifest',
  'source_id_rules',
]);

function isObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasString(value: JsonObject, key: string): boolean {
  return typeof value[key] === 'string' && value[key].trim().length > 0;
}

function hasBoolean(value: JsonObject, key: string): boolean {
  return typeof value[key] === 'boolean';
}

function validIntentExports(value: unknown): boolean {
  if (!isObject(value)) return false;
  return Object.entries(value).every(([intent, entry]) => {
    if (!STANDARD_INTENTS.has(intent) || !isObject(entry)) return false;
    return hasString(entry, 'flow_entry') && hasString(entry, 'mapper_output');
  });
}

function validFlow(value: unknown): boolean {
  if (!isObject(value) || !Array.isArray(value.nodes) || !Array.isArray(value.edges)) return false;

  const nodeIds = new Set<string>();
  for (const node of value.nodes) {
    if (!isObject(node) || !hasString(node, 'id') || !isObject(node.config)) return false;
    if (nodeIds.has(node.id as string)) return false;
    if (!NODE_KINDS.has(String(node.config.kind)) || !isObject(node.config.value)) return false;
    nodeIds.add(node.id as string);
  }

  return value.edges.every((edge) => {
    if (!isObject(edge) || !isObject(edge.from) || !isObject(edge.to)) return false;
    if (!hasString(edge.from, 'node_id') || !hasString(edge.from, 'handle')) return false;
    if (!hasString(edge.to, 'node_id') || !hasString(edge.to, 'handle')) return false;
    return nodeIds.has(edge.from.node_id as string) && nodeIds.has(edge.to.node_id as string);
  });
}

function validCapabilityManifest(value: unknown): boolean {
  if (!isObject(value) || !isObject(value.required) || !isObject(value.required.system)) {
    return false;
  }
  return (
    hasBoolean(value.required, 'network') &&
    hasBoolean(value.required.system, 'fs') &&
    hasBoolean(value.required.system, 'env') &&
    hasBoolean(value.required.system, 'process')
  );
}

export function parseNativeRuleDefinition(text: string): NativeRuleImportResult {
  let value: unknown;
  try {
    value = JSON.parse(text) as unknown;
  } catch {
    return { ok: false, issue: 'invalid_json' };
  }

  if (!isObject(value)) return { ok: false, issue: 'root_not_object' };
  if (value.contract !== 'rule_definition') {
    return { ok: false, issue: 'contract_mismatch' };
  }
  if (value.schema_version !== 1) {
    return { ok: false, issue: 'schema_unsupported' };
  }
  if (Object.keys(value).some((key) => !ROOT_FIELDS.has(key))) {
    return { ok: false, issue: 'unknown_root_field' };
  }
  if (!isObject(value.source_identity) || !hasString(value.source_identity, 'id')) {
    return { ok: false, issue: 'invalid_source_identity' };
  }
  if (typeof value.base_url !== 'string' || !validIntentExports(value.intent_exports)) {
    return { ok: false, issue: 'invalid_intent_exports' };
  }
  if (!validFlow(value.flow)) return { ok: false, issue: 'invalid_flow' };
  if (!validCapabilityManifest(value.capability_manifest)) {
    return { ok: false, issue: 'invalid_capability_manifest' };
  }
  if (
    !Array.isArray(value.source_id_rules) ||
    value.source_id_rules.some((rule) => typeof rule !== 'string')
  ) {
    return { ok: false, issue: 'invalid_source_id_rules' };
  }

  return { ok: true, definition: value as unknown as RuleDefinition };
}
