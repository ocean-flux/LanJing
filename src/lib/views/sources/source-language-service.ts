import {
  SOURCE_LANGUAGE_PROTOCOL_VERSION,
  containsSourceCredentialMarker,
  inspectSourceLanguageText,
  isSourceLanguageMethod,
  type SourceLanguageErrorCode,
  type SourceLanguageMethod,
  type SourceLanguageRequest,
  type SourceLanguageRequestInput,
  type SourceLanguageResultMap,
  type SourceLanguageVersion,
  type SourceLanguageWorkerInput,
} from './source-language-service-protocol';

export interface SourceLanguageWorker {
  postMessage(message: SourceLanguageWorkerInput): void;
  addEventListener(type: string, listener: EventListenerOrEventListenerObject): void;
  removeEventListener(type: string, listener: EventListenerOrEventListenerObject): void;
  terminate(): void;
}

export type SourceLanguageWorkerFactory = () => SourceLanguageWorker;

export interface SourceLanguageServiceClientOptions {
  readonly workerFactory?: SourceLanguageWorkerFactory;
  readonly onFailure?: (error: SourceLanguageServiceClientError) => void;
}

export interface SourceLanguageService {
  setDocument(version: SourceLanguageVersion): void;
  request<M extends SourceLanguageMethod>(
    input: SourceLanguageRequestInput<M>,
    options?: { readonly signal?: AbortSignal },
  ): Promise<SourceLanguageResultMap[M]>;
  dispose(): void;
}

interface PendingRequest {
  readonly request: SourceLanguageRequest;
  readonly resolve: (value: unknown) => void;
  readonly reject: (reason: SourceLanguageServiceClientError) => void;
  readonly removeAbortListener?: () => void;
}

const ERROR_MESSAGE_BY_CODE: Record<SourceLanguageErrorCode, string> = {
  cancelled: 'Source language request was cancelled',
  disposed: 'Source language service was disposed',
  document_too_large: 'Source document exceeds the authoring byte limit',
  document_too_complex: 'Source document exceeds the authoring complexity limits',
  document_version_conflict: 'Source text does not match its document version',
  invalid_request: 'Source language request is invalid',
  protocol_mismatch: 'Source language protocol version does not match',
  stale_document: 'Source language result is stale',
  unsafe_credential_marker: 'Source language result contained a credential marker',
  worker_failed: 'Source language worker failed',
};

export class SourceLanguageServiceClientError extends Error {
  readonly code: SourceLanguageErrorCode;

  constructor(code: SourceLanguageErrorCode) {
    super(ERROR_MESSAGE_BY_CODE[code]);
    this.name = 'SourceLanguageServiceClientError';
    this.code = code;
  }
}

export function createSourceLanguageWorker(): SourceLanguageWorker {
  if (typeof Worker === 'undefined') {
    throw new SourceLanguageServiceClientError('worker_failed');
  }
  return new Worker(new URL('./source-language-service.worker.ts', import.meta.url), {
    type: 'module',
    name: 'lanjing-source-language-service',
  }) as unknown as SourceLanguageWorker;
}

export function createSourceLanguageServiceClient(
  options: SourceLanguageServiceClientOptions = {},
): SourceLanguageServiceClient {
  return new SourceLanguageServiceClient(options);
}

export class SourceLanguageServiceClient implements SourceLanguageService {
  readonly #worker: SourceLanguageWorker;
  readonly #pending = new Map<string, PendingRequest>();
  #currentVersion: SourceLanguageVersion | null = null;
  #nextRequestNumber = 1;
  #disposed = false;
  #failed = false;
  readonly #onFailure: ((error: SourceLanguageServiceClientError) => void) | undefined;

  readonly #handleMessage = (event: Event): void => {
    const candidate = (event as MessageEvent<unknown>).data;
    if (!candidate || typeof candidate !== 'object') return;
    const result = candidate as Record<string, unknown>;
    if (result.type !== 'result' || typeof result.requestId !== 'string') return;

    const pending = this.#pending.get(result.requestId);
    if (!pending) return;

    if (result.protocolVersion !== SOURCE_LANGUAGE_PROTOCOL_VERSION) {
      this.#settleWithError(result.requestId, 'protocol_mismatch');
      return;
    }
    if (
      result.documentId !== pending.request.documentId ||
      result.documentEpoch !== pending.request.documentEpoch ||
      result.modelVersion !== pending.request.modelVersion ||
      result.method !== pending.request.method ||
      !this.#matchesCurrentVersion(pending.request)
    ) {
      this.#settleWithError(result.requestId, 'stale_document');
      return;
    }

    if (result.ok === true && 'result' in result && containsSourceCredentialMarker(result.result)) {
      this.#settleWithError(result.requestId, 'unsafe_credential_marker');
      return;
    }

    this.#pending.delete(result.requestId);
    pending.removeAbortListener?.();
    if (result.ok === true && 'result' in result) {
      pending.resolve(result.result);
      return;
    }
    if (result.ok === false && result.error && typeof result.error === 'object') {
      const code = (result.error as { readonly code?: unknown }).code;
      if (typeof code === 'string' && Object.hasOwn(ERROR_MESSAGE_BY_CODE, code)) {
        pending.reject(new SourceLanguageServiceClientError(code as SourceLanguageErrorCode));
        return;
      }
    }
    pending.reject(new SourceLanguageServiceClientError('worker_failed'));
  };

  readonly #handleWorkerFailure = (): void => {
    if (this.#disposed || this.#failed) return;
    this.#failed = true;
    const error = new SourceLanguageServiceClientError('worker_failed');
    this.#worker.removeEventListener('message', this.#handleMessage);
    this.#worker.removeEventListener('error', this.#handleWorkerFailure);
    this.#worker.removeEventListener('messageerror', this.#handleWorkerFailure);
    this.#worker.terminate();
    this.#rejectAll('worker_failed');
    try {
      this.#onFailure?.(error);
    } catch {
      // Consumer failure handling must not escape the worker event boundary.
    }
  };

  constructor(options: SourceLanguageServiceClientOptions = {}) {
    this.#onFailure = options.onFailure;
    this.#worker = (options.workerFactory ?? createSourceLanguageWorker)();
    this.#worker.addEventListener('message', this.#handleMessage);
    this.#worker.addEventListener('error', this.#handleWorkerFailure);
    this.#worker.addEventListener('messageerror', this.#handleWorkerFailure);
  }

  get pendingRequestCount(): number {
    return this.#pending.size;
  }

  get isDisposed(): boolean {
    return this.#disposed;
  }

  setDocument(version: SourceLanguageVersion): void {
    this.#assertUsable();
    this.#assertVersion(version);

    const current = this.#currentVersion;
    if (
      current?.documentId === version.documentId &&
      (version.documentEpoch < current.documentEpoch ||
        (version.documentEpoch === current.documentEpoch &&
          version.modelVersion < current.modelVersion))
    ) {
      throw new SourceLanguageServiceClientError('stale_document');
    }
    if (
      current?.documentId === version.documentId &&
      current.documentEpoch === version.documentEpoch &&
      current.modelVersion === version.modelVersion
    ) {
      return;
    }

    this.#currentVersion = { ...version };
    for (const [requestId, pending] of this.#pending) {
      if (!this.#matchesCurrentVersion(pending.request)) {
        this.#cancelPending(requestId, 'stale_document');
      }
    }
  }

  request<M extends SourceLanguageMethod>(
    input: SourceLanguageRequestInput<M>,
    options: { readonly signal?: AbortSignal } = {},
  ): Promise<SourceLanguageResultMap[M]> {
    try {
      this.#assertUsable();
      if (
        !isSourceLanguageMethod(input.method) ||
        !input.payload ||
        typeof input.payload.text !== 'string'
      ) {
        throw new SourceLanguageServiceClientError('invalid_request');
      }
      this.setDocument(input);
    } catch (error) {
      return Promise.reject(
        error instanceof SourceLanguageServiceClientError
          ? error
          : new SourceLanguageServiceClientError('invalid_request'),
      );
    }

    if (options.signal?.aborted) {
      return Promise.reject(new SourceLanguageServiceClientError('cancelled'));
    }

    const limit = inspectSourceLanguageText(input.payload.text);
    if (limit) {
      return Promise.reject(new SourceLanguageServiceClientError(limit));
    }

    const requestId = `source-language-${this.#nextRequestNumber}`;
    this.#nextRequestNumber += 1;
    const request = {
      type: 'request',
      protocolVersion: SOURCE_LANGUAGE_PROTOCOL_VERSION,
      requestId,
      documentId: input.documentId,
      documentEpoch: input.documentEpoch,
      modelVersion: input.modelVersion,
      method: input.method,
      payload: input.payload,
    } as SourceLanguageRequest;

    return new Promise<SourceLanguageResultMap[M]>((resolve, reject) => {
      let removeAbortListener: (() => void) | undefined;
      if (options.signal) {
        const handleAbort = (): void => {
          this.#cancelPending(requestId, 'cancelled');
        };
        options.signal.addEventListener('abort', handleAbort, { once: true });
        removeAbortListener = () => options.signal?.removeEventListener('abort', handleAbort);
      }

      this.#pending.set(requestId, {
        request,
        resolve: (value) => resolve(value as SourceLanguageResultMap[M]),
        reject,
        ...(removeAbortListener ? { removeAbortListener } : {}),
      });

      try {
        this.#worker.postMessage(request);
      } catch {
        this.#handleWorkerFailure();
      }
    });
  }

  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    if (!this.#failed) {
      try {
        this.#worker.postMessage({
          type: 'dispose',
          protocolVersion: SOURCE_LANGUAGE_PROTOCOL_VERSION,
        });
      } catch {
        // worker may already be unavailable; local teardown still completes.
      }
    }
    this.#worker.removeEventListener('message', this.#handleMessage);
    this.#worker.removeEventListener('error', this.#handleWorkerFailure);
    this.#worker.removeEventListener('messageerror', this.#handleWorkerFailure);
    this.#rejectAll('disposed');
    this.#currentVersion = null;
    if (!this.#failed) this.#worker.terminate();
  }

  #assertUsable(): void {
    if (this.#disposed) throw new SourceLanguageServiceClientError('disposed');
    if (this.#failed) throw new SourceLanguageServiceClientError('worker_failed');
  }

  #assertVersion(version: SourceLanguageVersion): void {
    if (
      typeof version.documentId !== 'string' ||
      version.documentId.length === 0 ||
      !Number.isSafeInteger(version.documentEpoch) ||
      version.documentEpoch < 0 ||
      !Number.isSafeInteger(version.modelVersion) ||
      version.modelVersion < 0
    ) {
      throw new SourceLanguageServiceClientError('invalid_request');
    }
  }

  #matchesCurrentVersion(version: SourceLanguageVersion): boolean {
    const current = this.#currentVersion;
    return (
      current !== null &&
      current.documentId === version.documentId &&
      current.documentEpoch === version.documentEpoch &&
      current.modelVersion === version.modelVersion
    );
  }

  #cancelPending(requestId: string, code: 'cancelled' | 'stale_document'): void {
    const pending = this.#pending.get(requestId);
    if (!pending) return;
    let workerFailed = false;
    try {
      this.#worker.postMessage({
        type: 'cancel',
        protocolVersion: SOURCE_LANGUAGE_PROTOCOL_VERSION,
        requestId,
        documentId: pending.request.documentId,
        documentEpoch: pending.request.documentEpoch,
        modelVersion: pending.request.modelVersion,
      });
    } catch {
      workerFailed = true;
    }
    this.#settleWithError(requestId, code);
    if (workerFailed) this.#handleWorkerFailure();
  }

  #settleWithError(requestId: string, code: SourceLanguageErrorCode): void {
    const pending = this.#pending.get(requestId);
    if (!pending) return;
    this.#pending.delete(requestId);
    pending.removeAbortListener?.();
    pending.reject(new SourceLanguageServiceClientError(code));
  }

  #rejectAll(code: SourceLanguageErrorCode): void {
    for (const requestId of this.#pending.keys()) {
      this.#settleWithError(requestId, code);
    }
  }
}
