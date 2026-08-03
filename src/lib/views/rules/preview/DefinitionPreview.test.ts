import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import DefinitionPreview from './DefinitionPreview.svelte';

vi.mock('$lib/rules/native-authoring/wire', () => ({
  getNativeRuleDocument: vi.fn(),
  getNativeRuleProvenance: vi.fn(async () => null),
}));

function makeSession() {
  return {
    documentId: 'doc:1',
    definition: {
      contract: 'rule_definition',
      schema_version: 1,
      source_identity: 'source:test',
      base_url: 'https://example.test',
      intent_exports: {},
      flow: { nodes: [], edges: [] },
      capability_manifest: {
        required: { network: true, system: { fs: false, env: false, process: false } },
      },
      source_id_rules: [],
    },
  } as never;
}

describe('DefinitionPreview', () => {
  it('renders masked definition with no-secret hint', () => {
    render(DefinitionPreview, { props: { session: makeSession() } });
    expect(screen.getByText('规则定义预览')).toBeTruthy();
    expect(screen.getByText('凭证值已省略')).toBeTruthy();
  });
});
