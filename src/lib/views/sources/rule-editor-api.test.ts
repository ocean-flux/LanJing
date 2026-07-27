import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { InstallCandidate } from '$lib/stores/rules.svelte';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

import {
  clearSourceDocumentCredential,
  createSourceDocument,
  deleteSourceDocument,
  getSourceDocument,
  listSourceDocuments,
  installSourceCandidate,
  pinSourceDocumentRevision,
  prepareInstallFromDocument,
  rebaseSourceDocument,
  releaseSourceDocumentRevisionPin,
  renameSourceDocument,
  replaceSourceDocumentCredential,
  revealSourceDocumentCredential,
  saveSourceDocument,
  type DocumentMutationOutcome,
  type RebaseSourceDocumentRequest,
  type SourceDocumentRevisionPin,
  type MaskedSourceDocument,
  type SourceDocumentCredentialTarget,
  type SourceDocumentSummary,
} from './rule-editor-api';

const summary: SourceDocumentSummary = {
  document_id: '00000000-0000-4000-8000-000000000001',
  format: 'legado',
  title: '示例来源',
  state: 'draft',
  source_identity: null,
  revision: 3,
  installed_revision: null,
  masked_hash: 'masked-hash',
  credential_slot_count: 1,
  schema_version: 1,
  created_at_ms: 100,
  updated_at_ms: 200,
};

const target: SourceDocumentCredentialTarget = {
  document_id: summary.document_id,
  document_revision: summary.revision,
  slot_id: '00000000-0000-4000-8000-000000000002',
};

const documentRef = {
  document_id: summary.document_id,
  document_revision: summary.revision,
};

const revisionPin: SourceDocumentRevisionPin = {
  pin_id: '00000000-0000-4000-8000-000000000004',
  document_id: documentRef.document_id,
  document_revision: documentRef.document_revision,
  expires_at_ms: 20_000,
};

const maskedDocument: MaskedSourceDocument = {
  summary,
  masked_text: '{"header":"__LANJING_CREDENTIAL_SLOT_V1__:00000000-0000-4000-8000-000000000002"}',
  credential_slots: [
    {
      slot_id: target.slot_id,
      path: '/header',
      name: 'header',
      has_value: true,
    },
  ],
};

const savedOutcome = {
  status: 'saved',
  document: maskedDocument,
} satisfies DocumentMutationOutcome;

const candidate: InstallCandidate = {
  id: '00000000-0000-4000-8000-000000000003',
  document_ref: documentRef,
  transient: false,
  expected_installed_revision: 0,
  profile: {
    id: 'source:example',
    title: '示例来源',
    icon_url: null,
    version: '1',
    group: null,
    supported_intents: ['Search'],
    risk_notes: [],
  },
  required_grant: {
    network: true,
    system: { fs: false, env: false, process: false },
  },
  diagnostics: [],
  definition_hash: 'definition-hash',
  plan_hash: 'plan-hash',
  expires_at_ms: 10_000,
};

describe('source document RuleSystem wire', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('wraps list, create, and masked get requests exactly', async () => {
    const createRequest = {
      format: 'legado' as const,
      title: '示例来源',
      text: '{"bookSourceName":"示例来源"}',
    };
    const getRequest = { document_id: summary.document_id };
    invoke
      .mockResolvedValueOnce([summary])
      .mockResolvedValueOnce(savedOutcome)
      .mockResolvedValueOnce(maskedDocument);

    await expect(listSourceDocuments()).resolves.toEqual([summary]);
    await expect(createSourceDocument(createRequest)).resolves.toBe(savedOutcome);
    await expect(getSourceDocument(getRequest)).resolves.toBe(maskedDocument);

    expect(invoke).toHaveBeenNthCalledWith(1, 'list_source_documents', { request: {} });
    expect(invoke).toHaveBeenNthCalledWith(2, 'create_source_document', {
      request: createRequest,
    });
    expect(invoke).toHaveBeenNthCalledWith(3, 'get_source_document', {
      request: getRequest,
    });
    expect(maskedDocument).not.toHaveProperty('value');
    expect(maskedDocument.credential_slots[0]).toEqual({
      slot_id: target.slot_id,
      path: '/header',
      name: 'header',
      has_value: true,
    });
  });

  it('forwards save, rename, and delete outcomes without replacing conflict current', async () => {
    const saveRequest = {
      document_id: summary.document_id,
      expected_revision: 3,
      masked_text: maskedDocument.masked_text,
    };
    const renameRequest = {
      document_id: summary.document_id,
      expected_revision: 3,
      title: '新标题',
    };
    const deleteRequest = {
      document_id: summary.document_id,
      expected_revision: 3,
    };
    const conflict = {
      status: 'conflict',
      expected_revision: 3,
      actual_revision: 4,
      current: maskedDocument,
    } satisfies DocumentMutationOutcome;
    const deleted = { status: 'saved', document: null } satisfies DocumentMutationOutcome;
    invoke
      .mockResolvedValueOnce(savedOutcome)
      .mockResolvedValueOnce(conflict)
      .mockResolvedValueOnce(deleted);

    await expect(saveSourceDocument(saveRequest)).resolves.toBe(savedOutcome);
    await expect(renameSourceDocument(renameRequest)).resolves.toBe(conflict);
    await expect(deleteSourceDocument(deleteRequest)).resolves.toBe(deleted);

    expect(invoke).toHaveBeenNthCalledWith(1, 'save_source_document', {
      request: saveRequest,
    });
    expect(invoke).toHaveBeenNthCalledWith(2, 'rename_source_document', {
      request: renameRequest,
    });
    expect(invoke).toHaveBeenNthCalledWith(3, 'delete_source_document', {
      request: deleteRequest,
    });
    expect(conflict).toEqual({
      status: 'conflict',
      expected_revision: 3,
      actual_revision: 4,
      current: maskedDocument,
    });
    expect(conflict.current).not.toHaveProperty('value');
  });

  it('passes every safe tagged mutation failure through without parsing strings', async () => {
    const request = {
      document_id: summary.document_id,
      expected_revision: 3,
      masked_text: maskedDocument.masked_text,
    };
    const outcomes: DocumentMutationOutcome[] = [
      { status: 'not_found' },
      { status: 'locked' },
      { status: 'key_unavailable' },
      { status: 'key_lost' },
      { status: 'corrupt' },
      {
        status: 'invalid',
        issues: [
          {
            code: 'document_too_large',
            message: '文档超过限制',
            path: null,
            byte_offset: null,
            byte_length: null,
          },
        ],
      },
    ];
    outcomes.forEach((outcome) => invoke.mockResolvedValueOnce(outcome));

    for (const outcome of outcomes) {
      await expect(saveSourceDocument(request)).resolves.toBe(outcome);
    }

    expect(invoke).toHaveBeenCalledTimes(outcomes.length);
    for (const call of invoke.mock.calls) {
      expect(call).toEqual(['save_source_document', { request }]);
    }
  });

  it('keeps slot ownership nested and invokes reveal every time instead of caching plaintext', async () => {
    const firstReveal = { target, value: 'one-time-value-1' };
    const secondReveal = { target, value: 'one-time-value-2' };
    invoke.mockResolvedValueOnce(firstReveal).mockResolvedValueOnce(secondReveal);

    await expect(revealSourceDocumentCredential({ target })).resolves.toBe(firstReveal);
    await expect(revealSourceDocumentCredential({ target })).resolves.toBe(secondReveal);

    expect(invoke).toHaveBeenCalledTimes(2);
    expect(invoke).toHaveBeenNthCalledWith(1, 'reveal_source_document_credential', {
      request: { target },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, 'reveal_source_document_credential', {
      request: { target },
    });

    invoke.mockResolvedValueOnce(savedOutcome).mockResolvedValueOnce(savedOutcome);
    await expect(
      replaceSourceDocumentCredential({ target, value: 'replacement-value' }),
    ).resolves.toBe(savedOutcome);
    await expect(clearSourceDocumentCredential({ target })).resolves.toBe(savedOutcome);

    expect(invoke).toHaveBeenNthCalledWith(3, 'replace_source_document_credential', {
      request: { target, value: 'replacement-value' },
    });
    expect(invoke).toHaveBeenNthCalledWith(4, 'clear_source_document_credential', {
      request: { target },
    });
  });

  it('prepares only the exact saved document reference', async () => {
    invoke.mockResolvedValue(candidate);

    const result = await prepareInstallFromDocument(documentRef);
    expect(result).toBe(candidate);
    expect(result).toMatchObject({
      document_ref: documentRef,
      transient: false,
      expected_installed_revision: 0,
    });
    expect(invoke).toHaveBeenCalledWith('prepare_install_from_document', {
      request: documentRef,
    });
  });

  it('pins, releases, rebases, and installs through the exact frozen commands', async () => {
    const released = { status: 'released' as const, pin_id: revisionPin.pin_id };
    const rebaseRequest: RebaseSourceDocumentRequest = {
      pin_id: revisionPin.pin_id,
      document_id: summary.document_id,
      base_revision: 3,
      current_revision: 4,
      local_masked_text: maskedDocument.masked_text,
      mode: { kind: 'fork', title: '冲突副本' },
      credential_resolutions: [
        { path: '/header', action: { kind: 'replace', value: 'one-shot-replacement' } },
      ],
    };
    const installed = {
      source_id: 'source:example',
      version: '1',
      profile: candidate.profile,
      revision: 8,
      document_ref: documentRef,
    };
    invoke
      .mockResolvedValueOnce(revisionPin)
      .mockResolvedValueOnce(released)
      .mockResolvedValueOnce(released)
      .mockResolvedValueOnce(savedOutcome)
      .mockResolvedValueOnce(installed);

    await expect(pinSourceDocumentRevision(documentRef)).resolves.toBe(revisionPin);
    await expect(releaseSourceDocumentRevisionPin({ pin_id: revisionPin.pin_id })).resolves.toBe(
      released,
    );
    await expect(releaseSourceDocumentRevisionPin({ pin_id: revisionPin.pin_id })).resolves.toBe(
      released,
    );
    await expect(rebaseSourceDocument(rebaseRequest)).resolves.toBe(savedOutcome);
    await expect(
      installSourceCandidate({ candidate_id: candidate.id, grant: 'network_only' }),
    ).resolves.toBe(installed);

    expect(invoke).toHaveBeenNthCalledWith(1, 'pin_source_document_revision', {
      request: documentRef,
    });
    expect(invoke).toHaveBeenNthCalledWith(2, 'release_source_document_revision_pin', {
      request: { pin_id: revisionPin.pin_id },
    });
    expect(invoke).toHaveBeenNthCalledWith(3, 'release_source_document_revision_pin', {
      request: { pin_id: revisionPin.pin_id },
    });
    expect(invoke).toHaveBeenNthCalledWith(4, 'rebase_source_document', {
      request: rebaseRequest,
    });
    expect(invoke).toHaveBeenNthCalledWith(5, 'install', {
      request: { candidate_id: candidate.id, grant: 'network_only' },
    });
    expect(installed.document_ref).toEqual(documentRef);
  });

  it('preserves invoke rejections instead of logging or converting them', async () => {
    const failure = { code: 'revision_pin_revision_mismatch' };
    invoke.mockRejectedValue(failure);

    await expect(getSourceDocument({ document_id: summary.document_id })).rejects.toBe(failure);
    await expect(pinSourceDocumentRevision(documentRef)).rejects.toBe(failure);
    expect(invoke).toHaveBeenLastCalledWith('pin_source_document_revision', {
      request: documentRef,
    });
  });
});
