import { describe, expect, it } from 'vitest';
import { createSourceWorkflow, type SourceWorkflowAdapter } from '@/features/sources/workflow';
import type { InstallCandidate, InstalledSource, SourceOperation } from '@/shared/tauri/sources';

const NO_GRANT: InstallCandidate['required_grant'] = {
  env: false,
  fs: false,
  process: false,
};

function installedSource(sourceId = 'source:legado:one'): InstalledSource {
  return {
    source_id: sourceId,
    version: 'v1',
    revision: 4,
    grant: NO_GRANT,
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
  operation: SourceOperation = 'install',
  grant: InstallCandidate['required_grant'] = NO_GRANT,
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
    required_grant: grant,
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
          return candidate('source:maccms:example');
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

  it('parses pasted JSON locally without a prepare call', async () => {
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
        prepare: async () => candidate(undefined, 'update'),
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

  it('marks an existing identity as an update and returns to pick after a stale install', async () => {
    const workflow = createSourceWorkflow(
      adapter({
        prepare: async () => candidate(undefined, 'update'),
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

    expect(workflow.getState()).toMatchObject({
      phase: 'confirm',
      prepared: [{ isUpdate: true }],
    });
    await workflow.install();

    expect(workflow.getState()).toMatchObject({
      phase: 'pick',
      rawInput: '{"bookSourceName":"本地来源"}',
      prepared: [],
      errorCode: 'candidate_stale',
    });
  });

  it('installs a candidate with its id only and no grant parameter', async () => {
    const installCalls: string[] = [];
    const workflow = createSourceWorkflow(
      adapter({
        install: async (candidateId) => {
          installCalls.push(candidateId);
          return installedSource();
        },
      }),
    );
    workflow.setInput(JSON.stringify({ bookSourceName: '本地来源' }));
    await workflow.prepareInput();
    await workflow.prepareSelected();

    await workflow.install();

    expect(installCalls).toEqual(['candidate:source:legado:one']);
    expect(workflow.getState()).toMatchObject({ phase: 'done', errorCode: null });
  });

  it('refuses a candidate that requires system capability', async () => {
    let installed = false;
    const workflow = createSourceWorkflow(
      adapter({
        prepare: async () =>
          candidate(undefined, 'install', { env: false, fs: true, process: false }),
        install: async () => {
          installed = true;
          return installedSource();
        },
      }),
    );
    workflow.setInput(JSON.stringify({ bookSourceName: '系统能力来源' }));
    await workflow.prepareInput();
    await workflow.prepareSelected();

    await workflow.install();

    expect(installed).toBe(false);
    expect(workflow.getState()).toMatchObject({
      phase: 'confirm',
      errorCode: 'system_grant_unsupported',
      errorDetail: null,
    });
  });
});
