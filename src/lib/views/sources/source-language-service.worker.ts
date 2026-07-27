import sourceSchemaJson from '../../../../schemas/sources/legado/source.schema.v1.json';
import {
  authoringLimits,
  legadoFieldCatalog,
  parseJsonPointer,
  toJsonPointer,
} from '$lib/rules/authoring';
import {
  DiagnosticSeverity,
  SchemaDraft,
  getLanguageService,
  type ASTNode,
  type Diagnostic,
  type FormattingOptions,
  type JSONDocument,
  type JSONSchema,
  type LanguageService,
  type Position,
  type Range,
} from 'vscode-json-languageservice';
import {
  TextDocument,
  type TextDocument as TextDocumentModel,
} from 'vscode-languageserver-textdocument';
import {
  SOURCE_LANGUAGE_PROTOCOL_VERSION,
  containsSourceCredentialMarker as containsCredentialMarker,
  inspectSourceLanguageText,
  isSourceLanguageMethod,
  type SourceLanguageDiagnostic,
  type SourceLanguageDiagnosticSeverity,
  type SourceLanguageErrorCode,
  type SourceLanguageFailureResult,
  type SourceLanguageMethod,
  type SourceLanguageRequest,
  type SourceLanguageResultMap,
  type SourceLanguageSuccessResult,
  type SourceLanguageVersion,
  type SourceLanguageWorkerOutput,
} from './source-language-service-protocol';

const SCHEMA_URI = sourceSchemaJson.$id;
const CREDENTIAL_MARKER_REPLACEMENT = /__LANJING_[A-Z0-9_]*CREDENTIAL[A-Z0-9_]*__:[^\s"'\\]*/gi;

interface RequestEnvelope extends SourceLanguageVersion {
  readonly requestId: string;
  readonly method: SourceLanguageMethod;
}

interface CachedModel extends SourceLanguageVersion {
  readonly text: string;
  readonly document: TextDocumentModel;
  readonly jsonDocument: JSONDocument;
}

export interface SourceLanguageWorkerRuntime {
  readonly activeRequestCount: number;
  handleMessage(message: unknown): Promise<void>;
  dispose(): void;
}

class WorkerRequestError extends Error {
  readonly code: SourceLanguageErrorCode;

  constructor(code: SourceLanguageErrorCode) {
    super(code);
    this.name = 'WorkerRequestError';
    this.code = code;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object';
}

function isPosition(value: unknown): value is Position {
  return (
    isRecord(value) &&
    typeof value.line === 'number' &&
    Number.isSafeInteger(value.line) &&
    value.line >= 0 &&
    typeof value.character === 'number' &&
    Number.isSafeInteger(value.character) &&
    value.character >= 0
  );
}

function isRange(value: unknown): value is Range {
  return isRecord(value) && isPosition(value.start) && isPosition(value.end);
}

function readRequestEnvelope(value: Record<string, unknown>): RequestEnvelope | null {
  if (
    typeof value.requestId !== 'string' ||
    value.requestId.length === 0 ||
    typeof value.documentId !== 'string' ||
    value.documentId.length === 0 ||
    typeof value.documentEpoch !== 'number' ||
    !Number.isSafeInteger(value.documentEpoch) ||
    value.documentEpoch < 0 ||
    typeof value.modelVersion !== 'number' ||
    !Number.isSafeInteger(value.modelVersion) ||
    value.modelVersion < 0 ||
    !isSourceLanguageMethod(value.method)
  ) {
    return null;
  }
  return {
    requestId: value.requestId,
    documentId: value.documentId,
    documentEpoch: value.documentEpoch,
    modelVersion: value.modelVersion,
    method: value.method,
  };
}

function createLanguageSchema(): JSONSchema {
  const schema = sourceSchemaJson as unknown as JSONSchema;
  const properties = { ...(schema.properties ?? {}) };
  for (const field of legadoFieldCatalog.fields) {
    const pointer = parseJsonPointer(field.pointer);
    if (!pointer.ok || pointer.segments.length !== 1) continue;
    const propertyName = pointer.segments[0];
    const propertySchema = properties[propertyName];
    if (!propertySchema || typeof propertySchema === 'boolean') continue;
    properties[propertyName] = {
      ...propertySchema,
      description: field.hint,
      completionDetail: `Legado · ${field.group} · ${field.support}`,
    };
  }
  return { ...schema, properties };
}

const LANGUAGE_SCHEMA = createLanguageSchema();

function createLanguageService(): LanguageService {
  const service = getLanguageService({});
  service.configure({
    validate: true,
    allowComments: false,
    schemas: [
      {
        uri: SCHEMA_URI,
        fileMatch: ['*'],
        schema: LANGUAGE_SCHEMA,
      },
    ],
  });
  return service;
}

function redactCredentialMarkers(value: string): string {
  return value.replace(CREDENTIAL_MARKER_REPLACEMENT, '[masked credential]');
}

function pathForDiagnostic(jsonDocument: JSONDocument, offset: number): string {
  let node = jsonDocument.getNodeFromOffset(offset, true);
  if (!node) return '';
  const path: (string | number)[] = [];
  while (node.parent) {
    const parent: ASTNode = node.parent;
    if (parent.type === 'property') {
      path.unshift(parent.keyNode.value);
      node = parent;
      continue;
    }
    if (parent.type === 'array') {
      const index = parent.items.indexOf(node);
      if (index >= 0) path.unshift(index);
    }
    node = parent;
  }
  return redactCredentialMarkers(toJsonPointer(path));
}

function diagnosticSeverity(severity: Diagnostic['severity']): SourceLanguageDiagnosticSeverity {
  switch (severity) {
    case DiagnosticSeverity.Error:
      return 'error';
    case DiagnosticSeverity.Information:
      return 'information';
    case DiagnosticSeverity.Hint:
      return 'hint';
    case DiagnosticSeverity.Warning:
    default:
      return 'warning';
  }
}

function normalizeDiagnostics(
  diagnostics: readonly Diagnostic[],
  document: TextDocumentModel,
  jsonDocument: JSONDocument,
): readonly SourceLanguageDiagnostic[] {
  return diagnostics.map<SourceLanguageDiagnostic>((diagnostic) => {
    const offset = document.offsetAt(diagnostic.range.start);
    const endOffset = document.offsetAt(diagnostic.range.end);
    const rawCode = diagnostic.code ?? 'json-language-service';
    return {
      source: 'json-language-service',
      severity: diagnosticSeverity(diagnostic.severity),
      code: typeof rawCode === 'string' ? redactCredentialMarkers(rawCode) : (rawCode as number),
      message: redactCredentialMarkers(
        typeof diagnostic.message === 'string' ? diagnostic.message : diagnostic.message.value,
      ),
      path: pathForDiagnostic(jsonDocument, offset),
      offset,
      length: Math.max(0, endOffset - offset),
      range: diagnostic.range,
    };
  });
}

function normalizedFormattingOptions(value: unknown): FormattingOptions {
  if (
    !isRecord(value) ||
    typeof value.tabSize !== 'number' ||
    !Number.isSafeInteger(value.tabSize) ||
    value.tabSize < 1 ||
    value.tabSize > 16 ||
    typeof value.insertSpaces !== 'boolean'
  ) {
    throw new WorkerRequestError('invalid_request');
  }
  if (
    (value.insertFinalNewline !== undefined && typeof value.insertFinalNewline !== 'boolean') ||
    (value.keepLines !== undefined && typeof value.keepLines !== 'boolean')
  ) {
    throw new WorkerRequestError('invalid_request');
  }
  return {
    tabSize: value.tabSize,
    insertSpaces: value.insertSpaces,
    ...(value.insertFinalNewline === undefined
      ? {}
      : { insertFinalNewline: value.insertFinalNewline as boolean }),
    ...(value.keepLines === undefined ? {} : { keepLines: value.keepLines as boolean }),
  };
}

function optionalResultLimit(value: unknown): number | undefined {
  if (value === undefined) return undefined;
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    throw new WorkerRequestError('invalid_request');
  }
  return Math.min(value, authoringLimits.max_nodes);
}

async function executeRequest(
  service: LanguageService,
  request: SourceLanguageRequest,
  model: CachedModel,
): Promise<SourceLanguageResultMap[SourceLanguageMethod]> {
  let result: SourceLanguageResultMap[SourceLanguageMethod];

  switch (request.method) {
    case 'validation': {
      const diagnostics = await Promise.resolve(
        service.doValidation(
          model.document,
          model.jsonDocument,
          {
            comments: 'error',
            trailingCommas: 'error',
            schemaValidation: 'error',
            schemaRequest: 'ignore',
            schemaDraft: SchemaDraft.v2020_12,
          },
          LANGUAGE_SCHEMA,
        ),
      );
      result = normalizeDiagnostics(diagnostics, model.document, model.jsonDocument);
      break;
    }
    case 'completion': {
      if (!isPosition(request.payload.position)) {
        throw new WorkerRequestError('invalid_request');
      }
      const completion = await Promise.resolve(
        service.doComplete(model.document, request.payload.position, model.jsonDocument),
      );
      result = completion
        ? {
            ...completion,
            items: completion.items.filter((item) => !containsCredentialMarker(item)),
          }
        : null;
      break;
    }
    case 'completionResolve': {
      if (!isRecord(request.payload.item) || typeof request.payload.item.label !== 'string') {
        throw new WorkerRequestError('invalid_request');
      }
      if (containsCredentialMarker(request.payload.item)) {
        throw new WorkerRequestError('unsafe_credential_marker');
      }
      const resolved = await Promise.resolve(service.doResolve(request.payload.item));
      if (containsCredentialMarker(resolved)) {
        throw new WorkerRequestError('unsafe_credential_marker');
      }
      result = resolved;
      break;
    }
    case 'hover': {
      if (!isPosition(request.payload.position)) {
        throw new WorkerRequestError('invalid_request');
      }
      const hover = await Promise.resolve(
        service.doHover(model.document, request.payload.position, model.jsonDocument),
      );
      result = hover && !containsCredentialMarker(hover) ? hover : null;
      break;
    }
    case 'symbols': {
      const resultLimit = optionalResultLimit(request.payload.resultLimit);
      const symbols = service.findDocumentSymbols2(
        model.document,
        model.jsonDocument,
        resultLimit === undefined ? undefined : { resultLimit },
      );
      result = symbols.filter((symbol) => !containsCredentialMarker(symbol));
      break;
    }
    case 'format': {
      if (request.payload.range !== undefined && !isRange(request.payload.range)) {
        throw new WorkerRequestError('invalid_request');
      }
      const edits = service.format(
        model.document,
        request.payload.range,
        normalizedFormattingOptions(request.payload.options),
      );
      if (containsCredentialMarker(edits)) {
        throw new WorkerRequestError('unsafe_credential_marker');
      }
      result = edits;
      break;
    }
    case 'folding': {
      const rangeLimit = optionalResultLimit(request.payload.rangeLimit);
      const ranges = service.getFoldingRanges(
        model.document,
        rangeLimit === undefined ? undefined : { rangeLimit },
      );
      if (containsCredentialMarker(ranges)) {
        throw new WorkerRequestError('unsafe_credential_marker');
      }
      result = ranges;
      break;
    }
    case 'selectionRanges': {
      if (
        !Array.isArray(request.payload.positions) ||
        request.payload.positions.length > authoringLimits.max_nodes ||
        !request.payload.positions.every(isPosition)
      ) {
        throw new WorkerRequestError('invalid_request');
      }
      const ranges = service.getSelectionRanges(
        model.document,
        [...request.payload.positions],
        model.jsonDocument,
      );
      if (containsCredentialMarker(ranges)) {
        throw new WorkerRequestError('unsafe_credential_marker');
      }
      result = ranges;
      break;
    }
    default:
      throw new WorkerRequestError('invalid_request');
  }

  return result;
}

function sameVersion(left: SourceLanguageVersion, right: SourceLanguageVersion): boolean {
  return (
    left.documentId === right.documentId &&
    left.documentEpoch === right.documentEpoch &&
    left.modelVersion === right.modelVersion
  );
}

export function createSourceLanguageWorkerRuntime(
  postMessage: (message: SourceLanguageWorkerOutput) => void,
): SourceLanguageWorkerRuntime {
  const service = createLanguageService();
  const active = new Map<string, RequestEnvelope>();
  let model: CachedModel | null = null;
  let disposed = false;

  const safePost = (message: SourceLanguageWorkerOutput): void => {
    if (disposed) return;
    try {
      postMessage(message);
    } catch {
      // owner 已失效时无法消费结果，runtime 仍会释放 request 状态。
    }
  };

  const postFailure = (request: RequestEnvelope, code: SourceLanguageErrorCode): void => {
    const result: SourceLanguageFailureResult = {
      type: 'result',
      protocolVersion: SOURCE_LANGUAGE_PROTOCOL_VERSION,
      requestId: request.requestId,
      documentId: request.documentId,
      documentEpoch: request.documentEpoch,
      modelVersion: request.modelVersion,
      method: request.method,
      ok: false,
      error: { code, message: code },
    } as SourceLanguageFailureResult;
    safePost(result);
  };

  const replaceModel = (
    request: RequestEnvelope,
    text: string,
  ): CachedModel | SourceLanguageErrorCode => {
    const limit = inspectSourceLanguageText(text);
    if (limit) return limit;

    if (model?.documentId === request.documentId) {
      if (
        request.documentEpoch < model.documentEpoch ||
        (request.documentEpoch === model.documentEpoch && request.modelVersion < model.modelVersion)
      ) {
        return 'stale_document';
      }
      if (
        request.documentEpoch === model.documentEpoch &&
        request.modelVersion === model.modelVersion
      ) {
        return model.text === text ? model : 'document_version_conflict';
      }
    }

    for (const [requestId, pending] of active) {
      if (requestId === request.requestId || sameVersion(pending, request)) continue;
      active.delete(requestId);
      postFailure(pending, 'stale_document');
    }

    const document = TextDocument.create(
      `lanjing-source://document/${encodeURIComponent(request.documentId)}.json`,
      'json',
      request.modelVersion,
      text,
    );
    model = {
      documentId: request.documentId,
      documentEpoch: request.documentEpoch,
      modelVersion: request.modelVersion,
      text,
      document,
      jsonDocument: service.parseJSONDocument(document),
    };
    return model;
  };

  const runtime: SourceLanguageWorkerRuntime = {
    get activeRequestCount() {
      return active.size;
    },

    async handleMessage(message: unknown): Promise<void> {
      if (disposed || !isRecord(message) || typeof message.type !== 'string') return;

      if (message.type === 'dispose') {
        if (message.protocolVersion === SOURCE_LANGUAGE_PROTOCOL_VERSION) runtime.dispose();
        return;
      }

      if (message.type === 'cancel') {
        if (message.protocolVersion !== SOURCE_LANGUAGE_PROTOCOL_VERSION) return;
        const envelope = readRequestEnvelope({ ...message, method: 'validation' });
        if (!envelope) return;
        const pending = active.get(envelope.requestId);
        if (pending && sameVersion(pending, envelope)) active.delete(envelope.requestId);
        return;
      }

      if (message.type !== 'request') return;
      const envelope = readRequestEnvelope(message);
      if (!envelope) return;
      if (message.protocolVersion !== SOURCE_LANGUAGE_PROTOCOL_VERSION) {
        postFailure(envelope, 'protocol_mismatch');
        return;
      }
      if (active.has(envelope.requestId)) {
        postFailure(envelope, 'invalid_request');
        return;
      }
      if (!isRecord(message.payload) || typeof message.payload.text !== 'string') {
        postFailure(envelope, 'invalid_request');
        return;
      }

      const request = message as unknown as SourceLanguageRequest;
      active.set(envelope.requestId, envelope);
      try {
        const nextModel = replaceModel(envelope, message.payload.text);
        if (typeof nextModel === 'string') {
          postFailure(envelope, nextModel);
          return;
        }
        const result = await executeRequest(service, request, nextModel);
        const pending = active.get(envelope.requestId);
        if (!pending || !model || !sameVersion(pending, model)) return;
        const response: SourceLanguageSuccessResult = {
          type: 'result',
          protocolVersion: SOURCE_LANGUAGE_PROTOCOL_VERSION,
          requestId: envelope.requestId,
          documentId: envelope.documentId,
          documentEpoch: envelope.documentEpoch,
          modelVersion: envelope.modelVersion,
          method: envelope.method,
          ok: true,
          result,
        } as SourceLanguageSuccessResult;
        safePost(response);
      } catch (error) {
        if (active.has(envelope.requestId)) {
          postFailure(envelope, error instanceof WorkerRequestError ? error.code : 'worker_failed');
        }
      } finally {
        active.delete(envelope.requestId);
      }
    },

    dispose(): void {
      if (disposed) return;
      disposed = true;
      active.clear();
      model = null;
    },
  };

  return runtime;
}

interface WorkerModuleScope {
  readonly document?: unknown;
  postMessage(message: SourceLanguageWorkerOutput): void;
  addEventListener(type: 'message', listener: (event: MessageEvent<unknown>) => void): void;
}

const workerScope = globalThis as unknown as Partial<WorkerModuleScope>;
if (
  workerScope.document === undefined &&
  typeof workerScope.postMessage === 'function' &&
  typeof workerScope.addEventListener === 'function'
) {
  const runtime = createSourceLanguageWorkerRuntime((message) =>
    workerScope.postMessage?.(message),
  );
  workerScope.addEventListener('message', (event) => {
    void runtime.handleMessage(event.data);
  });
}
