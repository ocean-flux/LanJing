import { describe, expect, it } from 'vitest';
import { createSourceWorkflow, type SourceWorkflowAdapter } from '@/features/sources/workflow';
import type { InstallCandidate, InstalledSource, SourceOperation } from '@/shared/tauri/sources';

function installedSource(sourceId = 'source:legado:one'): InstalledSource {
  return {
    source_id: sourceId,
    version: 'v1',
    revision: 4,
    grant: { network: false, system: { env: false, fs: false, process: false } },
    profile: {
      id: sourceId,
      title: '已有来源',
      icon_url: null,
      version: 'v1',
      group: '小说',
      supported_intents: ['Search'],
      risk_notes: [],
    },
  };
}

function candidate(
  sourceId = 'source:legado:one',
  network = false,
  operation: SourceOperation = 'install',
): InstallCandidate {
  return {
    id: `candidate:${sourceId}`,
    expected_installed_revision: operation === 'update' ? 4 : 0,
    operation,
    profile: {
      id: sourceId,
      title: '待安装来源',
      icon_url: null,
      version: 'v2',
      group: '小说',
      supported_intents: ['Search'],
      risk_notes: [],
    },
    required_grant: { network, system: { env: false, fs: false, process: false } },
    diagnostics: [],
    definition_hash: 'definition-hash',
    plan_hash: 'plan-hash',
    expires_at_ms: 4000,
  };
}

function adapter(overrides: Partial<SourceWorkflowAdapter> = {}): SourceWorkflowAdapter {
  return {
    listSources: async () => [installedSource()],
    prepare: async () => candidate(),
    install: async () => installedSource(),
    ...overrides,
  };
}

describe('source workflow', () => {
  it('sends an HTTP input directly to the Maccms prepare contract', async () => {
    const requests: unknown[] = [];
    const workflow = createSourceWorkflow(
      adapter({
        prepare: async (request) => {
          requests.push(request);
          return candidate('source:maccms:example', true);
        },
      }),
    );
    workflow.setInput('https://example.test/api.php/provide/vod/');

    await workflow.prepareInput();

    expect(requests).toEqual([
      { kind: 'maccms_json', url: 'https://example.test/api.php/provide/vod/' },
    ]);
    expect(workflow.getState()).toMatchObject({
      phase: 'confirm',
      prepared: [{ isUpdate: false }],
    });
  });

  it('parses pasted JSON locally without a network or prepare call', async () => {
    let prepared = false;
    const workflow = createSourceWorkflow(
      adapter({
        prepare: async () => {
          prepared = true;
          return candidate();
        },
      }),
    );
    workflow.setInput(JSON.stringify({ bookSourceName: '本地来源', bookSourceGroup: '测试' }));

    await workflow.prepareInput();

    expect(prepared).toBe(false);
    expect(workflow.getState()).toMatchObject({
      phase: 'pick',
      catalog: [{ name: '本地来源', group: '测试' }],
      selectedIds: ['catalog:0'],
    });
  });

  it('reports an unrecognized input with a stable code', async () => {
    let prepared = false;
    const workflow = createSourceWorkflow(
      adapter({
        prepare: async () => {
          prepared = true;
          return candidate();
        },
      }),
    );
    workflow.setInput('plain text that is neither JSON nor a URL');

    await workflow.prepareInput();

    expect(prepared).toBe(false);
    expect(workflow.getState()).toMatchObject({ phase: 'error', errorCode: 'input_unrecognized' });
  });

  it('trusts the declared update operation when the installed list misses the source', async () => {
    const workflow = createSourceWorkflow(
      adapter({
        listSources: async () => [],
        prepare: async () => candidate(undefined, false, 'update'),
      }),
    );
    await workflow.refreshSources();
    workflow.setInput('https://example.test/api.php/provide/vod/');

    await workflow.prepareInput();

    expect(workflow.getState()).toMatchObject({
      phase: 'confirm',
      prepared: [{ isUpdate: true, previous: null }],
    });
  });

  it('marks an existing identity as an update and resets grant after stale install', async () => {
    const workflow = createSourceWorkflow(
      adapter({
        prepare: async () => candidate(undefined, true, 'update'),
        install: async () => {
          const stale = new Error('stale') as Error & { code: string };
          stale.code = 'candidate_stale';
          throw stale;
        },
      }),
    );
    await workflow.refreshSources();
    workflow.setInput(JSON.stringify({ bookSourceName: '本地来源' }));
    await workflow.prepareInput();
    await workflow.prepareSelected();
    workflow.setAllowNetwork(true);

    expect(workflow.getState()).toMatchObject({
      phase: 'confirm',
      prepared: [{ isUpdate: true }],
    });
    await workflow.install();

    expect(workflow.getState()).toMatchObject({
      phase: 'pick',
      rawInput: '{"bookSourceName":"本地来源"}',
      allowNetwork: false,
      prepared: [],
      errorCode: 'candidate_stale',
    });
  });
});
