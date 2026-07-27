import { describe, expect, it, vi } from 'vitest';
import type { InstallCandidate, InstalledSource } from './rules.svelte';
import {
  RuleEditorSession,
  type RuleEditorEditorHostFactory,
  type RuleEditorSessionApi,
} from './rule-editor-session.svelte';
import type {
  DocumentMutationOutcome,
  DocumentRef,
  MaskedSourceDocument,
  SourceDocumentRevisionPin,
  SourceDocumentSummary,
} from '$lib/views/sources/rule-editor-api';
import type { SourceDocumentEditorHost } from '$lib/views/sources/source-document-editor-host';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function summary(documentId: string, revision: number): SourceDocumentSummary {
  return {
    document_id: documentId,
    format: 'legado',
    title: `Document ${documentId}`,
    state: 'draft',
    source_identity: null,
    revision,
    installed_revision: null,
    masked_hash: `masked-${documentId}-${revision}`,
    credential_slot_count: 0,
    schema_version: 1,
    created_at_ms: 100,
    updated_at_ms: 200 + revision,
  };
}

function document(
  documentId: string,
  revision: number,
  maskedText: string,
  slots: MaskedSourceDocument['credential_slots'] = [],
): MaskedSourceDocument {
  return {
    summary: {
      ...summary(documentId, revision),
      credential_slot_count: slots.length,
    },
    masked_text: maskedText,
    credential_slots: slots,
  };
}

function pin(
  reference: DocumentRef,
  sequence = reference.document_revision,
): SourceDocumentRevisionPin {
  return {
    pin_id: `pin-${reference.document_id}-${sequence}`,
    document_id: reference.document_id,
    document_revision: reference.document_revision,
    expires_at_ms: 100_000 + sequence,
  };
}

const profile = {
  id: 'source:one',
  title: 'Example',
  icon_url: null,
  version: '1',
  group: null,
  supported_intents: ['Search' as const],
  risk_notes: [],
};

function candidate(reference: DocumentRef, expectedInstalledRevision = 0): InstallCandidate {
  return {
    id: `candidate-${reference.document_revision}`,
    document_ref: reference,
    transient: false,
    expected_installed_revision: expectedInstalledRevision,
    profile,
    required_grant: {
      network: true,
      system: { fs: false, env: false, process: false },
    },
    diagnostics: [],
    definition_hash: 'definition-hash',
    plan_hash: 'plan-hash',
    expires_at_ms: 100_000,
  };
}

function installed(reference: DocumentRef, revision = 1): InstalledSource {
  return {
    source_id: 'source:one',
    version: '1',
    profile,
    revision,
    document_ref: reference,
  };
}

interface ResolvedValueOnceMock<Value> {
  mockResolvedValueOnce(value: Value): unknown;
}

interface RuleEditorSessionApiFixture {
  readonly getDocument: ResolvedValueOnceMock<MaskedSourceDocument | null>;
  readonly saveDocument: ResolvedValueOnceMock<DocumentMutationOutcome>;
}

function createApi() {
  let pinSequence = 0;
  const listDocuments = vi.fn(async () => [] as SourceDocumentSummary[]);
  const createDocument = vi.fn(async () => ({ status: 'not_found' }) as DocumentMutationOutcome);
  const getDocument = vi.fn(async () => null as MaskedSourceDocument | null);
  const saveDocument = vi.fn(async () => ({ status: 'not_found' }) as DocumentMutationOutcome);
  const renameDocument = vi.fn(async () => ({ status: 'not_found' }) as DocumentMutationOutcome);
  const deleteDocument = vi.fn(async () => ({ status: 'not_found' }) as DocumentMutationOutcome);
  const revealCredential = vi.fn(
    async (request: Parameters<RuleEditorSessionApi['revealCredential']>[0]) => ({
      target: request.target,
      value: 'revealed-value',
    }),
  );
  const replaceCredential = vi.fn(async () => ({ status: 'not_found' }) as DocumentMutationOutcome);
  const clearCredential = vi.fn(async () => ({ status: 'not_found' }) as DocumentMutationOutcome);
  const pinRevision = vi.fn(async (reference: DocumentRef) => pin(reference, ++pinSequence));
  const releaseRevisionPin = vi.fn(async ({ pin_id }: { pin_id: string }) => ({
    status: 'released' as const,
    pin_id,
  }));
  const rebaseDocument = vi.fn(async () => ({ status: 'not_found' }) as DocumentMutationOutcome);
  const prepareInstall = vi.fn(async (reference: DocumentRef) => candidate(reference));
  const installCandidate = vi.fn(async () =>
    installed({ document_id: 'document-one', document_revision: 1 }),
  );
  const refreshInstalledSources = vi.fn(async () => undefined);

  return {
    listDocuments,
    createDocument,
    getDocument,
    saveDocument,
    renameDocument,
    deleteDocument,
    revealCredential,
    replaceCredential,
    clearCredential,
    pinRevision,
    releaseRevisionPin,
    rebaseDocument,
    prepareInstall,
    installCandidate,
    refreshInstalledSources,
  } satisfies RuleEditorSessionApi;
}

interface FakeHostSnapshot {
  readonly status: 'idle' | 'fallback';
}

interface FakeHostControl {
  readonly setDocument: ReturnType<typeof vi.fn>;
  readonly focus: ReturnType<typeof vi.fn>;
  readonly dispose: ReturnType<typeof vi.fn>;
  emitText(text: string): void;
  emitFallback(): void;
}

function createHostFactory(controls: FakeHostControl[]): RuleEditorEditorHostFactory {
  return (initialDocument, onTextEdit) => {
    let snapshot: FakeHostSnapshot = { status: 'idle' };
    const listeners: Array<(value: FakeHostSnapshot) => void> = [];
    const control: FakeHostControl = {
      setDocument: vi.fn(async () => undefined),
      focus: vi.fn(),
      dispose: vi.fn(),
      emitText: onTextEdit,
      emitFallback: () => {
        snapshot = { status: 'fallback' };
        for (const listener of listeners) listener(snapshot);
      },
    };
    const host = {
      getSnapshot: () => snapshot,
      subscribe: (listener: (value: FakeHostSnapshot) => void) => {
        listeners.push(listener);
        listener(snapshot);
        return () => {
          const index = listeners.indexOf(listener);
          if (index >= 0) listeners.splice(index, 1);
        };
      },
      setDocument: control.setDocument,
      focus: control.focus,
      dispose: control.dispose,
      initialDocument,
    };
    controls.push(control);
    return host as unknown as SourceDocumentEditorHost;
  };
}

async function open(
  session: RuleEditorSession,
  api: RuleEditorSessionApiFixture,
  maskedDocument: MaskedSourceDocument,
): Promise<void> {
  api.getDocument.mockResolvedValueOnce(maskedDocument);
  await expect(session.openDocument(maskedDocument.summary.document_id)).resolves.toMatchObject({
    status: 'opened',
  });
}

async function enterRevisionConflictWithReplacement(
  session: RuleEditorSession,
  api: RuleEditorSessionApiFixture,
  replacement: string,
): Promise<void> {
  const baseSlot = {
    slot_id: 'base-header',
    path: '/header',
    name: 'header',
    has_value: true,
  };
  await open(
    session,
    api,
    document('document-one', 3, '{"header":"masked","value":1}', [baseSlot]),
  );
  session.editText('{"header":"masked","value":2}');
  api.saveDocument.mockResolvedValueOnce({
    status: 'conflict',
    expected_revision: 3,
    actual_revision: 4,
    current: document('document-one', 4, '{"header":"masked","value":4}', [
      { slot_id: 'current-header', path: '/header', name: 'header', has_value: true },
    ]),
  });
  await expect(session.save()).resolves.toMatchObject({ status: 'conflict' });
  expect(
    session.setConflictCredentialResolution('/header', { kind: 'replace', value: replacement }),
  ).toBe(true);
}

describe('RuleEditorSession', () => {
  it('rotates credential sentinels across queued saves without losing in-flight edits', async () => {
    const api = createApi();
    const controls: FakeHostControl[] = [];
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory(controls) });
    const revisionThreeSentinel =
      '__LANJING_CREDENTIAL_SLOT_V1__:00000000-0000-4000-8000-000000000003';
    const revisionFourSentinel =
      '__LANJING_CREDENTIAL_SLOT_V1__:00000000-0000-4000-8000-000000000004';
    const revisionFiveSentinel =
      '__LANJING_CREDENTIAL_SLOT_V1__:00000000-0000-4000-8000-000000000005';
    await open(
      session,
      api,
      document(
        'document-one',
        3,
        `{"header":"${revisionThreeSentinel}",  "unknown":{"keep":true},"value":1}`,
        [
          {
            slot_id: '00000000-0000-4000-8000-000000000003',
            path: '/header',
            name: 'header',
            has_value: true,
          },
        ],
      ),
    );

    const firstResponse = deferred<DocumentMutationOutcome>();
    const secondResponse = deferred<DocumentMutationOutcome>();
    api.saveDocument
      .mockReturnValueOnce(firstResponse.promise)
      .mockReturnValueOnce(secondResponse.promise);

    session.editText(`{"header":"${revisionThreeSentinel}",  "unknown":{"keep":true},"value":2}`);
    const firstSave = session.save();
    await vi.waitFor(() => expect(api.saveDocument).toHaveBeenCalledTimes(1));
    expect(api.saveDocument).toHaveBeenNthCalledWith(1, {
      document_id: 'document-one',
      expected_revision: 3,
      masked_text: `{"header":"${revisionThreeSentinel}",  "unknown":{"keep":true},"value":2}`,
    });

    session.editText(`{"header":"${revisionThreeSentinel}",  "unknown":{"keep":true},"value":3}`);
    const secondSave = session.save();
    expect(api.saveDocument).toHaveBeenCalledTimes(1);
    expect(session.snapshot.save_phase).toBe('queued');

    firstResponse.resolve({
      status: 'saved',
      document: document(
        'document-one',
        4,
        `{"header":"${revisionFourSentinel}",  "unknown":{"keep":true},"value":2}`,
        [
          {
            slot_id: '00000000-0000-4000-8000-000000000004',
            path: '/header',
            name: 'header',
            has_value: true,
          },
        ],
      ),
    });
    await vi.waitFor(() => expect(api.saveDocument).toHaveBeenCalledTimes(2));
    expect(api.saveDocument).toHaveBeenNthCalledWith(2, {
      document_id: 'document-one',
      expected_revision: 4,
      masked_text: `{"header":"${revisionFourSentinel}",  "unknown":{"keep":true},"value":3}`,
    });

    session.editText(`{"header":"${revisionFourSentinel}",  "unknown":{"keep":true},"value":4}`);
    secondResponse.resolve({
      status: 'saved',
      document: document(
        'document-one',
        5,
        `{"header":"${revisionFiveSentinel}",  "unknown":{"keep":true},"value":3}`,
        [
          {
            slot_id: '00000000-0000-4000-8000-000000000005',
            path: '/header',
            name: 'header',
            has_value: true,
          },
        ],
      ),
    });

    await expect(firstSave).resolves.toMatchObject({ status: 'saved' });
    await expect(secondSave).resolves.toMatchObject({ status: 'saved' });
    expect(session.snapshot).toMatchObject({
      text: `{"header":"${revisionFiveSentinel}",  "unknown":{"keep":true},"value":4}`,
      saved_text: `{"header":"${revisionFiveSentinel}",  "unknown":{"keep":true},"value":3}`,
      saved_revision: 5,
      dirty: true,
      save_phase: 'idle',
      conflict: null,
    });
    expect(session.snapshot.text).not.toContain(revisionThreeSentinel);
    expect(session.snapshot.text).not.toContain(revisionFourSentinel);
    expect(api.pinRevision.mock.calls.map(([reference]) => reference.document_revision)).toEqual([
      3, 4, 5,
    ]);
    expect(api.releaseRevisionPin).toHaveBeenCalledTimes(2);
  });

  it('adopts the server masked baseline atomically when no edit follows the save intent', async () => {
    const api = createApi();
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory([]) });
    const previousSentinel = '__LANJING_CREDENTIAL_SLOT_V1__:00000000-0000-4000-8000-000000000006';
    const savedSentinel = '__LANJING_CREDENTIAL_SLOT_V1__:00000000-0000-4000-8000-000000000007';
    await open(
      session,
      api,
      document('document-one', 1, `{"header":"${previousSentinel}","value":1}`, [
        {
          slot_id: '00000000-0000-4000-8000-000000000006',
          path: '/header',
          name: 'header',
          has_value: true,
        },
      ]),
    );
    session.editText(`{"header":"${previousSentinel}","value":2}`);
    api.saveDocument.mockResolvedValueOnce({
      status: 'saved',
      document: document('document-one', 2, `{"header":"${savedSentinel}","value":2}`, [
        {
          slot_id: '00000000-0000-4000-8000-000000000007',
          path: '/header',
          name: 'header',
          has_value: true,
        },
      ]),
    });

    await expect(session.save()).resolves.toMatchObject({ status: 'saved' });
    expect(session.snapshot).toMatchObject({
      text: `{"header":"${savedSentinel}","value":2}`,
      saved_text: `{"header":"${savedSentinel}","value":2}`,
      saved_revision: 2,
      dirty: false,
      conflict: null,
      history: { can_undo: false, can_redo: false },
    });
    expect(session.snapshot.text).not.toContain(previousSentinel);
  });

  it('requires reload when an in-flight edit changes a credential token path', async () => {
    const api = createApi();
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory([]) });
    const previousSentinel = '__LANJING_CREDENTIAL_SLOT_V1__:00000000-0000-4000-8000-000000000011';
    const savedSentinel = '__LANJING_CREDENTIAL_SLOT_V1__:00000000-0000-4000-8000-000000000012';
    await open(
      session,
      api,
      document('document-one', 1, `{"header":"${previousSentinel}","value":1}`, [
        {
          slot_id: '00000000-0000-4000-8000-000000000011',
          path: '/header',
          name: 'header',
          has_value: true,
        },
      ]),
    );

    const response = deferred<DocumentMutationOutcome>();
    api.saveDocument.mockReturnValueOnce(response.promise);
    session.editText(`{"header":"${previousSentinel}","value":2}`);
    const firstSave = session.save();
    await vi.waitFor(() => expect(api.saveDocument).toHaveBeenCalledOnce());
    session.editText('{"header":"manually-changed","value":3}');
    const queuedSave = session.save();
    response.resolve({
      status: 'saved',
      document: document('document-one', 2, `{"header":"${savedSentinel}","value":2}`, [
        {
          slot_id: '00000000-0000-4000-8000-000000000012',
          path: '/header',
          name: 'header',
          has_value: true,
        },
      ]),
    });

    await expect(firstSave).resolves.toMatchObject({ status: 'saved' });
    await expect(queuedSave).resolves.toEqual({
      status: 'blocked',
      reason: 'document_snapshot_unavailable',
    });
    expect(api.saveDocument).toHaveBeenCalledOnce();
    expect(session.snapshot).toMatchObject({
      text: '{"header":"manually-changed","value":3}',
      saved_text: `{"header":"${savedSentinel}","value":2}`,
      saved_revision: 2,
      dirty: true,
      conflict: {
        kind: 'sentinel_rotation',
        reload_required: true,
      },
    });
    await expect(session.save()).resolves.toEqual({
      status: 'blocked',
      reason: 'conflict_unresolved',
    });
    await expect(session.prepare()).resolves.toEqual({
      status: 'blocked',
      reason: 'conflict_unresolved',
    });
    expect(session.continueEditingConflict()).toBe(false);
    await expect(session.mergeConflict()).resolves.toEqual({
      status: 'blocked',
      reason: 'document_snapshot_unavailable',
    });

    await expect(session.reloadConflict()).resolves.toMatchObject({ status: 'saved' });
    expect(session.snapshot.text).toBe(`{"header":"${savedSentinel}","value":2}`);
    expect(session.snapshot.dirty).toBe(false);
    expect(session.snapshot.conflict).toBeNull();
  });

  it('drops a stale prepare response and invalidates candidate and grant on typed edits', async () => {
    const api = createApi();
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory([]) });
    await open(
      session,
      api,
      document(
        'document-one',
        1,
        '{"bookSourceType":0,"bookSourceUrl":"https://example.com","bookSourceName":"Test","value":1,"unknown":true}',
      ),
    );

    const pendingCandidate = deferred<InstallCandidate>();
    api.prepareInstall.mockReturnValueOnce(pendingCandidate.promise);
    const pendingPrepare = session.prepare();
    session.applyPatch({
      operation: 'set',
      pointer: '/value',
      value: 2,
      expectedEpoch: session.snapshot.document_epoch,
    });
    pendingCandidate.resolve(candidate({ document_id: 'document-one', document_revision: 1 }));
    await expect(pendingPrepare).resolves.toEqual({ status: 'stale' });
    expect(session.snapshot.candidate).toBeNull();

    expect(session.undo()).toBe(true);
    await expect(session.prepare()).resolves.toMatchObject({ status: 'prepared' });
    session.grant = 'network_only';
    expect(session.snapshot.grant).toBe('network_only');

    const applied = session.applyPatch({
      operation: 'set',
      pointer: '/value',
      value: 3,
      expectedEpoch: session.snapshot.document_epoch,
    });
    expect(applied?.kind).toBe('applied');
    expect(session.snapshot.candidate).toBeNull();
    expect(session.snapshot.grant).toBe('none');
    expect(session.snapshot.prepare_diagnostics).toEqual([]);
  });

  it('shares exact text command history across original, form, and tree modes', async () => {
    const api = createApi();
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory([]) });
    const original =
      '{"bookSourceType":0,"bookSourceUrl":"https://example.com","bookSourceName":"Test","value":1,  "unknown":{"keep":true}}';
    await open(session, api, document('document-one', 1, original));

    expect(session.setMode('form')).toBe(true);
    const patch = session.applyPatch({
      operation: 'set',
      pointer: '/value',
      value: 2,
      expectedEpoch: session.snapshot.document_epoch,
    });
    expect(patch?.kind).toBe('applied');
    const patchedText = session.snapshot.text;
    expect(patchedText).toContain('"unknown":{"keep":true}');
    expect(patchedText).toContain('  "unknown"');

    expect(session.setMode('tree')).toBe(true);
    expect(session.undo()).toBe(true);
    expect(session.snapshot.text).toBe(original);
    expect(session.redo()).toBe(true);
    expect(session.snapshot.text).toBe(patchedText);
    expect(session.snapshot.history).toEqual({ can_undo: true, can_redo: false });

    const stale = session.applyPatch({
      operation: 'set',
      pointer: '/value',
      value: 4,
      expectedEpoch: 0,
    });
    expect(stale?.kind).toBe('rejected');
    expect(session.snapshot.text).toBe(patchedText);
  });

  it('filters endpoint documents instead of creating a Maccms workspace surface', async () => {
    const api = createApi();
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory([]) });
    api.listDocuments.mockResolvedValueOnce([
      summary('legado-document', 1),
      { ...summary('maccms-document', 1), format: 'maccms10_endpoint' },
    ]);

    await expect(session.refreshDocuments()).resolves.toEqual([summary('legado-document', 1)]);
    expect(session.snapshot.documents.map((entry) => entry.document_id)).toEqual([
      'legado-document',
    ]);
  });

  it('owns create, rename, and delete while preserving the single document pin', async () => {
    const api = createApi();
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory([]) });
    const created = document('document-created', 1, '{"value":1}');
    api.createDocument.mockResolvedValueOnce({ status: 'saved', document: created });

    await expect(session.createDocument('Created', '{"value":1}')).resolves.toMatchObject({
      status: 'saved',
    });
    expect(api.createDocument).toHaveBeenCalledWith({
      format: 'legado',
      title: 'Created',
      text: '{"value":1}',
    });
    expect(session.snapshot.document?.summary.document_id).toBe('document-created');

    const renamed = {
      ...created,
      summary: { ...created.summary, title: 'Renamed' },
    };
    api.renameDocument.mockResolvedValueOnce({ status: 'saved', document: renamed });
    await expect(session.renameCurrentDocument('Renamed')).resolves.toMatchObject({
      status: 'saved',
    });
    expect(api.renameDocument).toHaveBeenCalledWith({
      document_id: 'document-created',
      expected_revision: 1,
      title: 'Renamed',
    });
    expect(session.snapshot.document?.summary.title).toBe('Renamed');

    api.deleteDocument.mockResolvedValueOnce({ status: 'saved', document: null });
    await expect(session.deleteCurrentDocument()).resolves.toEqual({
      status: 'saved',
      document: null,
    });
    expect(api.releaseRevisionPin.mock.invocationCallOrder[0]).toBeLessThan(
      api.deleteDocument.mock.invocationCallOrder[0],
    );
    expect(session.snapshot.document).toBeNull();
    expect(session.editorHost).toBeNull();
  });

  it('normalizes a delete revision conflict to a reload-only snapshot state', async () => {
    const api = createApi();
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory([]) });
    await open(session, api, document('document-one', 1, '{"value":1}'));
    const current = document('document-one', 2, '{"value":2}');
    api.deleteDocument.mockResolvedValueOnce({
      status: 'conflict',
      expected_revision: 1,
      actual_revision: 2,
      current,
    });

    await expect(session.deleteCurrentDocument()).resolves.toMatchObject({ status: 'conflict' });
    expect(session.snapshot.conflict).toMatchObject({
      kind: 'snapshot_unavailable',
      reload_required: true,
      base_revision: 1,
      current_revision: 2,
      current,
    });
    expect(session.snapshot.pinned_revision).toBeNull();
    await expect(session.mergeConflict()).resolves.toEqual({
      status: 'blocked',
      reason: 'document_snapshot_unavailable',
    });
    await expect(session.forkConflict('Unsafe draft')).resolves.toEqual({
      status: 'blocked',
      reason: 'document_snapshot_unavailable',
    });
    expect(api.rebaseDocument).not.toHaveBeenCalled();

    await expect(session.reloadConflict()).resolves.toEqual({ status: 'saved', document: current });
    expect(session.snapshot.conflict).toBeNull();
    expect(session.snapshot.saved_revision).toBe(2);
  });

  it('releases pins on switch, successful save, and dispose', async () => {
    const api = createApi();
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory([]) });
    await open(session, api, document('document-one', 1, '{"value":1}'));
    await open(session, api, document('document-two', 4, '{"value":4}'));

    session.editText('{"value":5}');
    api.saveDocument.mockResolvedValueOnce({
      status: 'saved',
      document: document('document-two', 5, '{"value":5}'),
    });
    await expect(session.save()).resolves.toMatchObject({ status: 'saved' });
    await session.dispose();

    expect(api.pinRevision.mock.calls.map(([reference]) => reference)).toEqual([
      { document_id: 'document-one', document_revision: 1 },
      { document_id: 'document-two', document_revision: 4 },
      { document_id: 'document-two', document_revision: 5 },
    ]);
    expect(api.releaseRevisionPin.mock.calls.map(([request]) => request.pin_id)).toEqual([
      'pin-document-one-1',
      'pin-document-two-2',
      'pin-document-two-3',
    ]);
  });

  it('keeps revealed plaintext outside snapshots and clears it before fallback and switch', async () => {
    const api = createApi();
    const controls: FakeHostControl[] = [];
    const slot = { slot_id: 'slot-one', path: '/header', name: 'header', has_value: true };
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory(controls) });
    await open(session, api, document('document-one', 1, '{"header":"masked"}', [slot]));
    api.revealCredential.mockResolvedValueOnce({
      target: {
        document_id: 'document-one',
        document_revision: 1,
        slot_id: 'slot-one',
      },
      value: 'plaintext-never-in-snapshot',
    });

    await session.revealCredential('slot-one');
    expect(session.snapshot.revealed_slot_ids).toEqual(['slot-one']);
    expect(session.readRevealedCredential('slot-one')).toBe('plaintext-never-in-snapshot');
    expect(JSON.stringify(session.snapshot)).not.toContain('plaintext-never-in-snapshot');
    expect(JSON.stringify(session)).not.toContain('plaintext-never-in-snapshot');

    controls[0].emitFallback();
    await vi.waitFor(() => expect(session.snapshot.revealed_slot_ids).toEqual([]));
    expect(session.readRevealedCredential('slot-one')).toBeNull();

    api.revealCredential.mockResolvedValueOnce({
      target: {
        document_id: 'document-one',
        document_revision: 1,
        slot_id: 'slot-one',
      },
      value: 'second-plaintext',
    });
    await session.revealCredential('slot-one');
    await open(session, api, document('document-two', 1, '{"value":2}'));
    expect(session.readRevealedCredential('slot-one')).toBeNull();
    expect(JSON.stringify(session.snapshot)).not.toContain('second-plaintext');
  });

  it('invalidates current and pending reveal plaintext as open/create requests start and fail', async () => {
    const api = createApi();
    const controls: FakeHostControl[] = [];
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory(controls) });
    await enterRevisionConflictWithReplacement(session, api, 'switch-resolution-secret');
    api.revealCredential.mockResolvedValueOnce({
      target: {
        document_id: 'document-one',
        document_revision: 3,
        slot_id: 'base-header',
      },
      value: 'visible-before-switch',
    });
    await session.revealCredential('base-header');

    const openResponse = deferred<MaskedSourceDocument | null>();
    api.getDocument.mockReturnValueOnce(openResponse.promise);
    const switching = session.openDocument('document-two');
    expect(session.snapshot.revealed_slot_ids).toEqual([]);
    expect(session.readRevealedCredential('base-header')).toBeNull();
    expect(session.snapshot.conflict?.credentials[0]?.resolution).toBeNull();
    await vi.waitFor(() => expect(api.getDocument).toHaveBeenCalledTimes(2));
    openResponse.reject(new Error('open failed'));
    await expect(switching).rejects.toThrow('open failed');
    expect(session.readRevealedCredential('base-header')).toBeNull();

    const revealResponse = deferred<{
      target: { document_id: string; document_revision: number; slot_id: string };
      value: string;
    }>();
    api.revealCredential.mockReturnValueOnce(revealResponse.promise);
    const pendingReveal = session.revealCredential('base-header');
    const createResponse = deferred<DocumentMutationOutcome>();
    api.createDocument.mockReturnValueOnce(createResponse.promise);
    const creating = session.createDocument('Created', '{"value":1}');
    await vi.waitFor(() => expect(api.createDocument).toHaveBeenCalledOnce());
    revealResponse.resolve({
      target: {
        document_id: 'document-one',
        document_revision: 3,
        slot_id: 'base-header',
      },
      value: 'late-reveal-secret',
    });
    await pendingReveal;
    expect(session.readRevealedCredential('base-header')).toBeNull();
    createResponse.reject(new Error('create failed'));
    await expect(creating).rejects.toThrow('create failed');
    expect(JSON.stringify(session)).not.toContain('switch-resolution-secret');
    expect(JSON.stringify(session)).not.toContain('late-reveal-secret');
  });

  it.each(['continue', 'lock', 'fallback', 'dispose'] as const)(
    'clears conflict replacement plaintext on %s',
    async (boundary) => {
      const api = createApi();
      const controls: FakeHostControl[] = [];
      const session = new RuleEditorSession({ api, createEditorHost: createHostFactory(controls) });
      const replacement = `replacement-${boundary}-secret`;
      await enterRevisionConflictWithReplacement(session, api, replacement);
      expect(session.snapshot.conflict?.credentials[0]?.resolution).toBe('replace');
      expect(JSON.stringify(session)).not.toContain(replacement);

      if (boundary === 'continue') session.continueEditingConflict();
      else if (boundary === 'lock') await session.lock();
      else if (boundary === 'fallback') controls[0].emitFallback();
      else await session.dispose();

      expect(session.snapshot.conflict?.credentials[0]?.resolution).toBeNull();
      expect(JSON.stringify(session)).not.toContain(replacement);
      expect(api.rebaseDocument).not.toHaveBeenCalled();
    },
  );

  it('blocks unresolved conflicts and adopts a safe typed rebase result', async () => {
    const api = createApi();
    const baseSlot = { slot_id: 'base-header', path: '/header', name: 'header', has_value: true };
    const currentSlots = [
      { slot_id: 'current-header', path: '/header', name: 'header', has_value: true },
      { slot_id: 'current-cookie', path: '/cookie', name: 'cookie', has_value: true },
    ];
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory([]) });
    await open(session, api, document('document-one', 3, '{"value":1}', [baseSlot]));
    session.editText('{"value":2}');
    api.saveDocument.mockResolvedValueOnce({
      status: 'conflict',
      expected_revision: 3,
      actual_revision: 4,
      current: document('document-one', 4, '{"value":4}', currentSlots),
    });
    await expect(session.save()).resolves.toMatchObject({ status: 'conflict' });
    expect(session.snapshot.conflict).toMatchObject({
      kind: 'revision',
      reload_required: false,
    });

    await expect(session.save()).resolves.toEqual({
      status: 'blocked',
      reason: 'conflict_unresolved',
    });
    await expect(session.mergeConflict()).resolves.toEqual({
      status: 'blocked',
      reason: 'credential_resolutions_unresolved',
    });
    expect(api.rebaseDocument).not.toHaveBeenCalled();

    session.setConflictCredentialResolution('/header', { kind: 'keep_current' });
    session.setConflictCredentialResolution('/cookie', {
      kind: 'replace',
      value: 'replacement-never-in-snapshot',
    });
    expect(JSON.stringify(session.snapshot)).not.toContain('replacement-never-in-snapshot');
    expect(JSON.stringify(session)).not.toContain('replacement-never-in-snapshot');
    expect(session.snapshot.conflict?.credentials).toEqual([
      {
        path: '/cookie',
        base_present: false,
        current_present: true,
        resolution: 'replace',
      },
      {
        path: '/header',
        base_present: true,
        current_present: true,
        resolution: 'keep_current',
      },
    ]);

    api.rebaseDocument.mockResolvedValueOnce({
      status: 'saved',
      document: document('document-one', 5, '{"value":2}', [
        { slot_id: 'rebased-header', path: '/header', name: 'header', has_value: true },
        { slot_id: 'rebased-cookie', path: '/cookie', name: 'cookie', has_value: true },
      ]),
    });
    await expect(session.mergeConflict()).resolves.toMatchObject({ status: 'saved' });
    expect(api.rebaseDocument).toHaveBeenCalledWith({
      pin_id: 'pin-document-one-1',
      document_id: 'document-one',
      base_revision: 3,
      current_revision: 4,
      local_masked_text: '{"value":2}',
      mode: { kind: 'merge' },
      credential_resolutions: [
        { path: '/cookie', action: { kind: 'replace', value: 'replacement-never-in-snapshot' } },
        { path: '/header', action: { kind: 'keep_current' } },
      ],
    });
    expect(session.snapshot).toMatchObject({
      saved_revision: 5,
      dirty: false,
      conflict: null,
    });
  });

  it('uses InstalledSource.document_ref as the only working-copy relation', async () => {
    const api = createApi();
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory([]) });
    const legacySource: InstalledSource = {
      ...installed({ document_id: 'ignored', document_revision: 1 }),
      source_id: 'legacy-source',
      document_ref: null,
    };

    await expect(session.openInstalledSource(legacySource)).resolves.toEqual({
      status: 'snapshot_unavailable',
      source_id: 'legacy-source',
    });
    expect(api.getDocument).not.toHaveBeenCalled();

    const linked = installed({ document_id: 'authoritative-document', document_revision: 7 }, 8);
    api.getDocument.mockResolvedValueOnce(document('authoritative-document', 9, '{"value":9}'));
    await expect(session.openInstalledSource(linked)).resolves.toMatchObject({ status: 'opened' });
    expect(api.getDocument).toHaveBeenCalledWith({ document_id: 'authoritative-document' });
    expect(session.snapshot.installed_source).toBe(linked);
  });

  it('commits a prepared update with the generic install command and authoritative receipt', async () => {
    const api = createApi();
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory([]) });
    const savedDocument = document(
      'document-one',
      7,
      '{"bookSourceType":0,"bookSourceUrl":"https://example.com","bookSourceName":"Test","value":7}',
    );
    await open(session, api, savedDocument);
    api.prepareInstall.mockResolvedValueOnce(
      candidate({ document_id: 'document-one', document_revision: 7 }, 4),
    );
    await session.prepare();
    session.grant = 'network_only';
    const receipt = installed({ document_id: 'document-one', document_revision: 7 }, 5);
    api.installCandidate.mockResolvedValueOnce(receipt);
    api.getDocument.mockResolvedValueOnce({
      ...savedDocument,
      summary: { ...savedDocument.summary, state: 'linked', installed_revision: 5 },
    });

    await expect(session.installPrepared()).resolves.toEqual({
      status: 'installed',
      source: receipt,
    });
    expect(api.installCandidate).toHaveBeenCalledWith('candidate-7', 'network_only');
    expect(api.refreshInstalledSources).toHaveBeenCalledOnce();
    expect(session.snapshot.candidate).toBeNull();
    expect(session.snapshot.grant).toBe('none');
    expect(session.snapshot.installed_source).toBe(receipt);
  });

  it('surfaces a rejecting installed-list refresh without losing the install receipt', async () => {
    const api = createApi();
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory([]) });
    const savedDocument = document(
      'document-one',
      7,
      '{"bookSourceType":0,"bookSourceUrl":"https://example.com","bookSourceName":"Test"}',
    );
    await open(session, api, savedDocument);
    api.prepareInstall.mockResolvedValueOnce(
      candidate({ document_id: 'document-one', document_revision: 7 }),
    );
    await session.prepare();
    const receipt = installed({ document_id: 'document-one', document_revision: 7 }, 5);
    api.installCandidate.mockResolvedValueOnce(receipt);
    api.refreshInstalledSources.mockRejectedValueOnce(new Error('installed list unavailable'));
    api.getDocument.mockResolvedValueOnce(savedDocument);

    await expect(session.installPrepared()).resolves.toEqual({
      status: 'installed',
      source: receipt,
    });
    expect(session.snapshot.installed_source).toBe(receipt);
    expect(session.snapshot.operation_error).toBe('installed_refresh_failed');
  });

  it('deduplicates repeated leave-save intents and replays only after one clean save', async () => {
    const api = createApi();
    const session = new RuleEditorSession({ api, createEditorHost: createHostFactory([]) });
    await open(session, api, document('document-one', 1, '{"value":1}'));
    session.editText('{"value":2}');
    const saveResponse = deferred<DocumentMutationOutcome>();
    api.saveDocument.mockReturnValueOnce(saveResponse.promise);

    const firstLeave = session.resolveLeave('save');
    const repeatedLeave = session.resolveLeave('save');
    expect(repeatedLeave).toBe(firstLeave);
    await vi.waitFor(() => expect(api.saveDocument).toHaveBeenCalledOnce());
    saveResponse.resolve({
      status: 'saved',
      document: document('document-one', 2, '{"value":2}'),
    });

    await expect(firstLeave).resolves.toBe('resolved');
    await expect(repeatedLeave).resolves.toBe('resolved');
    expect(api.saveDocument).toHaveBeenCalledOnce();
    expect(session.snapshot.dirty).toBe(false);
    expect(session.snapshot.pinned_revision).toBeNull();

    await expect(session.resolveLeave('continue')).resolves.toBe('resolved');
    expect(api.saveDocument).toHaveBeenCalledOnce();
  });
});
