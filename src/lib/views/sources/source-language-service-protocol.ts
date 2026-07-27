import { SourceAuthoringDocument, type AuthoringDiagnosticCode } from '$lib/rules/authoring';
import type {
  CompletionItem,
  CompletionList,
  DocumentSymbol,
  FoldingRange,
  FormattingOptions,
  Hover,
  Position,
  Range,
  SelectionRange,
  TextEdit,
} from 'vscode-json-languageservice';

export const SOURCE_LANGUAGE_PROTOCOL_VERSION = 1 as const;

export type SourceLanguageProtocolVersion = typeof SOURCE_LANGUAGE_PROTOCOL_VERSION;
export const SOURCE_LANGUAGE_METHODS = [
  'validation',
  'completion',
  'completionResolve',
  'hover',
  'symbols',
  'format',
  'folding',
  'selectionRanges',
] as const;

export type SourceLanguageMethod = (typeof SOURCE_LANGUAGE_METHODS)[number];

export function isSourceLanguageMethod(value: unknown): value is SourceLanguageMethod {
  return (
    typeof value === 'string' && (SOURCE_LANGUAGE_METHODS as readonly string[]).includes(value)
  );
}

const SOURCE_CREDENTIAL_MARKER_PATTERN = /__LANJING_[A-Z0-9_]*CREDENTIAL[A-Z0-9_]*__:[^\s"'\\]*/i;

export function containsSourceCredentialMarker(
  value: unknown,
  seen = new WeakSet<object>(),
): boolean {
  if (typeof value === 'string') return SOURCE_CREDENTIAL_MARKER_PATTERN.test(value);
  if (!value || typeof value !== 'object') return false;
  if (seen.has(value)) return false;
  seen.add(value);
  if (Array.isArray(value)) {
    return value.some((entry) => containsSourceCredentialMarker(entry, seen));
  }
  return Object.values(value).some((entry) => containsSourceCredentialMarker(entry, seen));
}

export type SourceLanguageDiagnosticSeverity = 'error' | 'warning' | 'information' | 'hint';

/** offset 与 length 使用 JavaScript UTF-16 code unit，与 Monaco、CodeMirror 和 LSP 对齐。 */
export interface SourceLanguageDiagnostic {
  readonly source: 'json-language-service';
  readonly severity: SourceLanguageDiagnosticSeverity;
  readonly code: string | number;
  readonly message: string;
  readonly path: string;
  readonly offset: number;
  readonly length: number;
  readonly range: Range;
}

export interface SourceLanguageRequestPayloadMap {
  readonly validation: {
    readonly text: string;
  };
  readonly completion: {
    readonly text: string;
    readonly position: Position;
  };
  readonly completionResolve: {
    readonly text: string;
    readonly item: CompletionItem;
  };
  readonly hover: {
    readonly text: string;
    readonly position: Position;
  };
  readonly symbols: {
    readonly text: string;
    readonly resultLimit?: number;
  };
  readonly format: {
    readonly text: string;
    readonly range?: Range;
    readonly options: FormattingOptions;
  };
  readonly folding: {
    readonly text: string;
    readonly rangeLimit?: number;
  };
  readonly selectionRanges: {
    readonly text: string;
    readonly positions: readonly Position[];
  };
}

export interface SourceLanguageRequestWithoutText<M extends SourceLanguageMethod> {
  readonly method: M;
  readonly payload: Omit<SourceLanguageRequestPayloadMap[M], 'text'>;
}

export interface SourceLanguageResultMap {
  readonly validation: readonly SourceLanguageDiagnostic[];
  readonly completion: CompletionList | null;
  readonly completionResolve: CompletionItem;
  readonly hover: Hover | null;
  readonly symbols: readonly DocumentSymbol[];
  readonly format: readonly TextEdit[];
  readonly folding: readonly FoldingRange[];
  readonly selectionRanges: readonly SelectionRange[];
}

export interface SourceLanguageVersion {
  readonly documentId: string;
  readonly documentEpoch: number;
  readonly modelVersion: number;
}

interface SourceLanguageRequestBase<M extends SourceLanguageMethod> extends SourceLanguageVersion {
  readonly type: 'request';
  readonly protocolVersion: SourceLanguageProtocolVersion;
  readonly requestId: string;
  readonly method: M;
}

export type SourceLanguageRequest<M extends SourceLanguageMethod = SourceLanguageMethod> = {
  readonly [K in M]: SourceLanguageRequestBase<K> & {
    readonly payload: SourceLanguageRequestPayloadMap[K];
  };
}[M];

interface SourceLanguageResultBase<M extends SourceLanguageMethod> extends SourceLanguageVersion {
  readonly type: 'result';
  readonly protocolVersion: SourceLanguageProtocolVersion;
  readonly requestId: string;
  readonly method: M;
}

export type SourceLanguageErrorCode =
  | 'cancelled'
  | 'disposed'
  | 'document_too_large'
  | 'document_too_complex'
  | 'document_version_conflict'
  | 'invalid_request'
  | 'protocol_mismatch'
  | 'stale_document'
  | 'unsafe_credential_marker'
  | 'worker_failed';

export interface SourceLanguageError {
  readonly code: SourceLanguageErrorCode;
  readonly message: string;
}

export type SourceLanguageSuccessResult<M extends SourceLanguageMethod = SourceLanguageMethod> = {
  readonly [K in M]: SourceLanguageResultBase<K> & {
    readonly ok: true;
    readonly result: SourceLanguageResultMap[K];
  };
}[M];

export type SourceLanguageFailureResult<M extends SourceLanguageMethod = SourceLanguageMethod> = {
  readonly [K in M]: SourceLanguageResultBase<K> & {
    readonly ok: false;
    readonly error: SourceLanguageError;
  };
}[M];

export type SourceLanguageResult<M extends SourceLanguageMethod = SourceLanguageMethod> =
  SourceLanguageSuccessResult<M> | SourceLanguageFailureResult<M>;

export interface SourceLanguageCancel extends SourceLanguageVersion {
  readonly type: 'cancel';
  readonly protocolVersion: SourceLanguageProtocolVersion;
  readonly requestId: string;
}

export interface SourceLanguageDispose {
  readonly type: 'dispose';
  readonly protocolVersion: SourceLanguageProtocolVersion;
}

export type SourceLanguageWorkerInput =
  SourceLanguageRequest | SourceLanguageCancel | SourceLanguageDispose;
export type SourceLanguageWorkerOutput = SourceLanguageResult;

export interface SourceLanguageRequestInput<
  M extends SourceLanguageMethod,
> extends SourceLanguageVersion {
  readonly method: M;
  readonly payload: SourceLanguageRequestPayloadMap[M];
}

export type SourceLanguageInputLimitCode = 'document_too_large' | 'document_too_complex';

const AUTHORING_COMPLEXITY_DIAGNOSTICS: Partial<Record<AuthoringDiagnosticCode, true>> = {
  depth_exceeded: true,
  node_count_exceeded: true,
  property_count_exceeded: true,
  property_name_utf8_bytes_exceeded: true,
  string_utf8_bytes_exceeded: true,
};

/** 字节与结构上限只消费 authoring document owner；非法 JSON 仍允许继续编辑。 */
export function inspectSourceLanguageText(text: string): SourceLanguageInputLimitCode | null {
  const diagnostics = SourceAuthoringDocument.open(text).diagnostics;
  if (diagnostics.some((diagnostic) => diagnostic.code === 'document_bytes_exceeded')) {
    return 'document_too_large';
  }
  if (diagnostics.some((diagnostic) => AUTHORING_COMPLEXITY_DIAGNOSTICS[diagnostic.code])) {
    return 'document_too_complex';
  }
  return null;
}
