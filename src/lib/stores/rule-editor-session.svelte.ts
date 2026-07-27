import { tick } from 'svelte';
import {
  SourceAuthoringDocument,
  type AuthoringDiagnostic,
  type PointerLookupResult,
  type SourceAuthoringEditResult,
  type SourceAuthoringFormatRequest,
  type SourceAuthoringPatch,
} from '$lib/rules/authoring';
import {
  refreshInstalledSources,
  type CapabilityGrantPreset,
  type InstallCandidate,
  type InstallDiagnostic,
  type InstalledSource,
} from './rules.svelte';
import {
  SourceDocumentEditorHost,
  type SourceDocumentEditorSnapshot,
  type SourceEditorDocumentInput,
} from '$lib/views/sources/source-document-editor-host';
import {
  clearSourceDocumentCredential as clearCredentialApi,
  createSourceDocument as createDocumentApi,
  deleteSourceDocument as deleteDocumentApi,
  getSourceDocument as getDocumentApi,
  listSourceDocuments as listDocumentsApi,
  installSourceCandidate as installCandidateApi,
  pinSourceDocumentRevision as pinRevisionApi,
  prepareInstallFromDocument as prepareInstallApi,
  rebaseSourceDocument as rebaseDocumentApi,
  releaseSourceDocumentRevisionPin as releaseRevisionPinApi,
  renameSourceDocument as renameDocumentApi,
  replaceSourceDocumentCredential as replaceCredentialApi,
  revealSourceDocumentCredential as revealCredentialApi,
  saveSourceDocument as saveDocumentApi,
  type CredentialSlotSummary,
  type DocumentMutationOutcome,
  type DocumentRef,
  type MaskedSourceDocument,
  type RebaseSourceDocumentRequest,
  type SourceDocumentCredentialResolution,
  type SourceDocumentCredentialResolutionAction,
  type SourceDocumentCredentialTarget,
  type SourceDocumentRevisionPin,
  type SourceDocumentSummary,
} from '$lib/views/sources/rule-editor-api';

export type RuleEditorMode = 'original' | 'form' | 'tree';
export type RuleEditorLeaveAction = 'save' | 'discard' | 'continue';
export type RuleEditorLeaveResolution = 'resolved' | 'blocked';

export type RuleEditorBlockedReason =
  | 'no_document'
  | 'disposed'
  | 'dirty'
  | 'conflict_unresolved'
  | 'credential_resolutions_unresolved'
  | 'document_invalid'
  | 'locked'
  | 'candidate_missing'
  | 'candidate_stale'
  | 'document_snapshot_unavailable';

export interface RuleEditorBlockedResult {
  readonly status: 'blocked';
  readonly reason: RuleEditorBlockedReason;
}

export type RuleEditorSaveResult =
  | DocumentMutationOutcome
  | { readonly status: 'unchanged'; readonly document: MaskedSourceDocument }
  | { readonly status: 'cancelled' }
  | RuleEditorBlockedResult;

export type RuleEditorPrepareResult =
  | { readonly status: 'prepared'; readonly candidate: InstallCandidate }
  | { readonly status: 'stale' }
  | RuleEditorBlockedResult;

export type RuleEditorInstallResult =
  | { readonly status: 'installed'; readonly source: InstalledSource }
  | { readonly status: 'stale' }
  | RuleEditorBlockedResult;

export type RuleEditorOpenResult =
  | { readonly status: 'opened'; readonly document: MaskedSourceDocument }
  | { readonly status: 'not_found' }
  | { readonly status: 'cancelled' }
  | { readonly status: 'snapshot_unavailable'; readonly source_id: string }
  | RuleEditorBlockedResult;

export interface RuleEditorPinnedRevisionSnapshot {
  readonly document_id: string;
  readonly document_revision: number;
  readonly expires_at_ms: number;
}

export type RuleEditorCredentialResolutionKind =
  'keep_current' | 'keep_local' | 'replace' | 'clear';

export interface RuleEditorConflictCredentialSnapshot {
  readonly path: string;
  readonly base_present: boolean;
  readonly current_present: boolean;
  readonly resolution: RuleEditorCredentialResolutionKind | null;
}

export type RuleEditorConflictKind = 'revision' | 'sentinel_rotation' | 'snapshot_unavailable';

export interface RuleEditorConflictSnapshot {
  readonly kind: RuleEditorConflictKind;
  readonly reload_required: boolean;
  readonly base_revision: number;
  readonly current_revision: number;
  readonly current: MaskedSourceDocument;
  readonly dismissed: boolean;
  readonly credentials: readonly RuleEditorConflictCredentialSnapshot[];
}

export interface RuleEditorHistorySnapshot {
  readonly can_undo: boolean;
  readonly can_redo: boolean;
}

export type RuleEditorOperationErrorCode =
  | 'document_load_failed'
  | 'document_pin_failed'
  | 'document_pin_release_failed'
  | 'editor_sync_failed'
  | 'save_failed'
  | 'prepare_failed'
  | 'install_failed'
  | 'installed_refresh_failed'
  | 'credential_reveal_failed'
  | 'credential_mutation_failed'
  | 'rebase_failed'
  | 'leave_failed';

export interface RuleEditorSessionSnapshot {
  readonly documents: readonly SourceDocumentSummary[];
  readonly documents_loading: boolean;
  readonly document: MaskedSourceDocument | null;
  readonly text: string;
  readonly saved_text: string;
  readonly saved_revision: number;
  readonly dirty: boolean;
  readonly document_epoch: number;
  readonly mode: RuleEditorMode;
  readonly diagnostics: readonly AuthoringDiagnostic[];
  readonly history: RuleEditorHistorySnapshot;
  readonly save_phase: 'idle' | 'saving' | 'queued';
  readonly queued_save_count: number;
  readonly prepare_pending: boolean;
  readonly prepare_diagnostics: readonly InstallDiagnostic[];
  readonly candidate: InstallCandidate | null;
  readonly grant: CapabilityGrantPreset;
  readonly install_pending: boolean;
  readonly installed_source: InstalledSource | null;
  readonly revealed_slot_ids: readonly string[];
  readonly conflict: RuleEditorConflictSnapshot | null;
  readonly rebase_pending: boolean;
  readonly pinned_revision: RuleEditorPinnedRevisionSnapshot | null;
  readonly locked: boolean;
  readonly operation_error: RuleEditorOperationErrorCode | null;
  readonly disposed: boolean;
}

export interface RuleEditorSessionApi {
  readonly listDocuments: () => Promise<SourceDocumentSummary[]>;
  readonly createDocument: (request: {
    format: 'legado';
    title: string;
    text: string;
  }) => Promise<DocumentMutationOutcome>;
  readonly getDocument: (request: { document_id: string }) => Promise<MaskedSourceDocument | null>;
  readonly saveDocument: (request: {
    document_id: string;
    expected_revision: number;
    masked_text: string;
  }) => Promise<DocumentMutationOutcome>;
  readonly renameDocument: (request: {
    document_id: string;
    expected_revision: number;
    title: string;
  }) => Promise<DocumentMutationOutcome>;
  readonly deleteDocument: (request: {
    document_id: string;
    expected_revision: number;
  }) => Promise<DocumentMutationOutcome>;
  readonly revealCredential: (request: {
    target: SourceDocumentCredentialTarget;
  }) => Promise<{ target: SourceDocumentCredentialTarget; value: string }>;
  readonly replaceCredential: (request: {
    target: SourceDocumentCredentialTarget;
    value: string;
  }) => Promise<DocumentMutationOutcome>;
  readonly clearCredential: (request: {
    target: SourceDocumentCredentialTarget;
  }) => Promise<DocumentMutationOutcome>;
  readonly pinRevision: (request: DocumentRef) => Promise<SourceDocumentRevisionPin>;
  readonly releaseRevisionPin: (request: {
    pin_id: string;
  }) => Promise<{ status: 'released'; pin_id: string }>;
  readonly rebaseDocument: (
    request: RebaseSourceDocumentRequest,
  ) => Promise<DocumentMutationOutcome>;
  readonly prepareInstall: (request: DocumentRef) => Promise<InstallCandidate>;
  readonly installCandidate: (
    candidateId: string,
    grant: CapabilityGrantPreset,
  ) => Promise<InstalledSource>;
  readonly refreshInstalledSources: () => Promise<void>;
}

export type RuleEditorEditorHostFactory = (
  document: SourceEditorDocumentInput,
  onTextEdit: (text: string) => void,
) => SourceDocumentEditorHost;

export interface RuleEditorSessionOptions {
  readonly api?: Partial<RuleEditorSessionApi>;
  readonly createEditorHost?: RuleEditorEditorHostFactory;
  readonly historyLimit?: number;
}

interface TextCommand {
  readonly before: string;
  readonly after: string;
}

interface PendingSaveIntent {
  payloadText: string;
  credentialSlots: CredentialSlotSummary[];
  readonly requestEpoch: number;
  readonly documentGeneration: number;
  readonly resolve: (result: RuleEditorSaveResult) => void;
  readonly reject: (error: unknown) => void;
}

interface ConflictState {
  readonly kind: RuleEditorConflictKind;
  readonly baseRevision: number;
  readonly baseCredentialPaths: readonly string[];
  current: MaskedSourceDocument;
  dismissed: boolean;
}

class CredentialRevealHolder {
  #values: Array<{ readonly slotId: string; readonly value: string }> = [];

  set(slotId: string, value: string): void {
    this.#values = [...this.#values.filter((entry) => entry.slotId !== slotId), { slotId, value }];
  }

  read(slotId: string): string | null {
    return this.#values.find((entry) => entry.slotId === slotId)?.value ?? null;
  }

  clear(): void {
    this.#values = [];
  }
}

class CredentialResolutionHolder {
  #entries: Array<{
    readonly path: string;
    readonly action: SourceDocumentCredentialResolutionAction;
  }> = [];

  get size(): number {
    return this.#entries.length;
  }

  set(path: string, action: SourceDocumentCredentialResolutionAction): void {
    this.#entries = [...this.#entries.filter((entry) => entry.path !== path), { path, action }];
  }

  get(path: string): SourceDocumentCredentialResolutionAction | null {
    return this.#entries.find((entry) => entry.path === path)?.action ?? null;
  }

  has(path: string): boolean {
    return this.#entries.some((entry) => entry.path === path);
  }

  delete(path: string): boolean {
    const hadEntry = this.has(path);
    if (hadEntry) this.#entries = this.#entries.filter((entry) => entry.path !== path);
    return hadEntry;
  }

  clear(): void {
    this.#entries = [];
  }
}

const EMPTY_SNAPSHOT: RuleEditorSessionSnapshot = Object.freeze({
  documents: Object.freeze([]),
  documents_loading: false,
  document: null,
  text: '',
  saved_text: '',
  saved_revision: 0,
  dirty: false,
  document_epoch: 0,
  mode: 'original',
  diagnostics: Object.freeze([]),
  authoring: null,
  history: Object.freeze({ can_undo: false, can_redo: false }),
  save_phase: 'idle',
  queued_save_count: 0,
  prepare_pending: false,
  prepare_diagnostics: Object.freeze([]),
  candidate: null,
  grant: 'none',
  install_pending: false,
  installed_source: null,
  revealed_slot_ids: Object.freeze([]),
  conflict: null,
  rebase_pending: false,
  pinned_revision: null,
  locked: false,
  operation_error: null,
  disposed: false,
});

const defaultApi: RuleEditorSessionApi = {
  listDocuments: listDocumentsApi,
  createDocument: createDocumentApi,
  getDocument: getDocumentApi,
  saveDocument: saveDocumentApi,
  renameDocument: renameDocumentApi,
  deleteDocument: deleteDocumentApi,
  revealCredential: revealCredentialApi,
  replaceCredential: replaceCredentialApi,
  clearCredential: clearCredentialApi,
  pinRevision: pinRevisionApi,
  releaseRevisionPin: releaseRevisionPinApi,
  rebaseDocument: rebaseDocumentApi,
  prepareInstall: prepareInstallApi,
  installCandidate: (candidateId, grant) =>
    installCandidateApi({ candidate_id: candidateId, grant }),
  refreshInstalledSources,
};

function createDefaultEditorHost(
  document: SourceEditorDocumentInput,
  onTextEdit: (text: string) => void,
): SourceDocumentEditorHost {
  return new SourceDocumentEditorHost(document, { onTextEdit });
}

function uniqueSorted(values: readonly string[]): readonly string[] {
  const sorted = [...values].sort();
  return Object.freeze(sorted.filter((value, index) => index === 0 || value !== sorted[index - 1]));
}

function pathsFromSlots(slots: readonly CredentialSlotSummary[]): readonly string[] {
  return uniqueSorted(slots.map((slot) => slot.path));
}

function pointerStringValue(document: SourceAuthoringDocument, path: string): string | null {
  const lookup = document.lookupPointer(path);
  return lookup.kind === 'found' && typeof lookup.entry.node.value === 'string'
    ? lookup.entry.node.value
    : null;
}

function reconcileCredentialSentinels(
  text: string,
  epoch: number,
  payloadText: string,
  previousSlots: readonly CredentialSlotSummary[],
  saved: MaskedSourceDocument,
): SourceAuthoringDocument | null {
  if (text === payloadText) {
    return SourceAuthoringDocument.open(
      saved.masked_text,
      saved.masked_text === text ? epoch : epoch + 1,
    );
  }

  const previousPaths = pathsFromSlots(previousSlots);
  const savedPaths = pathsFromSlots(saved.credential_slots);
  if (
    previousPaths.length !== previousSlots.length ||
    savedPaths.length !== saved.credential_slots.length ||
    previousPaths.length !== savedPaths.length ||
    previousPaths.some((path, index) => path !== savedPaths[index])
  ) {
    return null;
  }

  const payloadDocument = SourceAuthoringDocument.open(payloadText);
  const savedDocument = SourceAuthoringDocument.open(saved.masked_text);
  const reconciled = SourceAuthoringDocument.open(text, epoch);
  const rotations: Array<{ readonly path: string; readonly sentinel: string }> = [];
  for (const path of previousPaths) {
    const previousSentinel = pointerStringValue(payloadDocument, path);
    const currentSentinel = pointerStringValue(reconciled, path);
    const savedSentinel = pointerStringValue(savedDocument, path);
    if (
      previousSentinel === null ||
      savedSentinel === null ||
      currentSentinel !== previousSentinel
    ) {
      return null;
    }
    rotations.push({ path, sentinel: savedSentinel });
  }

  for (const rotation of rotations) {
    const result = reconciled.applyPatch({
      operation: 'set',
      pointer: rotation.path,
      value: rotation.sentinel,
      expectedEpoch: reconciled.epoch,
    });
    if (result.kind === 'rejected') return null;
  }
  return reconciled;
}

function sameDocumentRef(left: DocumentRef | null, right: DocumentRef | null): boolean {
  return (
    left !== null &&
    right !== null &&
    left.document_id === right.document_id &&
    left.document_revision === right.document_revision
  );
}

function isBlockedDiagnostic(diagnostic: AuthoringDiagnostic): boolean {
  return diagnostic.severity === 'error' || diagnostic.support === 'blocked';
}

function blocked(reason: RuleEditorBlockedReason): RuleEditorBlockedResult {
  return Object.freeze({ status: 'blocked', reason });
}

/**
 * 页面级 Legado 工作区状态机。正文、revision、dirty、candidate、credential 与 conflict
 * 只由本类持有；编辑器 host 仅承载运行时 adapter，并通过受控回调提交文本。
 */
export class RuleEditorSession {
  private readonly api: RuleEditorSessionApi;
  private readonly createEditorHost: RuleEditorEditorHostFactory;
  private readonly historyLimit: number;
  private subscribers: Array<(snapshot: RuleEditorSessionSnapshot) => void> = [];
  private readonly revealHolder = new CredentialRevealHolder();

  private snapshotState = $state.raw<RuleEditorSessionSnapshot>(EMPTY_SNAPSHOT);
  private editorHostState = $state.raw<SourceDocumentEditorHost | null>(null);

  private documents: SourceDocumentSummary[] = [];
  private documentsLoading = false;
  private summary: SourceDocumentSummary | null = null;
  private credentialSlots: CredentialSlotSummary[] = [];
  private authoringDocument: SourceAuthoringDocument | null = null;
  private savedText = '';
  private savedRevision = 0;
  private epochCounter = 0;
  private modeValue: RuleEditorMode = 'original';
  private candidateValue: InstallCandidate | null = null;
  private grantValue: CapabilityGrantPreset = 'none';
  private prepareDiagnostics: InstallDiagnostic[] = [];
  private preparePending = false;
  private prepareGeneration = 0;
  private installPending = false;
  private installedSource: InstalledSource | null = null;
  private currentPin: SourceDocumentRevisionPin | null = null;
  private conflictState: ConflictState | null = null;
  private readonly conflictResolutionHolder = new CredentialResolutionHolder();
  private rebasePending = false;
  private revealedSlotIds: string[] = [];
  private revealGeneration = 0;
  private undoStack: TextCommand[] = [];
  private redoStack: TextCommand[] = [];
  private readonly saveQueue: PendingSaveIntent[] = [];
  private saveActive = false;
  private documentGeneration = 0;
  private documentOperationGeneration = 0;
  private editorUnsubscribe: (() => void) | null = null;
  private lastEditorStatus: SourceDocumentEditorSnapshot['status'] | null = null;
  private lockedValue = false;
  private operationError: RuleEditorOperationErrorCode | null = null;
  private disposedValue = false;
  private leaveResolutionPromise: Promise<RuleEditorLeaveResolution> | null = null;

  constructor(options: RuleEditorSessionOptions = {}) {
    this.api = { ...defaultApi, ...options.api };
    this.createEditorHost = options.createEditorHost ?? createDefaultEditorHost;
    this.historyLimit = options.historyLimit ?? 100;
    if (!Number.isSafeInteger(this.historyLimit) || this.historyLimit <= 0) {
      throw new RangeError('historyLimit must be a positive safe integer');
    }
  }

  get snapshot(): RuleEditorSessionSnapshot {
    return this.snapshotState;
  }

  toJSON(): RuleEditorSessionSnapshot {
    return this.snapshotState;
  }

  get editorHost(): SourceDocumentEditorHost | null {
    return this.editorHostState;
  }

  get grant(): CapabilityGrantPreset {
    return this.snapshotState.grant;
  }

  set grant(value: CapabilityGrantPreset) {
    this.setGrant(value);
  }

  subscribe(subscriber: (snapshot: RuleEditorSessionSnapshot) => void): () => void {
    if (!this.subscribers.includes(subscriber)) this.subscribers.push(subscriber);
    subscriber(this.snapshotState);
    return () => {
      this.subscribers = this.subscribers.filter((entry) => entry !== subscriber);
    };
  }

  async refreshDocuments(): Promise<readonly SourceDocumentSummary[]> {
    if (this.disposedValue) return this.documents;
    this.documentsLoading = true;
    this.operationError = null;
    this.publish();
    try {
      this.documents = (await this.api.listDocuments()).filter(
        (document) => document.format === 'legado',
      );
      return this.documents;
    } catch (error) {
      this.operationError = 'document_load_failed';
      throw error;
    } finally {
      this.documentsLoading = false;
      this.publish();
    }
  }

  async createDocument(title: string, text: string): Promise<DocumentMutationOutcome> {
    this.assertUsable();
    await this.clearCredentialPlaintextState();
    const outcome = await this.api.createDocument({ format: 'legado', title, text });
    if (outcome.status === 'saved' && outcome.document) {
      const pin = await this.pinDocument(outcome.document);
      await this.transitionToDocument(outcome.document, pin, null);
    } else if (outcome.status === 'locked') {
      await this.lock();
    }
    return outcome;
  }

  async openDocument(
    documentId: string,
    installedSource: InstalledSource | null = null,
  ): Promise<RuleEditorOpenResult> {
    if (this.disposedValue) return blocked('disposed');
    const operationGeneration = ++this.documentOperationGeneration;
    this.operationError = null;
    await this.clearCredentialPlaintextState();

    let document: MaskedSourceDocument | null;
    try {
      document = await this.api.getDocument({ document_id: documentId });
    } catch (error) {
      this.operationError = 'document_load_failed';
      this.publish();
      throw error;
    }
    if (operationGeneration !== this.documentOperationGeneration) return { status: 'cancelled' };
    if (!document) return { status: 'not_found' };
    if (document.summary.format !== 'legado') return blocked('document_snapshot_unavailable');

    let pin: SourceDocumentRevisionPin;
    try {
      pin = await this.pinDocument(document);
    } catch (error) {
      this.operationError = 'document_pin_failed';
      this.publish();
      throw error;
    }
    if (operationGeneration !== this.documentOperationGeneration) {
      await this.releasePinBestEffort(pin);
      return { status: 'cancelled' };
    }

    await this.transitionToDocument(document, pin, installedSource);
    return { status: 'opened', document };
  }

  async openInstalledSource(source: InstalledSource): Promise<RuleEditorOpenResult> {
    if (!source.document_ref) {
      return { status: 'snapshot_unavailable', source_id: source.source_id };
    }
    return this.openDocument(source.document_ref.document_id, source);
  }

  async renameCurrentDocument(
    title: string,
  ): Promise<DocumentMutationOutcome | RuleEditorBlockedResult> {
    const current = this.currentDocumentRef();
    if (!current) return blocked(this.disposedValue ? 'disposed' : 'no_document');
    const outcome = await this.api.renameDocument({
      document_id: current.document_id,
      expected_revision: current.document_revision,
      title,
    });
    if (outcome.status === 'saved' && outcome.document) {
      this.summary = outcome.document.summary;
      this.credentialSlots = [...outcome.document.credential_slots];
      this.upsertDocumentSummary(outcome.document.summary);
      this.publish();
    } else if (outcome.status === 'conflict') {
      this.enterConflict(
        current.document_revision,
        pathsFromSlots(this.credentialSlots),
        outcome.current,
      );
    } else if (outcome.status === 'locked') {
      await this.lock();
    }
    return outcome;
  }

  async deleteCurrentDocument(): Promise<DocumentMutationOutcome | RuleEditorBlockedResult> {
    const current = this.currentDocumentRef();
    if (!current) return blocked(this.disposedValue ? 'disposed' : 'no_document');
    if (this.snapshotState.dirty) return blocked('dirty');
    if (this.conflictState) return blocked('conflict_unresolved');

    await this.clearRevealedCredentials();
    const releasedPin = this.currentPin;
    if (releasedPin) {
      try {
        await this.releasePin(releasedPin);
        this.currentPin = null;
      } catch (error) {
        this.operationError = 'document_pin_release_failed';
        this.publish();
        throw error;
      }
    }

    let outcome: DocumentMutationOutcome;
    try {
      outcome = await this.api.deleteDocument({
        document_id: current.document_id,
        expected_revision: current.document_revision,
      });
    } catch (error) {
      if (releasedPin) {
        try {
          this.currentPin = await this.api.pinRevision(current);
        } catch {
          this.operationError = 'document_pin_failed';
        }
      }
      this.publish();
      throw error;
    }
    if (outcome.status === 'saved' && outcome.document === null) {
      this.documents = this.documents.filter((entry) => entry.document_id !== current.document_id);
      await this.clearDocumentState();
    } else if (outcome.status === 'conflict') {
      this.enterSnapshotUnavailableConflict(current.document_revision, outcome.current);
    } else {
      if (releasedPin) {
        try {
          this.currentPin = await this.api.pinRevision(current);
        } catch {
          this.operationError = 'document_pin_failed';
        }
      }
      this.publish();
    }
    return outcome;
  }

  setMode(mode: RuleEditorMode): boolean {
    if (this.disposedValue || !this.authoringDocument) return false;
    if (mode === this.modeValue) return true;
    this.modeValue = mode;
    this.publish();
    return true;
  }

  editText(text: string): SourceAuthoringEditResult | null {
    const document = this.authoringDocument;
    if (!document || this.disposedValue) return null;
    const before = document.text;
    const result = document.replaceText({ expectedEpoch: document.epoch, text });
    this.acceptAuthoringEdit(before, result);
    return result;
  }

  applyPatch(patch: SourceAuthoringPatch): SourceAuthoringEditResult | null {
    const document = this.authoringDocument;
    if (!document || this.disposedValue) return null;
    const before = document.text;
    const result = document.applyPatch(patch);
    this.acceptAuthoringEdit(before, result);
    return result;
  }

  format(
    request?: Omit<SourceAuthoringFormatRequest, 'expectedEpoch'>,
  ): SourceAuthoringEditResult | null {
    const document = this.authoringDocument;
    if (!document || this.disposedValue) return null;
    const before = document.text;
    const result = document.format({ expectedEpoch: document.epoch, ...request });
    this.acceptAuthoringEdit(before, result);
    return result;
  }

  lookupPointer(pointer: string): PointerLookupResult | null {
    return this.authoringDocument?.lookupPointer(pointer) ?? null;
  }

  undo(): boolean {
    const document = this.authoringDocument;
    const command = this.undoStack.pop();
    if (!document || !command || document.text !== command.after) {
      if (command) this.undoStack.push(command);
      return false;
    }
    const result = document.replaceText({ expectedEpoch: document.epoch, text: command.before });
    if (result.kind !== 'applied') {
      this.undoStack.push(command);
      return false;
    }
    this.redoStack.push(command);
    this.afterTextTransition(result.text);
    return true;
  }

  redo(): boolean {
    const document = this.authoringDocument;
    const command = this.redoStack.pop();
    if (!document || !command || document.text !== command.before) {
      if (command) this.redoStack.push(command);
      return false;
    }
    const result = document.replaceText({ expectedEpoch: document.epoch, text: command.after });
    if (result.kind !== 'applied') {
      this.redoStack.push(command);
      return false;
    }
    this.undoStack.push(command);
    this.afterTextTransition(result.text);
    return true;
  }

  save(): Promise<RuleEditorSaveResult> {
    if (this.disposedValue) return Promise.resolve(blocked('disposed'));
    if (this.lockedValue) return Promise.resolve(blocked('locked'));
    if (!this.summary || !this.authoringDocument) return Promise.resolve(blocked('no_document'));
    if (this.conflictState) return Promise.resolve(blocked('conflict_unresolved'));
    if (!this.snapshotState.dirty) {
      return Promise.resolve({ status: 'unchanged', document: this.currentMaskedDocument()! });
    }

    return new Promise<RuleEditorSaveResult>((resolve, reject) => {
      this.saveQueue.push({
        payloadText: this.authoringDocument!.text,
        credentialSlots: [...this.credentialSlots],
        requestEpoch: this.authoringDocument!.epoch,
        documentGeneration: this.documentGeneration,
        resolve,
        reject,
      });
      this.publish();
      void this.drainSaveQueue();
    });
  }

  async prepare(): Promise<RuleEditorPrepareResult> {
    if (this.disposedValue) return blocked('disposed');
    if (this.lockedValue) return blocked('locked');
    const documentRef = this.currentDocumentRef();
    const document = this.authoringDocument;
    if (!documentRef || !document) return blocked('no_document');
    if (this.conflictState) return blocked('conflict_unresolved');
    if (this.snapshotState.dirty) return blocked('dirty');
    if (document.diagnostics.some(isBlockedDiagnostic)) return blocked('document_invalid');

    const prepareGeneration = ++this.prepareGeneration;
    const documentGeneration = this.documentGeneration;
    const requestEpoch = document.epoch;
    this.preparePending = true;
    this.prepareDiagnostics = [];
    this.candidateValue = null;
    this.grantValue = 'none';
    this.operationError = null;
    this.publish();

    try {
      const candidate = await this.api.prepareInstall(documentRef);
      const currentRef = this.currentDocumentRef();
      if (
        prepareGeneration !== this.prepareGeneration ||
        documentGeneration !== this.documentGeneration ||
        requestEpoch !== this.authoringDocument?.epoch ||
        this.snapshotState.dirty ||
        !sameDocumentRef(candidate.document_ref, currentRef)
      ) {
        return { status: 'stale' };
      }
      this.candidateValue = candidate;
      this.grantValue = 'none';
      this.prepareDiagnostics = [...candidate.diagnostics];
      return { status: 'prepared', candidate };
    } catch (error) {
      if (prepareGeneration === this.prepareGeneration) this.operationError = 'prepare_failed';
      throw error;
    } finally {
      if (prepareGeneration === this.prepareGeneration) this.preparePending = false;
      this.publish();
    }
  }

  setGrant(grant: CapabilityGrantPreset): void {
    if (!this.candidateValue || this.disposedValue) return;
    if (grant === this.grantValue) return;
    this.grantValue = grant;
    this.publish();
  }

  async installPrepared(): Promise<RuleEditorInstallResult> {
    if (this.disposedValue) return blocked('disposed');
    if (this.lockedValue) return blocked('locked');
    const candidate = this.candidateValue;
    const currentRef = this.currentDocumentRef();
    if (!candidate) return blocked('candidate_missing');
    if (!sameDocumentRef(candidate.document_ref, currentRef) || this.snapshotState.dirty) {
      return blocked('candidate_stale');
    }

    const candidateId = candidate.id;
    const grant = this.grantValue;
    this.installPending = true;
    this.operationError = null;
    this.publish();
    try {
      const source = await this.api.installCandidate(candidateId, grant);
      if (!sameDocumentRef(source.document_ref, candidate.document_ref)) {
        this.operationError = 'install_failed';
        return { status: 'stale' };
      }

      this.installedSource = source;
      if (this.candidateValue?.id === candidateId) this.invalidatePreparedState();
      const refreshes = await Promise.allSettled([
        this.api.refreshInstalledSources(),
        this.api.getDocument({ document_id: source.document_ref!.document_id }),
      ]);
      const refreshedDocument = refreshes[1];
      const refreshRevisionMatches =
        refreshedDocument.status === 'fulfilled' &&
        refreshedDocument.value?.summary.revision === this.savedRevision;
      if (refreshRevisionMatches && refreshedDocument.value) {
        this.summary = refreshedDocument.value.summary;
        this.credentialSlots = [...refreshedDocument.value.credential_slots];
        this.upsertDocumentSummary(refreshedDocument.value.summary);
      }
      if (refreshes.some((result) => result.status === 'rejected') || !refreshRevisionMatches) {
        this.operationError = 'installed_refresh_failed';
      }
      return { status: 'installed', source };
    } catch (error) {
      this.operationError = 'install_failed';
      throw error;
    } finally {
      this.installPending = false;
      this.publish();
    }
  }

  async revealCredential(slotId: string): Promise<void> {
    this.assertUsable();
    if (this.lockedValue) throw new Error('Rule editor session is locked');
    const current = this.currentDocumentRef();
    if (!current) throw new Error('Rule editor session has no document');
    if (!this.credentialSlots.some((slot) => slot.slot_id === slotId)) {
      throw new Error('Credential slot does not belong to the current document');
    }

    const target: SourceDocumentCredentialTarget = {
      document_id: current.document_id,
      document_revision: current.document_revision,
      slot_id: slotId,
    };
    const revealGeneration = this.revealGeneration;
    const documentGeneration = this.documentGeneration;
    try {
      const revealed = await this.api.revealCredential({ target });
      if (
        revealGeneration !== this.revealGeneration ||
        documentGeneration !== this.documentGeneration ||
        revealed.target.document_id !== target.document_id ||
        revealed.target.document_revision !== target.document_revision ||
        revealed.target.slot_id !== target.slot_id
      ) {
        return;
      }
      this.revealHolder.set(slotId, revealed.value);
      if (!this.revealedSlotIds.includes(slotId)) this.revealedSlotIds.push(slotId);
      this.publish();
    } catch (error) {
      this.operationError = 'credential_reveal_failed';
      this.publish();
      throw error;
    }
  }

  readRevealedCredential(slotId: string): string | null {
    if (!this.revealedSlotIds.includes(slotId)) return null;
    return this.revealHolder.read(slotId);
  }

  async clearRevealedCredentials(): Promise<void> {
    this.revealGeneration += 1;
    const hadVisibleValues = this.revealedSlotIds.length > 0;
    this.revealedSlotIds = [];
    if (hadVisibleValues) {
      this.publish();
      await tick();
    }
    this.revealHolder.clear();
  }

  replaceCredential(
    slotId: string,
    value: string,
  ): Promise<DocumentMutationOutcome | RuleEditorBlockedResult> {
    return this.mutateCredential(slotId, { kind: 'replace', value });
  }

  clearCredential(slotId: string): Promise<DocumentMutationOutcome | RuleEditorBlockedResult> {
    return this.mutateCredential(slotId, { kind: 'clear' });
  }

  setConflictCredentialResolution(
    path: string,
    action: SourceDocumentCredentialResolutionAction,
  ): boolean {
    if (
      !this.conflictState ||
      this.conflictState.kind !== 'revision' ||
      !this.conflictCredentialPaths().includes(path)
    ) {
      return false;
    }
    this.conflictResolutionHolder.set(path, action);
    this.invalidatePreparedState();
    this.publish();
    return true;
  }

  clearConflictCredentialResolution(path: string): void {
    if (!this.conflictResolutionHolder.delete(path)) return;
    this.invalidatePreparedState();
    this.publish();
  }

  continueEditingConflict(): boolean {
    if (!this.conflictState || this.conflictState.kind !== 'revision') return false;
    this.conflictResolutionHolder.clear();
    this.conflictState.dismissed = true;
    this.invalidatePreparedState();
    this.publish();
    this.focusEditor();
    return true;
  }

  async reloadConflict(): Promise<DocumentMutationOutcome | RuleEditorBlockedResult> {
    const conflict = this.conflictState;
    if (!conflict) return blocked('conflict_unresolved');
    const pin = await this.pinDocument(conflict.current);
    await this.transitionToDocument(conflict.current, pin, this.installedSource);
    return { status: 'saved', document: conflict.current };
  }

  mergeConflict(): Promise<DocumentMutationOutcome | RuleEditorBlockedResult> {
    return this.rebaseConflict({ kind: 'merge' });
  }

  forkConflict(title: string): Promise<DocumentMutationOutcome | RuleEditorBlockedResult> {
    return this.rebaseConflict({ kind: 'fork', title });
  }

  canLeave(): boolean {
    return (
      !this.disposedValue &&
      !this.snapshotState.dirty &&
      !this.saveActive &&
      this.saveQueue.length === 0 &&
      this.conflictState === null
    );
  }

  resolveLeave(action: RuleEditorLeaveAction): Promise<RuleEditorLeaveResolution> {
    if (this.leaveResolutionPromise) return this.leaveResolutionPromise;
    this.leaveResolutionPromise = this.resolveLeaveOnce(action).finally(() => {
      this.leaveResolutionPromise = null;
    });
    return this.leaveResolutionPromise;
  }

  focusEditor(): void {
    this.editorHostState?.focus();
  }

  async lock(): Promise<void> {
    if (this.disposedValue) return;
    this.lockedValue = true;
    this.conflictResolutionHolder.clear();
    this.invalidatePreparedState();
    this.publish();
    await this.clearRevealedCredentials();
    const pin = this.currentPin;
    this.currentPin = null;
    if (pin) await this.releasePinBestEffort(pin);
    this.publish();
  }

  async dispose(): Promise<void> {
    if (this.disposedValue) return;
    this.disposedValue = true;
    this.documentOperationGeneration += 1;
    this.documentGeneration += 1;
    this.cancelQueuedSaves();
    this.invalidatePreparedState();
    this.conflictResolutionHolder.clear();
    this.publish();
    await this.clearRevealedCredentials();
    const pin = this.currentPin;
    this.currentPin = null;
    if (pin) await this.releasePinBestEffort(pin);
    this.editorUnsubscribe?.();
    this.editorUnsubscribe = null;
    this.editorHostState?.dispose();
    this.editorHostState = null;
    this.publish();
    this.subscribers = [];
  }

  private async clearCredentialPlaintextState(): Promise<void> {
    const hadConflictResolutions = this.conflictResolutionHolder.size > 0;
    this.conflictResolutionHolder.clear();
    if (hadConflictResolutions) this.publish();
    await this.clearRevealedCredentials();
  }

  private assertUsable(): void {
    if (this.disposedValue) throw new Error('Rule editor session is disposed');
  }

  private currentDocumentRef(): DocumentRef | null {
    return this.summary
      ? {
          document_id: this.summary.document_id,
          document_revision: this.savedRevision,
        }
      : null;
  }

  private currentMaskedDocument(): MaskedSourceDocument | null {
    if (!this.summary || !this.authoringDocument) return null;
    return {
      summary: this.summary,
      masked_text: this.authoringDocument.text,
      credential_slots: [...this.credentialSlots],
    };
  }

  private async pinDocument(document: MaskedSourceDocument): Promise<SourceDocumentRevisionPin> {
    const pin = await this.api.pinRevision({
      document_id: document.summary.document_id,
      document_revision: document.summary.revision,
    });
    if (
      pin.document_id !== document.summary.document_id ||
      pin.document_revision !== document.summary.revision
    ) {
      await this.releasePinBestEffort(pin);
      throw new Error('Revision pin owner mismatch');
    }
    return pin;
  }

  private async releasePin(pin: SourceDocumentRevisionPin): Promise<void> {
    const released = await this.api.releaseRevisionPin({ pin_id: pin.pin_id });
    if (released.status !== 'released' || released.pin_id !== pin.pin_id) {
      throw new Error('Revision pin release mismatch');
    }
  }

  private async releasePinBestEffort(pin: SourceDocumentRevisionPin): Promise<void> {
    try {
      await this.releasePin(pin);
    } catch {
      this.operationError = 'document_pin_release_failed';
      this.publish();
    }
  }

  private async transitionToDocument(
    document: MaskedSourceDocument,
    pin: SourceDocumentRevisionPin,
    installedSource: InstalledSource | null,
  ): Promise<void> {
    await this.clearCredentialPlaintextState();
    const oldPin = this.currentPin;
    if (oldPin && oldPin.pin_id !== pin.pin_id) {
      try {
        await this.releasePin(oldPin);
      } catch (error) {
        await this.releasePinBestEffort(pin);
        this.operationError = 'document_pin_release_failed';
        this.publish();
        throw error;
      }
    }
    this.currentPin = pin;
    this.adoptDocument(document, installedSource);
    await this.syncEditorHost();
  }

  private adoptDocument(
    document: MaskedSourceDocument,
    installedSource: InstalledSource | null,
  ): void {
    this.documentGeneration += 1;
    this.cancelQueuedSaves();
    this.epochCounter += 1;
    this.authoringDocument = SourceAuthoringDocument.open(document.masked_text, this.epochCounter);
    this.summary = document.summary;
    this.credentialSlots = [...document.credential_slots];
    this.savedText = document.masked_text;
    this.savedRevision = document.summary.revision;
    this.modeValue = 'original';
    this.installedSource = installedSource;
    this.lockedValue = false;
    this.conflictState = null;
    this.conflictResolutionHolder.clear();
    this.undoStack = [];
    this.redoStack = [];
    this.invalidatePreparedState();
    this.operationError = null;
    this.upsertDocumentSummary(document.summary);
    this.publish();
  }

  private async clearDocumentState(): Promise<void> {
    this.documentGeneration += 1;
    this.summary = null;
    this.credentialSlots = [];
    this.authoringDocument = null;
    this.savedText = '';
    this.savedRevision = 0;
    this.modeValue = 'original';
    this.installedSource = null;
    this.currentPin = null;
    this.conflictState = null;
    this.conflictResolutionHolder.clear();
    this.undoStack = [];
    this.redoStack = [];
    this.invalidatePreparedState();
    this.editorUnsubscribe?.();
    this.editorUnsubscribe = null;
    this.editorHostState?.dispose();
    this.editorHostState = null;
    this.lastEditorStatus = null;
    this.publish();
  }

  private acceptAuthoringEdit(before: string, result: SourceAuthoringEditResult): void {
    if (result.kind !== 'applied') {
      if (result.kind === 'rejected') this.publish();
      return;
    }
    this.undoStack.push({ before, after: result.text });
    if (this.undoStack.length > this.historyLimit) this.undoStack.shift();
    this.redoStack = [];
    this.afterTextTransition(result.text);
  }

  private afterTextTransition(text: string): void {
    this.epochCounter = this.authoringDocument?.epoch ?? this.epochCounter;
    this.conflictResolutionHolder.clear();
    this.invalidatePreparedState();
    this.operationError = null;
    this.publish();
    void this.syncEditorHost(text).catch(() => {
      this.operationError = 'editor_sync_failed';
      this.publish();
    });
  }

  private invalidatePreparedState(): void {
    this.prepareGeneration += 1;
    this.preparePending = false;
    this.prepareDiagnostics = [];
    this.candidateValue = null;
    this.grantValue = 'none';
  }

  private async drainSaveQueue(): Promise<void> {
    if (this.saveActive || this.disposedValue) return;
    this.saveActive = true;
    this.publish();
    try {
      while (this.saveQueue.length > 0 && !this.disposedValue) {
        const intent = this.saveQueue.shift()!;
        if (
          intent.documentGeneration !== this.documentGeneration ||
          !this.summary ||
          !this.authoringDocument
        ) {
          intent.resolve({ status: 'cancelled' });
          continue;
        }
        if (this.conflictState) {
          intent.resolve(blocked('conflict_unresolved'));
          continue;
        }
        if (intent.payloadText === this.savedText) {
          intent.resolve({ status: 'unchanged', document: this.currentMaskedDocument()! });
          continue;
        }

        const expectedRevision = this.savedRevision;
        this.publish();
        let outcome: DocumentMutationOutcome;
        try {
          outcome = await this.api.saveDocument({
            document_id: this.summary.document_id,
            expected_revision: expectedRevision,
            masked_text: intent.payloadText,
          });
        } catch (error) {
          this.operationError = 'save_failed';
          intent.reject(error);
          this.cancelQueuedSaves();
          break;
        }

        if (intent.documentGeneration !== this.documentGeneration) {
          intent.resolve({ status: 'cancelled' });
          continue;
        }
        await this.applySaveOutcome(intent, expectedRevision, outcome);
        intent.resolve(outcome);
        if (outcome.status !== 'saved' || outcome.document === null) {
          this.cancelQueuedSaves(
            outcome.status === 'conflict' ? 'conflict_unresolved' : 'document_invalid',
          );
          break;
        }
      }
    } finally {
      this.saveActive = false;
      this.publish();
    }
  }

  private async applySaveOutcome(
    intent: PendingSaveIntent,
    expectedRevision: number,
    outcome: DocumentMutationOutcome,
  ): Promise<void> {
    if (outcome.status === 'saved' && outcome.document) {
      await this.clearRevealedCredentials();
      if (intent.documentGeneration !== this.documentGeneration || !this.authoringDocument) return;

      const saved = outcome.document;
      const reconciledCurrent = reconcileCredentialSentinels(
        this.authoringDocument.text,
        this.authoringDocument.epoch,
        intent.payloadText,
        intent.credentialSlots,
        saved,
      );
      const reconciledQueued = this.saveQueue.map((queuedIntent) => ({
        intent: queuedIntent,
        document: reconcileCredentialSentinels(
          queuedIntent.payloadText,
          queuedIntent.requestEpoch,
          intent.payloadText,
          queuedIntent.credentialSlots,
          saved,
        ),
      }));
      const reconciliationFailed =
        reconciledCurrent === null || reconciledQueued.some((entry) => entry.document === null);

      this.summary = saved.summary;
      this.credentialSlots = [...saved.credential_slots];
      this.savedText = saved.masked_text;
      this.savedRevision = saved.summary.revision;
      this.conflictResolutionHolder.clear();
      this.upsertDocumentSummary(saved.summary);
      if (saved.masked_text !== intent.payloadText) {
        this.undoStack = [];
        this.redoStack = [];
      }

      if (reconciliationFailed) {
        this.cancelQueuedSaves('document_snapshot_unavailable');
        this.enterSentinelRotationConflict(saved);
      } else {
        this.authoringDocument = reconciledCurrent;
        this.epochCounter = reconciledCurrent.epoch;
        for (const entry of reconciledQueued) {
          entry.intent.payloadText = entry.document!.text;
          entry.intent.credentialSlots = [...saved.credential_slots];
        }
        this.conflictState = null;
        this.publish();
        try {
          await this.syncEditorHost();
        } catch {
          this.operationError = 'editor_sync_failed';
        }
      }
      await this.refreshPinAfterMutation(saved);
      this.publish();
      return;
    }
    if (outcome.status === 'conflict') {
      this.enterConflict(expectedRevision, pathsFromSlots(intent.credentialSlots), outcome.current);
      return;
    }
    if (outcome.status === 'locked') await this.lock();
  }

  private async refreshPinAfterMutation(document: MaskedSourceDocument): Promise<void> {
    try {
      const nextPin = await this.pinDocument(document);
      const oldPin = this.currentPin;
      this.currentPin = nextPin;
      if (oldPin && oldPin.pin_id !== nextPin.pin_id) await this.releasePinBestEffort(oldPin);
    } catch {
      this.operationError = 'document_pin_failed';
    }
  }

  private enterConflict(
    baseRevision: number,
    baseCredentialPaths: readonly string[],
    current: MaskedSourceDocument,
  ): void {
    this.conflictState = {
      kind: 'revision',
      baseRevision,
      baseCredentialPaths,
      current,
      dismissed: false,
    };
    this.conflictResolutionHolder.clear();
    this.invalidatePreparedState();
    this.publish();
  }

  private enterSentinelRotationConflict(current: MaskedSourceDocument): void {
    this.conflictState = {
      kind: 'sentinel_rotation',
      baseRevision: current.summary.revision,
      baseCredentialPaths: pathsFromSlots(current.credential_slots),
      current,
      dismissed: false,
    };
    this.conflictResolutionHolder.clear();
    this.invalidatePreparedState();
  }

  private enterSnapshotUnavailableConflict(
    baseRevision: number,
    current: MaskedSourceDocument,
  ): void {
    this.conflictState = {
      kind: 'snapshot_unavailable',
      baseRevision,
      baseCredentialPaths: [],
      current,
      dismissed: false,
    };
    this.conflictResolutionHolder.clear();
    this.invalidatePreparedState();
    this.publish();
  }

  private conflictCredentialPaths(): readonly string[] {
    const conflict = this.conflictState;
    if (!conflict) return [];
    return uniqueSorted([
      ...conflict.baseCredentialPaths,
      ...pathsFromSlots(conflict.current.credential_slots),
    ]);
  }

  private conflictCredentialSnapshot(): readonly RuleEditorConflictCredentialSnapshot[] {
    const conflict = this.conflictState;
    if (!conflict) return [];
    const currentPaths = pathsFromSlots(conflict.current.credential_slots);
    return Object.freeze(
      this.conflictCredentialPaths().map((path) => ({
        path,
        base_present: conflict.baseCredentialPaths.includes(path),
        current_present: currentPaths.includes(path),
        resolution: this.conflictResolutionHolder.get(path)?.kind ?? null,
      })),
    );
  }

  private async rebaseConflict(
    mode: RebaseSourceDocumentRequest['mode'],
  ): Promise<DocumentMutationOutcome | RuleEditorBlockedResult> {
    const conflict = this.conflictState;
    const pin = this.currentPin;
    const document = this.authoringDocument;
    if (!conflict) return blocked('conflict_unresolved');
    if (conflict.kind !== 'revision') return blocked('document_snapshot_unavailable');
    if (!pin || !document || !this.summary) return blocked('conflict_unresolved');
    if (
      pin.document_id !== this.summary.document_id ||
      pin.document_revision !== conflict.baseRevision
    ) {
      return blocked('document_snapshot_unavailable');
    }
    const paths = this.conflictCredentialPaths();
    if (
      paths.length !== this.conflictResolutionHolder.size ||
      paths.some((path) => !this.conflictResolutionHolder.has(path))
    ) {
      return blocked('credential_resolutions_unresolved');
    }

    const credentialResolutions: SourceDocumentCredentialResolution[] = paths.map((path) => ({
      path,
      action: this.conflictResolutionHolder.get(path)!,
    }));
    const request: RebaseSourceDocumentRequest = {
      pin_id: pin.pin_id,
      document_id: this.summary.document_id,
      base_revision: conflict.baseRevision,
      current_revision: conflict.current.summary.revision,
      local_masked_text: document.text,
      mode,
      credential_resolutions: credentialResolutions,
    };

    this.invalidatePreparedState();
    this.rebasePending = true;
    this.operationError = null;
    this.publish();
    try {
      const outcome = await this.api.rebaseDocument(request);
      this.conflictResolutionHolder.clear();
      if (outcome.status === 'saved' && outcome.document) {
        const nextPin = await this.pinDocument(outcome.document);
        await this.transitionToDocument(outcome.document, nextPin, this.installedSource);
      } else if (outcome.status === 'conflict') {
        conflict.current = outcome.current;
        conflict.dismissed = false;
        this.publish();
      } else if (outcome.status === 'locked') {
        await this.lock();
      }
      return outcome;
    } catch (error) {
      this.conflictResolutionHolder.clear();
      this.operationError = 'rebase_failed';
      throw error;
    } finally {
      this.rebasePending = false;
      this.publish();
    }
  }

  private async mutateCredential(
    slotId: string,
    action: { readonly kind: 'replace'; readonly value: string } | { readonly kind: 'clear' },
  ): Promise<DocumentMutationOutcome | RuleEditorBlockedResult> {
    if (this.disposedValue) return blocked('disposed');
    if (this.lockedValue) return blocked('locked');
    const current = this.currentDocumentRef();
    if (!current) return blocked('no_document');
    if (this.snapshotState.dirty) return blocked('dirty');
    if (this.conflictState) return blocked('conflict_unresolved');
    if (!this.credentialSlots.some((slot) => slot.slot_id === slotId)) {
      return blocked('document_snapshot_unavailable');
    }

    this.invalidatePreparedState();
    this.operationError = null;
    this.publish();
    await this.clearRevealedCredentials();
    const target: SourceDocumentCredentialTarget = {
      document_id: current.document_id,
      document_revision: current.document_revision,
      slot_id: slotId,
    };
    try {
      const outcome =
        action.kind === 'replace'
          ? await this.api.replaceCredential({ target, value: action.value })
          : await this.api.clearCredential({ target });
      if (outcome.status === 'saved' && outcome.document) {
        const pin = await this.pinDocument(outcome.document);
        await this.transitionToDocument(outcome.document, pin, this.installedSource);
      } else if (outcome.status === 'conflict') {
        this.enterConflict(
          current.document_revision,
          pathsFromSlots(this.credentialSlots),
          outcome.current,
        );
      } else if (outcome.status === 'locked') {
        await this.lock();
      }
      return outcome;
    } catch (error) {
      this.operationError = 'credential_mutation_failed';
      this.publish();
      throw error;
    }
  }

  private async resolveLeaveOnce(
    action: RuleEditorLeaveAction,
  ): Promise<RuleEditorLeaveResolution> {
    if (action === 'continue') {
      this.focusEditor();
      return 'resolved';
    }
    try {
      if (action === 'save') {
        const result = await this.save();
        if (result.status !== 'saved' && result.status !== 'unchanged') {
          this.focusEditor();
          return 'blocked';
        }
        if (this.snapshotState.dirty || this.conflictState) {
          this.focusEditor();
          return 'blocked';
        }
      } else if (this.authoringDocument) {
        this.documentGeneration += 1;
        this.cancelQueuedSaves();
        const result = this.authoringDocument.replaceText({
          expectedEpoch: this.authoringDocument.epoch,
          text: this.savedText,
        });
        if (result.kind === 'applied') this.epochCounter = result.epoch;
        this.conflictState = null;
        this.conflictResolutionHolder.clear();
        this.undoStack = [];
        this.redoStack = [];
        this.invalidatePreparedState();
        this.publish();
        await this.syncEditorHost();
      }

      await this.clearRevealedCredentials();
      const pin = this.currentPin;
      if (pin) await this.releasePin(pin);
      this.currentPin = null;
      this.publish();
      return 'resolved';
    } catch {
      this.operationError = 'leave_failed';
      this.publish();
      this.focusEditor();
      return 'blocked';
    }
  }

  private cancelQueuedSaves(reason?: RuleEditorBlockedReason): void {
    while (this.saveQueue.length > 0) {
      const intent = this.saveQueue.shift()!;
      intent.resolve(reason ? blocked(reason) : { status: 'cancelled' });
    }
  }

  private upsertDocumentSummary(summary: SourceDocumentSummary): void {
    const index = this.documents.findIndex((entry) => entry.document_id === summary.document_id);
    if (index === -1) this.documents = [summary, ...this.documents];
    else
      this.documents = this.documents.map((entry, entryIndex) =>
        entryIndex === index ? summary : entry,
      );
  }

  private async syncEditorHost(text = this.authoringDocument?.text ?? ''): Promise<void> {
    if (!this.authoringDocument || !this.summary || this.disposedValue) return;
    const input: SourceEditorDocumentInput = {
      documentId: this.summary.document_id,
      text,
      businessEpoch: this.authoringDocument.epoch,
    };
    if (!this.editorHostState) {
      const host = this.createEditorHost(input, (nextText) => {
        this.editText(nextText);
      });
      this.editorHostState = host;
      this.editorUnsubscribe = host.subscribe((snapshot) => {
        const enteredFallback =
          snapshot.status === 'fallback' && this.lastEditorStatus !== 'fallback';
        this.lastEditorStatus = snapshot.status;
        if (enteredFallback) {
          const hadConflictResolutions = this.conflictResolutionHolder.size > 0;
          this.conflictResolutionHolder.clear();
          if (hadConflictResolutions) this.publish();
          void this.clearRevealedCredentials();
        }
      });
      this.publish();
      return;
    }
    await this.editorHostState.setDocument(input);
  }

  private publish(): void {
    const document = this.currentMaskedDocument();
    const authoring = this.authoringDocument;
    const conflict = this.conflictState;
    const snapshot: RuleEditorSessionSnapshot = Object.freeze({
      documents: Object.freeze([...this.documents]),
      documents_loading: this.documentsLoading,
      document,
      text: authoring?.text ?? '',
      saved_text: this.savedText,
      saved_revision: this.savedRevision,
      dirty: Boolean(authoring && authoring.text !== this.savedText),
      document_epoch: authoring?.epoch ?? this.epochCounter,
      mode: this.modeValue,
      diagnostics: Object.freeze([...(authoring?.diagnostics ?? [])]),
      history: Object.freeze({
        can_undo: this.undoStack.length > 0,
        can_redo: this.redoStack.length > 0,
      }),
      save_phase: this.saveActive ? (this.saveQueue.length > 0 ? 'queued' : 'saving') : 'idle',
      queued_save_count: this.saveQueue.length,
      prepare_pending: this.preparePending,
      prepare_diagnostics: Object.freeze([...this.prepareDiagnostics]),
      candidate: this.candidateValue,
      grant: this.grantValue,
      install_pending: this.installPending,
      installed_source: this.installedSource,
      revealed_slot_ids: Object.freeze([...this.revealedSlotIds]),
      conflict: conflict
        ? Object.freeze({
            kind: conflict.kind,
            reload_required: conflict.kind !== 'revision',
            base_revision: conflict.baseRevision,
            current_revision: conflict.current.summary.revision,
            current: conflict.current,
            dismissed: conflict.dismissed,
            credentials: this.conflictCredentialSnapshot(),
          })
        : null,
      rebase_pending: this.rebasePending,
      pinned_revision: this.currentPin
        ? Object.freeze({
            document_id: this.currentPin.document_id,
            document_revision: this.currentPin.document_revision,
            expires_at_ms: this.currentPin.expires_at_ms,
          })
        : null,
      locked: this.lockedValue,
      operation_error: this.operationError,
      disposed: this.disposedValue,
    });
    this.snapshotState = snapshot;
    for (const subscriber of this.subscribers) subscriber(snapshot);
  }
}
