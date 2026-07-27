import type { RuntimePlatform } from '$lib/app/platform-runtime';
import { loadSourceEditorAdapter, type SourceEditorAdapterLoader } from './source-editor-loaders';
import {
  createSourceLanguageServiceClient,
  SourceLanguageServiceClientError,
  type SourceLanguageService,
} from './source-language-service';
import {
  inspectSourceLanguageText,
  type SourceLanguageInputLimitCode,
  type SourceLanguageMethod,
  type SourceLanguageRequestPayloadMap,
  type SourceLanguageRequestWithoutText,
  type SourceLanguageResultMap,
  type SourceLanguageVersion,
} from './source-language-service-protocol';
import {
  resolveEditorKind,
  type EditorAdapter,
  type EditorCommand,
  type EditorKind,
  type EditorSelection,
} from './source-editor-adapters';

export type SourceEditorStatus = 'idle' | 'loading' | 'ready' | 'fallback' | 'disposed';
export type SourceEditorFallbackReason =
  'unsupported-platform' | 'load-failed' | 'mount-failed' | 'runtime-error';
export type SourceEditorFocusTarget = 'editor' | 'fallback' | 'retry';

/** 受控输入只接受 RuleEditorSession 持有的遮罩正文。 */
export interface SourceEditorDocumentInput {
  readonly documentId: string;
  readonly text: string;
  readonly businessEpoch: number;
}

export interface SourceDocumentEditorSnapshot {
  readonly documentId: string;
  readonly text: string;
  readonly businessEpoch: number;
  readonly adapterModelVersion: number;
  readonly languageGeneration: number;
  readonly languageInputLimit: SourceLanguageInputLimitCode | null;
  readonly editorKind: EditorKind;
  readonly status: SourceEditorStatus;
  readonly fallbackReason?: SourceEditorFallbackReason;
  readonly advancedStateReset: boolean;
  readonly focusTarget: SourceEditorFocusTarget;
  readonly focusRequestId: number;
}

export type SourceEditorLanguageServiceFactory = (
  onFailure: (error: SourceLanguageServiceClientError) => void,
) => SourceLanguageService;

export interface SourceDocumentEditorHostOptions {
  readonly loadAdapter?: SourceEditorAdapterLoader;
  readonly createLanguageService?: SourceEditorLanguageServiceFactory;
  readonly onTextEdit: (text: string) => void;
  readonly onSelectionChange?: (selection: EditorSelection) => void;
}

type SourceDocumentEditorSubscriber = (snapshot: SourceDocumentEditorSnapshot) => void;

interface PendingTransition {
  readonly operationId: number;
  readonly promise: Promise<void>;
  readonly resolve: () => void;
  started: boolean;
}

function assertNonNegativeSafeInteger(value: number, name: string): void {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new RangeError(`${name} must be a non-negative safe integer`);
  }
}

export class SourceDocumentEditorHost {
  private readonly loadAdapter: SourceEditorAdapterLoader;
  private readonly textEditCallback: (text: string) => void;
  private readonly selectionCallback: ((selection: EditorSelection) => void) | undefined;
  private readonly createLanguageService: SourceEditorLanguageServiceFactory;
  private readonly subscribers = new Set<SourceDocumentEditorSubscriber>();

  private documentId: string;
  private text: string;
  private businessEpoch: number;
  private adapterModelVersion = 0;
  private languageGeneration = 0;
  private languageInputLimit: SourceLanguageInputLimitCode | null;
  private editorKind: EditorKind = 'fallback';
  private status: SourceEditorStatus = 'idle';
  private fallbackReason: SourceEditorFallbackReason | undefined;
  private advancedStateReset = false;
  private focusTarget: SourceEditorFocusTarget = 'editor';
  private focusRequestId = 0;
  private snapshot: SourceDocumentEditorSnapshot;

  private requestedPlatform: RuntimePlatform | null = null;
  private container: HTMLElement | null = null;
  private attachmentId = 0;
  private operationId = 0;
  private transition: PendingTransition | null = null;
  private activeAdapter: EditorAdapter | null = null;
  private languageService: SourceLanguageService | null = null;
  private focusPending = false;
  private disposed = false;

  constructor(document: SourceEditorDocumentInput, options: SourceDocumentEditorHostOptions) {
    this.assertDocument(document);
    this.documentId = document.documentId;
    this.text = document.text;
    this.businessEpoch = document.businessEpoch;
    this.languageInputLimit = inspectSourceLanguageText(document.text);
    this.loadAdapter = options.loadAdapter ?? loadSourceEditorAdapter;
    this.createLanguageService =
      options.createLanguageService ??
      ((onFailure) => createSourceLanguageServiceClient({ onFailure }));
    this.textEditCallback = options.onTextEdit;
    this.selectionCallback = options.onSelectionChange;
    this.snapshot = this.buildSnapshot();
  }

  getSnapshot(): SourceDocumentEditorSnapshot {
    return this.snapshot;
  }

  subscribe(subscriber: SourceDocumentEditorSubscriber): () => void {
    this.subscribers.add(subscriber);
    subscriber(this.snapshot);
    return () => {
      this.subscribers.delete(subscriber);
    };
  }

  attach(container: HTMLElement): () => void {
    this.assertUsable();
    const attachmentId = ++this.attachmentId;

    if (this.container && this.container !== container) {
      const requestedPlatform = this.requestedPlatform;
      const reloadAdvancedEditor =
        (this.status === 'loading' || this.status === 'ready') &&
        requestedPlatform !== null &&
        resolveEditorKind(requestedPlatform) !== 'fallback';
      this.invalidateOperation();
      this.disposeActiveAdapter();
      this.disposeLanguageService();
      if (reloadAdvancedEditor && requestedPlatform) {
        this.languageGeneration += 1;
        this.advancedStateReset = true;
        this.status = 'loading';
        this.editorKind = resolveEditorKind(requestedPlatform);
        this.fallbackReason = undefined;
        this.createTransition();
        this.publish();
      }
    }

    this.container = container;
    if (
      this.status === 'idle' &&
      this.requestedPlatform &&
      resolveEditorKind(this.requestedPlatform) !== 'fallback'
    ) {
      this.status = 'loading';
      this.editorKind = resolveEditorKind(this.requestedPlatform);
      this.fallbackReason = undefined;
      this.createTransition();
      this.publish();
    }
    this.beginLoadingIfPossible();

    return () => {
      if (this.disposed || this.attachmentId !== attachmentId || this.container !== container) {
        return;
      }

      this.container = null;
      this.invalidateOperation();
      this.disposeActiveAdapter();
      this.disposeLanguageService();
      if (this.status === 'loading' || this.status === 'ready') {
        this.languageGeneration += 1;
        this.advancedStateReset = true;
        this.status = 'idle';
        this.fallbackReason = undefined;
        this.publish();
      }
    };
  }

  start(platform: RuntimePlatform): Promise<void> {
    this.assertUsable();
    const restarting =
      this.requestedPlatform !== null && resolveEditorKind(this.requestedPlatform) !== 'fallback';
    this.invalidateOperation();
    this.disposeActiveAdapter();
    this.disposeLanguageService();

    if (restarting) {
      this.languageGeneration += 1;
      this.advancedStateReset = true;
    }

    this.requestedPlatform = platform;
    this.fallbackReason = undefined;
    this.focusTarget = 'editor';

    const kind = resolveEditorKind(platform);
    this.editorKind = kind;
    if (kind === 'fallback') {
      this.status = 'fallback';
      this.fallbackReason = 'unsupported-platform';
      this.focusTarget = 'fallback';
      this.publish();
      return Promise.resolve();
    }

    this.status = 'loading';
    const transition = this.createTransition();
    this.publish();
    this.beginLoadingIfPossible();
    return transition.promise;
  }

  setDocument(document: SourceEditorDocumentInput): Promise<void> {
    this.assertUsable();
    this.assertDocument(document);

    if (document.documentId === this.documentId) {
      if (document.businessEpoch < this.businessEpoch) {
        throw new RangeError('businessEpoch cannot move backwards for the current document');
      }

      const epochChanged = document.businessEpoch !== this.businessEpoch;
      const textChanged = document.text !== this.text;
      if (!epochChanged && !textChanged) return Promise.resolve();

      this.businessEpoch = document.businessEpoch;
      if (!textChanged) {
        this.publish();
        return Promise.resolve();
      }

      const nextVersion = this.nextAdapterModelVersion();
      this.text = document.text;
      this.adapterModelVersion = nextVersion;
      this.languageInputLimit = inspectSourceLanguageText(document.text);
      this.publish();
      this.synchronizeLanguageServiceVersion();

      const adapter = this.activeAdapter;
      if (adapter && this.status === 'ready') {
        try {
          adapter.setDocument(this.documentId, document.text, nextVersion);
        } catch (error) {
          this.handleRuntimeError(error, this.operationId, adapter);
        }
      }
      return Promise.resolve();
    }

    this.invalidateOperation();
    this.disposeActiveAdapter();
    this.disposeLanguageService();

    this.documentId = document.documentId;
    this.text = document.text;
    this.businessEpoch = document.businessEpoch;
    this.adapterModelVersion = 0;
    this.languageGeneration += 1;
    this.languageInputLimit = inspectSourceLanguageText(document.text);
    this.advancedStateReset = false;
    this.fallbackReason = undefined;
    this.focusTarget = 'editor';
    this.focusPending = false;

    if (!this.requestedPlatform) {
      this.status = 'idle';
      this.editorKind = 'fallback';
      this.publish();
      return Promise.resolve();
    }

    const kind = resolveEditorKind(this.requestedPlatform);
    this.editorKind = kind;
    if (kind === 'fallback') {
      this.status = 'fallback';
      this.fallbackReason = 'unsupported-platform';
      this.focusTarget = 'fallback';
      this.publish();
      return Promise.resolve();
    }

    this.status = 'loading';
    const transition = this.createTransition();
    this.publish();
    this.beginLoadingIfPossible();
    return transition.promise;
  }

  /** 接收当前 editor surface 的遮罩正文，并把业务修改意图交还 session。 */
  editText(text: string): void {
    this.assertUsable();
    if (text === this.text) return;

    const nextVersion = this.nextAdapterModelVersion();
    this.text = text;
    this.adapterModelVersion = nextVersion;
    this.languageInputLimit = inspectSourceLanguageText(text);
    this.publish();
    this.synchronizeLanguageServiceVersion();

    const adapter = this.activeAdapter;
    if (adapter && this.status === 'ready') {
      try {
        adapter.setDocument(this.documentId, text, nextVersion);
      } catch (error) {
        this.handleRuntimeError(error, this.operationId, adapter);
      }
    }

    this.textEditCallback(text);
  }

  retry(): Promise<void> {
    this.assertUsable();
    if (!this.requestedPlatform || resolveEditorKind(this.requestedPlatform) === 'fallback') {
      return Promise.resolve();
    }

    this.invalidateOperation();
    this.disposeActiveAdapter();
    this.disposeLanguageService();
    this.languageGeneration += 1;
    this.advancedStateReset = true;
    this.status = 'loading';
    this.editorKind = resolveEditorKind(this.requestedPlatform);
    this.fallbackReason = undefined;
    this.focusTarget = 'editor';
    this.focusPending = true;

    const transition = this.createTransition();
    this.publish();
    this.beginLoadingIfPossible();
    return transition.promise;
  }

  focus(): void {
    this.assertUsable();
    const adapter = this.activeAdapter;
    if (adapter && this.status === 'ready') {
      this.focusTarget = 'editor';
      try {
        adapter.focus();
      } catch (error) {
        this.handleRuntimeError(error, this.operationId, adapter);
      }
      return;
    }

    if (this.status === 'loading') {
      this.focusTarget = 'editor';
      this.focusPending = true;
      return;
    }

    this.focusTarget = this.status === 'fallback' ? 'fallback' : 'editor';
    this.focusRequestId += 1;
    this.publish();
  }

  async runCommand(command: EditorCommand): Promise<boolean> {
    this.assertUsable();
    const adapter = this.activeAdapter;
    if (!adapter || this.status !== 'ready') return false;

    try {
      return await adapter.runCommand(command);
    } catch (error) {
      this.handleRuntimeError(error, this.operationId, adapter);
      return false;
    }
  }

  requestLanguage<M extends SourceLanguageMethod>(
    request: SourceLanguageRequestWithoutText<M>,
    options: { readonly signal?: AbortSignal } = {},
  ): Promise<SourceLanguageResultMap[M]> {
    this.assertUsable();
    if (this.status !== 'ready') {
      return Promise.reject(new Error('Source language service requires a ready advanced editor'));
    }
    if (this.languageInputLimit) {
      return Promise.reject(new SourceLanguageServiceClientError(this.languageInputLimit));
    }

    const version = this.currentLanguageVersion();
    let service: SourceLanguageService;
    try {
      service = this.ensureLanguageService();
    } catch (error) {
      const failure =
        error instanceof SourceLanguageServiceClientError
          ? error
          : new SourceLanguageServiceClientError('worker_failed');
      const adapter = this.activeAdapter;
      if (adapter) this.handleRuntimeError(failure, this.operationId, adapter);
      return Promise.reject(failure);
    }

    return service
      .request(
        {
          ...version,
          method: request.method,
          payload: {
            ...request.payload,
            text: this.text,
          } as SourceLanguageRequestPayloadMap[M],
        },
        options,
      )
      .catch((error: unknown) => {
        if (
          this.languageService === service &&
          version.documentId === this.documentId &&
          version.documentEpoch === this.languageGeneration &&
          version.modelVersion === this.adapterModelVersion &&
          error instanceof SourceLanguageServiceClientError &&
          (error.code === 'worker_failed' || error.code === 'protocol_mismatch')
        ) {
          this.handleLanguageServiceFailure(error, service);
        }
        throw error;
      });
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.invalidateOperation();
    this.disposeActiveAdapter();
    this.disposeLanguageService();
    this.container = null;
    this.requestedPlatform = null;
    this.status = 'disposed';
    this.editorKind = 'fallback';
    this.fallbackReason = undefined;
    this.focusPending = false;
    this.publish();
    this.subscribers.clear();
  }

  private beginLoadingIfPossible(): void {
    const transition = this.transition;
    const platform = this.requestedPlatform;
    const container = this.container;
    if (!transition || transition.started || this.status !== 'loading' || !platform || !container) {
      return;
    }

    transition.started = true;
    const operationId = transition.operationId;
    const expectedKind = resolveEditorKind(platform);

    void this.loadAdapter(platform).then(
      (adapter) => {
        if (!this.isCurrentOperation(operationId, container)) {
          if (adapter) this.disposeAdapter(adapter);
          return;
        }
        if (!adapter || adapter.kind !== expectedKind) {
          if (adapter) this.disposeAdapter(adapter);
          this.failLoading('load-failed', operationId);
          return;
        }

        this.activeAdapter = adapter;
        try {
          adapter.mount({
            container,
            documentId: this.documentId,
            text: this.text,
            modelVersion: this.adapterModelVersion,
            requestLanguage: (request, options) => this.requestLanguage(request, options),
            onTextChange: (text, modelVersion) => {
              this.acceptAdapterText(operationId, adapter, text, modelVersion);
            },
            onSelectionChange: (selection) => {
              this.acceptAdapterSelection(operationId, adapter, selection);
            },
            onRuntimeError: (error) => {
              this.handleRuntimeError(error, operationId, adapter);
            },
          });
        } catch {
          if (this.activeAdapter === adapter) this.activeAdapter = null;
          this.disposeAdapter(adapter);
          this.failLoading('mount-failed', operationId);
          return;
        }

        if (!this.isCurrentOperation(operationId, container) || this.activeAdapter !== adapter) {
          if (this.activeAdapter === adapter) this.activeAdapter = null;
          this.disposeAdapter(adapter);
          return;
        }

        this.status = 'ready';
        this.editorKind = adapter.kind;
        this.fallbackReason = undefined;
        this.finishTransition(operationId);
        this.publish();

        if (this.focusPending) {
          this.focusPending = false;
          try {
            adapter.focus();
          } catch (error) {
            this.handleRuntimeError(error, operationId, adapter);
          }
        }
      },
      () => {
        this.failLoading('load-failed', operationId);
      },
    );
  }

  private acceptAdapterText(
    operationId: number,
    adapter: EditorAdapter,
    text: string,
    modelVersion: number,
  ): void {
    if (
      this.disposed ||
      operationId !== this.operationId ||
      adapter !== this.activeAdapter ||
      (this.status !== 'loading' && this.status !== 'ready')
    ) {
      return;
    }
    if (modelVersion <= this.adapterModelVersion) return;
    if (modelVersion !== this.adapterModelVersion + 1) {
      this.handleRuntimeError(
        new Error('Editor adapter model version skipped'),
        operationId,
        adapter,
      );
      return;
    }

    this.text = text;
    this.adapterModelVersion = modelVersion;
    this.languageInputLimit = inspectSourceLanguageText(text);
    this.publish();
    this.synchronizeLanguageServiceVersion();
    this.textEditCallback(text);
  }

  private acceptAdapterSelection(
    operationId: number,
    adapter: EditorAdapter,
    selection: EditorSelection,
  ): void {
    if (
      this.disposed ||
      operationId !== this.operationId ||
      adapter !== this.activeAdapter ||
      this.status !== 'ready'
    ) {
      return;
    }
    this.focusTarget = 'editor';
    this.selectionCallback?.(selection);
  }

  private handleRuntimeError(error: unknown, operationId: number, adapter: EditorAdapter): void {
    void error;
    if (this.disposed || operationId !== this.operationId || adapter !== this.activeAdapter) {
      return;
    }

    this.operationId += 1;
    this.disposeLanguageService();
    this.finishTransition(operationId);
    this.activeAdapter = null;
    try {
      adapter.dispose();
    } catch {
      // adapter 故障后仍需原子切到 fallback。
    }
    this.languageGeneration += 1;
    this.status = 'fallback';
    this.editorKind = 'fallback';
    this.fallbackReason = 'runtime-error';
    this.advancedStateReset = true;
    this.focusTarget = 'fallback';
    this.focusRequestId += 1;
    this.focusPending = false;
    this.publish();
  }

  private failLoading(reason: SourceEditorFallbackReason, operationId: number): void {
    if (this.disposed || operationId !== this.operationId || this.status !== 'loading') return;
    this.operationId += 1;
    this.finishTransition(operationId);
    this.disposeActiveAdapter();
    this.disposeLanguageService();
    this.languageGeneration += 1;
    this.status = 'fallback';
    this.editorKind = 'fallback';
    this.fallbackReason = reason;
    this.advancedStateReset = true;
    this.focusTarget = 'fallback';
    this.focusRequestId += 1;
    this.focusPending = false;
    this.publish();
  }

  private createTransition(): PendingTransition {
    let resolve = (): void => undefined;
    const promise = new Promise<void>((done) => {
      resolve = done;
    });
    const transition: PendingTransition = {
      operationId: this.operationId,
      promise,
      resolve,
      started: false,
    };
    this.transition = transition;
    return transition;
  }

  private finishTransition(operationId: number): void {
    if (!this.transition || this.transition.operationId !== operationId) return;
    const transition = this.transition;
    this.transition = null;
    transition.resolve();
  }

  private invalidateOperation(): void {
    const transition = this.transition;
    this.transition = null;
    this.operationId += 1;
    transition?.resolve();
  }

  private disposeActiveAdapter(): void {
    const adapter = this.activeAdapter;
    this.activeAdapter = null;
    if (adapter) this.disposeAdapter(adapter);
  }

  private disposeAdapter(adapter: EditorAdapter): void {
    try {
      adapter.dispose();
    } catch {
      // 第三方 adapter 清理失败不能中断 host teardown。
    }
  }

  private isCurrentOperation(operationId: number, container: HTMLElement): boolean {
    return (
      !this.disposed &&
      operationId === this.operationId &&
      this.status === 'loading' &&
      this.container === container
    );
  }

  private currentLanguageVersion(): SourceLanguageVersion {
    return {
      documentId: this.documentId,
      documentEpoch: this.languageGeneration,
      modelVersion: this.adapterModelVersion,
    };
  }

  private ensureLanguageService(): SourceLanguageService {
    if (this.languageService) return this.languageService;

    let created: SourceLanguageService | null = null;
    created = this.createLanguageService((error) => {
      if (created) this.handleLanguageServiceFailure(error, created);
    });
    this.languageService = created;
    return created;
  }

  private synchronizeLanguageServiceVersion(): void {
    const service = this.languageService;
    if (!service) return;
    try {
      service.setDocument(this.currentLanguageVersion());
    } catch (error) {
      const failure =
        error instanceof SourceLanguageServiceClientError
          ? error
          : new SourceLanguageServiceClientError('worker_failed');
      this.handleLanguageServiceFailure(failure, service);
    }
  }

  private handleLanguageServiceFailure(
    error: SourceLanguageServiceClientError,
    service: SourceLanguageService,
  ): void {
    if (this.disposed || service !== this.languageService) return;
    this.disposeLanguageService();
    const adapter = this.activeAdapter;
    if (adapter && (this.status === 'loading' || this.status === 'ready')) {
      this.handleRuntimeError(error, this.operationId, adapter);
    }
  }

  private disposeLanguageService(): void {
    const service = this.languageService;
    this.languageService = null;
    if (!service) return;
    try {
      service.dispose();
    } catch {
      // worker 清理失败不能中断 host teardown。
    }
  }

  private nextAdapterModelVersion(): number {
    const nextVersion = this.adapterModelVersion + 1;
    assertNonNegativeSafeInteger(nextVersion, 'adapterModelVersion');
    return nextVersion;
  }

  private buildSnapshot(): SourceDocumentEditorSnapshot {
    return Object.freeze({
      documentId: this.documentId,
      text: this.text,
      businessEpoch: this.businessEpoch,
      adapterModelVersion: this.adapterModelVersion,
      languageGeneration: this.languageGeneration,
      languageInputLimit: this.languageInputLimit,
      editorKind: this.editorKind,
      status: this.status,
      ...(this.fallbackReason ? { fallbackReason: this.fallbackReason } : {}),
      advancedStateReset: this.advancedStateReset,
      focusTarget: this.focusTarget,
      focusRequestId: this.focusRequestId,
    });
  }

  private publish(): void {
    this.snapshot = this.buildSnapshot();
    for (const subscriber of this.subscribers) subscriber(this.snapshot);
  }

  private assertDocument(document: SourceEditorDocumentInput): void {
    if (document.documentId.length === 0) throw new TypeError('documentId cannot be empty');
    assertNonNegativeSafeInteger(document.businessEpoch, 'businessEpoch');
  }

  private assertUsable(): void {
    if (this.disposed) throw new Error('Source document editor host is disposed');
  }
}
