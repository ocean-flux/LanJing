import { describe, expect, it } from 'vitest';
import { parseNativeRuleDefinition } from './native-import';

const definition = {
  contract: 'rule_definition',
  schema_version: 1,
  source_identity: { id: 'native:imported' },
  base_url: 'https://example.test',
  intent_exports: {},
  flow: { nodes: [], edges: [] },
  capability_manifest: {
    required: { fs: false, env: false, process: false },
  },
  source_id_rules: [],
};

describe('parseNativeRuleDefinition', () => {
  it('accepts the current rule definition contract', () => {
    expect(parseNativeRuleDefinition(JSON.stringify(definition))).toEqual({
      ok: true,
      definition,
    });
  });

  it('rejects legacy or unrelated JSON before IPC', () => {
    expect(parseNativeRuleDefinition('{"url":"https://example.test"}')).toEqual({
      ok: false,
      issue: 'contract_mismatch',
    });
    expect(parseNativeRuleDefinition(JSON.stringify({ ...definition, extra: true }))).toEqual({
      ok: false,
      issue: 'unknown_root_field',
    });
  });

  it('rejects edges that refer to missing nodes', () => {
    expect(
      parseNativeRuleDefinition(
        JSON.stringify({
          ...definition,
          flow: {
            nodes: [],
            edges: [
              {
                from: { node_id: 'missing', handle: 'output' },
                to: { node_id: 'missing', handle: 'input' },
              },
            ],
          },
        }),
      ),
    ).toEqual({ ok: false, issue: 'invalid_flow' });
  });
});
