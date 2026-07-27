import {
  applyEdits,
  format as computeFormattingEdits,
  findNodeAtOffset,
  getNodePath,
  modify,
  parseTree,
  type Edit,
  type FormattingOptions,
  type JSONPath,
  type Node,
  type ParseError,
} from 'jsonc-parser';
import {
  authoringLimits,
  getCatalogField,
  legadoFieldCatalog,
  type KnownSupportClass,
  type LegadoCatalogField,
} from './catalog';
import {
  jsonPathFromPointerSegments,
  parentJsonPointer,
  parseJsonPointer,
  toJsonPointer,
} from './json-pointer';

export type SupportClass = KnownSupportClass | 'unknown';
export type AuthoringDiagnosticSeverity = 'info' | 'warning' | 'error';

export type AuthoringDiagnosticCode =
  | 'invalid_json'
  | 'invalid_root'
  | 'duplicate_key'
  | 'document_bytes_exceeded'
  | 'depth_exceeded'
  | 'node_count_exceeded'
  | 'property_count_exceeded'
  | 'property_name_utf8_bytes_exceeded'
  | 'string_utf8_bytes_exceeded'
  | 'required_field_missing'
  | 'invalid_field_type'
  | 'known_field_preserved'
  | 'known_field_blocked'
  | 'unknown_field'
  | 'pointer_missing'
  | 'pointer_ambiguous'
  | 'credential_schema_unsupported'
  | 'credential_slot_duplicate'
  | 'credential_sentinel_invalid'
  | 'credential_sentinel_missing'
  | 'credential_sentinel_duplicate'
  | 'credential_format_mismatch'
  | 'credential_owner_mismatch'
  | 'credential_revision_mismatch'
  | 'credential_path_mismatch'
  | 'credential_duplicate_sensitive_key'
  | 'credential_request_header_blocked'
  | 'credential_query_blocked'
  | 'pointer_invalid'
  | 'epoch_stale'
  | 'patch_operation_invalid';

/** Rust/TypeScript parity 只比较这六个无敏感文案字段。 */
export interface AuthoringDiagnostic {
  readonly severity: AuthoringDiagnosticSeverity;
  readonly code: AuthoringDiagnosticCode;
  readonly path: string;
  readonly byte_offset: number;
  readonly byte_length: number;
  readonly support: SupportClass;
}

export type JsonValue =
  null | boolean | number | string | readonly JsonValue[] | { readonly [key: string]: JsonValue };

export interface AuthoringPointerNode {
  readonly pointer: string;
  readonly path: readonly (string | number)[];
  readonly node: Node;
  readonly keyNode?: Node;
}

export type PointerLookupResult =
  | { readonly kind: 'found'; readonly entry: AuthoringPointerNode }
  | { readonly kind: 'missing'; readonly pointer: string }
  | {
      readonly kind: 'ambiguous';
      readonly pointer: string;
      readonly entries: readonly AuthoringPointerNode[];
    }
  | { readonly kind: 'invalid'; readonly pointer: string };

export type SourceAuthoringPatch =
  | {
      readonly operation: 'set';
      readonly pointer: string;
      readonly value: JsonValue;
      readonly expectedEpoch: number;
    }
  | {
      readonly operation: 'remove';
      readonly pointer: string;
      readonly expectedEpoch: number;
    }
  | {
      readonly operation: 'insert';
      readonly pointer: string;
      readonly value: JsonValue;
      readonly expectedEpoch: number;
    };

export interface SourceAuthoringFormatRequest {
  readonly expectedEpoch: number;
  readonly options?: Readonly<FormattingOptions>;
}

export interface SourceAuthoringTextReplacement {
  readonly expectedEpoch: number;
  readonly text: string;
}

/** Edit 同时暴露编辑器原生 UTF-16 位置与诊断对齐所需的 UTF-8 span。 */
export interface AuthoringTextEdit {
  readonly utf16_offset: number;
  readonly utf16_length: number;
  readonly byte_offset: number;
  readonly byte_length: number;
  readonly text: string;
}

export type SourceAuthoringEditResult =
  | {
      readonly kind: 'applied';
      readonly text: string;
      readonly epoch: number;
      readonly edits: readonly AuthoringTextEdit[];
      readonly diagnostics: readonly AuthoringDiagnostic[];
    }
  | {
      readonly kind: 'unchanged';
      readonly text: string;
      readonly epoch: number;
      readonly edits: readonly [];
      readonly diagnostics: readonly AuthoringDiagnostic[];
    }
  | {
      readonly kind: 'rejected';
      readonly text: string;
      readonly epoch: number;
      readonly diagnostic: AuthoringDiagnostic;
      readonly diagnostics: readonly AuthoringDiagnostic[];
    };

interface Utf8Span {
  readonly byte_offset: number;
  readonly byte_length: number;
}

interface FieldOccurrence {
  readonly pointer: string;
  readonly keyNode: Node;
  readonly valueNode: Node;
  readonly catalogField: LegadoCatalogField | undefined;
  readonly support: SupportClass;
}

interface DocumentSnapshot {
  readonly text: string;
  readonly syntaxTree: Node | undefined;
  readonly pointerIndex: ReadonlyMap<string, readonly AuthoringPointerNode[]>;
  readonly diagnostics: readonly AuthoringDiagnostic[];
  readonly patchBlockers: readonly AuthoringDiagnostic[];
  readonly utf8Offsets: Utf8OffsetTable | undefined;
}

const STRICT_PARSE_OPTIONS = {
  allowEmptyContent: false,
  allowTrailingComma: false,
  disallowComments: true,
} as const;
const CREDENTIAL_SENTINEL_PREFIX = '__LANJING_CREDENTIAL_SLOT_V1__:';
const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

class ReadonlyMapView<K, V> implements ReadonlyMap<K, V> {
  readonly #source: ReadonlyMap<K, V>;

  constructor(source: ReadonlyMap<K, V>) {
    this.#source = source;
    Object.freeze(this);
  }

  get size(): number {
    return this.#source.size;
  }

  get(key: K): V | undefined {
    return this.#source.get(key);
  }

  has(key: K): boolean {
    return this.#source.has(key);
  }

  entries(): MapIterator<[K, V]> {
    return this.#source.entries();
  }

  keys(): MapIterator<K> {
    return this.#source.keys();
  }

  values(): MapIterator<V> {
    return this.#source.values();
  }

  forEach(callbackfn: (value: V, key: K, map: ReadonlyMap<K, V>) => void, thisArg?: unknown): void {
    this.#source.forEach((value, key) => callbackfn.call(thisArg, value, key, this));
  }

  [Symbol.iterator](): MapIterator<[K, V]> {
    return this.entries();
  }
}

function freezeSyntaxTree(root: Node | undefined): void {
  if (!root) return;
  const visited = new WeakSet<object>();
  const visit = (node: Node): void => {
    if (visited.has(node)) return;
    visited.add(node);
    if (node.children) {
      for (const child of node.children) visit(child);
      Object.freeze(node.children);
    }
    Object.freeze(node);
  };
  visit(root);
}

const EMPTY_POINTER_INDEX: ReadonlyMap<string, readonly AuthoringPointerNode[]> =
  new ReadonlyMapView(new Map());

class Utf8OffsetTable {
  private readonly offsets: Uint32Array;

  constructor(text: string) {
    this.offsets = new Uint32Array(text.length + 1);

    let utf16Offset = 0;
    let byteOffset = 0;
    while (utf16Offset < text.length) {
      const first = text.charCodeAt(utf16Offset);
      this.offsets[utf16Offset] = byteOffset;

      if (
        first >= 0xd800 &&
        first <= 0xdbff &&
        utf16Offset + 1 < text.length &&
        text.charCodeAt(utf16Offset + 1) >= 0xdc00 &&
        text.charCodeAt(utf16Offset + 1) <= 0xdfff
      ) {
        this.offsets[utf16Offset + 1] = byteOffset + 3;
        byteOffset += 4;
        utf16Offset += 2;
        this.offsets[utf16Offset] = byteOffset;
        continue;
      }

      byteOffset += first <= 0x7f ? 1 : first <= 0x7ff ? 2 : 3;
      utf16Offset += 1;
      this.offsets[utf16Offset] = byteOffset;
    }
  }

  span(utf16Offset: number, utf16Length: number): Utf8Span {
    const start = Math.max(0, Math.min(utf16Offset, this.offsets.length - 1));
    const end = Math.max(start, Math.min(utf16Offset + utf16Length, this.offsets.length - 1));
    return {
      byte_offset: this.offsets[start],
      byte_length: this.offsets[end] - this.offsets[start],
    };
  }
}

function utf8ByteLength(value: string): number {
  let bytes = 0;

  for (let index = 0; index < value.length; index += 1) {
    const first = value.charCodeAt(index);
    if (first <= 0x7f) {
      bytes += 1;
    } else if (first <= 0x7ff) {
      bytes += 2;
    } else if (
      first >= 0xd800 &&
      first <= 0xdbff &&
      index + 1 < value.length &&
      value.charCodeAt(index + 1) >= 0xdc00 &&
      value.charCodeAt(index + 1) <= 0xdfff
    ) {
      bytes += 4;
      index += 1;
    } else {
      bytes += 3;
    }
  }

  return bytes;
}

function diagnostic(
  severity: AuthoringDiagnosticSeverity,
  code: AuthoringDiagnosticCode,
  path: string,
  span: Utf8Span,
  support: SupportClass,
): AuthoringDiagnostic {
  return { severity, code, path, ...span, support };
}

const DIAGNOSTIC_SEVERITY_RANK: Readonly<Record<AuthoringDiagnosticSeverity, number>> = {
  error: 0,
  warning: 1,
  info: 2,
};

function sortDiagnostics(
  diagnostics: readonly AuthoringDiagnostic[],
): readonly AuthoringDiagnostic[] {
  return Object.freeze(
    [...diagnostics].sort(
      (left, right) =>
        left.byte_offset - right.byte_offset ||
        DIAGNOSTIC_SEVERITY_RANK[left.severity] - DIAGNOSTIC_SEVERITY_RANK[right.severity] ||
        (left.code < right.code ? -1 : left.code > right.code ? 1 : 0) ||
        (left.path < right.path ? -1 : left.path > right.path ? 1 : 0),
    ),
  );
}

function nodeSpan(node: Node, utf8Offsets: Utf8OffsetTable): Utf8Span {
  return utf8Offsets.span(node.offset, node.length);
}

function matchesCatalogType(field: LegadoCatalogField, node: Node): boolean {
  switch (field.type) {
    case 'string':
      return node.type === 'string';
    case 'integer':
      return node.type === 'number' && Number.isInteger(node.value);
    case 'integer_or_string':
      return (
        (node.type === 'number' && Number.isInteger(node.value)) ||
        (node.type === 'string' && /^-?\d+$/.test(String(node.value)))
      );
    case 'boolean':
      return node.type === 'boolean';
    case 'object_or_string':
      return node.type === 'object' || node.type === 'string';
  }
}

function nearestSyntaxParentPointer(root: Node | undefined, offset: number): string {
  let node = root ? findNodeAtOffset(root, offset, true) : undefined;
  while (node && node.type !== 'object' && node.type !== 'array') node = node.parent;
  return node ? toJsonPointer(getNodePath(node)) : '';
}

function isSupportedRuleExpression(value: string): boolean {
  return value.split('||').every((candidate) => {
    const selector = candidate.split('##', 1)[0]?.trim() ?? '';
    if (!selector) return false;
    const normalized = selector.toLowerCase();
    return !(
      normalized.startsWith('@js:') ||
      normalized.startsWith('@json:') ||
      normalized.startsWith('@regex:') ||
      normalized.startsWith('@xpath:') ||
      normalized.startsWith('/') ||
      normalized.startsWith('$') ||
      normalized.includes('<js>') ||
      normalized.includes('{{')
    );
  });
}

function freezePointerIndex(
  entries: Map<string, AuthoringPointerNode[]>,
): ReadonlyMap<string, readonly AuthoringPointerNode[]> {
  const result = new Map<string, readonly AuthoringPointerNode[]>();
  for (const [pointer, nodes] of entries) result.set(pointer, Object.freeze(nodes));
  return new ReadonlyMapView(result);
}

function addPointerEntry(
  index: Map<string, AuthoringPointerNode[]>,
  pointer: string,
  path: readonly (string | number)[],
  node: Node,
  keyNode?: Node,
): void {
  const entry: AuthoringPointerNode = Object.freeze({
    pointer,
    path: Object.freeze([...path]),
    node,
    ...(keyNode ? { keyNode } : {}),
  });
  const existing = index.get(pointer);
  if (existing) existing.push(entry);
  else index.set(pointer, [entry]);
}

function inspectValidTree(
  root: Node,
  utf8Offsets: Utf8OffsetTable,
): {
  readonly pointerIndex: ReadonlyMap<string, readonly AuthoringPointerNode[]>;
  readonly structuralDiagnostics: readonly AuthoringDiagnostic[];
  readonly supportDiagnostics: readonly AuthoringDiagnostic[];
} {
  const pointerEntries = new Map<string, AuthoringPointerNode[]>();
  const duplicateDiagnostics: AuthoringDiagnostic[] = [];
  const limitDiagnostics: AuthoringDiagnostic[] = [];
  const fieldOccurrences: FieldOccurrence[] = [];
  const credentialDiagnostics: AuthoringDiagnostic[] = [];
  const invalidCredentialPointers = new Set<string>();
  let nodeCount = 0;
  let propertyCount = 0;
  let stopped = false;
  let reportedDepth = false;
  let reportedNodeCount = false;
  let reportedPropertyCount = false;
  let reportedPropertyName = false;
  let reportedString = false;

  const visitValue = (
    node: Node,
    pointer: string,
    path: readonly (string | number)[],
    depth: number,
    keyNode?: Node,
  ): void => {
    if (stopped) return;

    nodeCount += 1;
    if (nodeCount > authoringLimits.max_nodes) {
      if (!reportedNodeCount) {
        limitDiagnostics.push(
          diagnostic(
            'error',
            'node_count_exceeded',
            pointer,
            utf8Offsets.span(node.offset, Math.min(1, node.length)),
            'blocked',
          ),
        );
        reportedNodeCount = true;
      }
      stopped = true;
      return;
    }

    addPointerEntry(pointerEntries, pointer, path, node, keyNode);

    if (depth > authoringLimits.max_depth) {
      if (!reportedDepth) {
        limitDiagnostics.push(
          diagnostic(
            'error',
            'depth_exceeded',
            pointer,
            utf8Offsets.span(node.offset, Math.min(1, node.length)),
            'blocked',
          ),
        );
        reportedDepth = true;
      }
      return;
    }

    if (node.type === 'string' && typeof node.value === 'string') {
      if (!reportedString && utf8ByteLength(node.value) > authoringLimits.max_string_utf8_bytes) {
        limitDiagnostics.push(
          diagnostic(
            'error',
            'string_utf8_bytes_exceeded',
            pointer,
            nodeSpan(node, utf8Offsets),
            'blocked',
          ),
        );
        reportedString = true;
      }
      if (
        node.value.startsWith(CREDENTIAL_SENTINEL_PREFIX) &&
        !UUID_PATTERN.test(node.value.slice(CREDENTIAL_SENTINEL_PREFIX.length))
      ) {
        invalidCredentialPointers.add(pointer);
        credentialDiagnostics.push(
          diagnostic(
            'error',
            'credential_sentinel_invalid',
            pointer,
            nodeSpan(node, utf8Offsets),
            'blocked',
          ),
        );
      }
      return;
    }

    if (node.type === 'array') {
      for (let index = 0; index < (node.children?.length ?? 0); index += 1) {
        const child = node.children?.[index];
        if (!child) continue;
        const childPath = [...path, index];
        visitValue(child, toJsonPointer(childPath), childPath, depth + 1);
        if (stopped) return;
      }
      return;
    }

    if (node.type !== 'object') return;

    const seenProperties = new Set<string>();
    for (const propertyNode of node.children ?? []) {
      const propertyChildren = propertyNode.children;
      const propertyKeyNode = propertyChildren?.[0];
      const propertyValueNode = propertyChildren?.[1];
      if (
        !propertyKeyNode ||
        propertyKeyNode.type !== 'string' ||
        typeof propertyKeyNode.value !== 'string' ||
        !propertyValueNode
      ) {
        continue;
      }

      propertyCount += 1;
      const propertyName = propertyKeyNode.value;
      const childPath = [...path, propertyName];
      const childPointer = toJsonPointer(childPath);
      const catalogField = getCatalogField(childPointer);
      const support = catalogField?.support ?? 'unknown';

      if (propertyCount > authoringLimits.max_properties) {
        if (!reportedPropertyCount) {
          limitDiagnostics.push(
            diagnostic(
              'error',
              'property_count_exceeded',
              childPointer,
              nodeSpan(propertyKeyNode, utf8Offsets),
              'blocked',
            ),
          );
          reportedPropertyCount = true;
        }
        stopped = true;
        return;
      }

      if (
        !reportedPropertyName &&
        utf8ByteLength(propertyName) > authoringLimits.max_property_name_utf8_bytes
      ) {
        limitDiagnostics.push(
          diagnostic(
            'error',
            'property_name_utf8_bytes_exceeded',
            childPointer,
            nodeSpan(propertyKeyNode, utf8Offsets),
            'blocked',
          ),
        );
        reportedPropertyName = true;
      }

      if (seenProperties.has(propertyName)) {
        duplicateDiagnostics.push(
          diagnostic(
            'error',
            'duplicate_key',
            childPointer,
            nodeSpan(propertyKeyNode, utf8Offsets),
            'blocked',
          ),
        );
      } else {
        seenProperties.add(propertyName);
      }

      fieldOccurrences.push({
        pointer: childPointer,
        keyNode: propertyKeyNode,
        valueNode: propertyValueNode,
        catalogField,
        support,
      });
      visitValue(propertyValueNode, childPointer, childPath, depth + 1, propertyKeyNode);
      if (stopped) return;
    }
  };

  visitValue(root, '', [], 1);

  const structuralDiagnostics = Object.freeze([...duplicateDiagnostics, ...limitDiagnostics]);
  const supportDiagnostics: AuthoringDiagnostic[] = [];
  if (structuralDiagnostics.length === 0) {
    for (const field of legadoFieldCatalog.fields) {
      if (!field.required || pointerEntries.has(field.pointer)) continue;
      const parentPointer = parentJsonPointer(field.pointer);
      if (parentPointer === null) continue;
      const parentEntries = pointerEntries.get(parentPointer);
      if (!parentEntries || parentEntries.length !== 1 || parentEntries[0].node.type !== 'object') {
        continue;
      }
      const parentNode = parentEntries[0].node;
      supportDiagnostics.push(
        diagnostic(
          'error',
          'required_field_missing',
          field.pointer,
          utf8Offsets.span(parentNode.offset, Math.min(1, parentNode.length)),
          'blocked',
        ),
      );
    }

    for (const occurrence of fieldOccurrences) {
      if (invalidCredentialPointers.has(occurrence.pointer)) continue;

      if (
        occurrence.catalogField &&
        !occurrence.catalogField.required &&
        occurrence.valueNode.type === 'null'
      ) {
        continue;
      }

      if (
        occurrence.catalogField &&
        !matchesCatalogType(occurrence.catalogField, occurrence.valueNode)
      ) {
        supportDiagnostics.push(
          diagnostic(
            'error',
            'invalid_field_type',
            occurrence.pointer,
            nodeSpan(occurrence.valueNode, utf8Offsets),
            'blocked',
          ),
        );
        continue;
      }

      if (
        occurrence.catalogField?.type === 'object_or_string' &&
        occurrence.valueNode.type === 'string'
      ) {
        supportDiagnostics.push(
          diagnostic(
            'error',
            'known_field_blocked',
            occurrence.pointer,
            nodeSpan(occurrence.valueNode, utf8Offsets),
            'blocked',
          ),
        );
        continue;
      }

      if (
        occurrence.support === 'blocked' &&
        occurrence.pointer === '/enabledCookieJar' &&
        occurrence.valueNode.type === 'boolean' &&
        occurrence.valueNode.value === false
      ) {
        continue;
      }

      if (
        occurrence.catalogField?.support === 'executable' &&
        occurrence.valueNode.type === 'string' &&
        typeof occurrence.valueNode.value === 'string' &&
        ((occurrence.pointer === '/exploreUrl' &&
          !occurrence.valueNode.value.trimStart().startsWith('@js:')) ||
          (occurrence.catalogField.rule_result_type !== null &&
            !isSupportedRuleExpression(occurrence.valueNode.value)))
      ) {
        supportDiagnostics.push(
          diagnostic(
            'error',
            'known_field_blocked',
            occurrence.pointer,
            nodeSpan(occurrence.valueNode, utf8Offsets),
            'blocked',
          ),
        );
        continue;
      }

      if (
        occurrence.pointer === '/bookSourceType' &&
        occurrence.valueNode.type === 'number' &&
        occurrence.valueNode.value !== 0
      ) {
        supportDiagnostics.push(
          diagnostic(
            'error',
            'known_field_blocked',
            occurrence.pointer,
            nodeSpan(occurrence.valueNode, utf8Offsets),
            'blocked',
          ),
        );
        continue;
      }

      if (occurrence.support === 'executable') continue;
      const severity: AuthoringDiagnosticSeverity =
        occurrence.support === 'preserved'
          ? 'info'
          : occurrence.support === 'blocked'
            ? 'error'
            : 'warning';
      const code: AuthoringDiagnosticCode =
        occurrence.support === 'preserved'
          ? 'known_field_preserved'
          : occurrence.support === 'blocked'
            ? 'known_field_blocked'
            : 'unknown_field';
      supportDiagnostics.push(
        diagnostic(
          severity,
          code,
          occurrence.pointer,
          nodeSpan(occurrence.keyNode, utf8Offsets),
          occurrence.support,
        ),
      );
    }
    for (const credentialDiagnostic of credentialDiagnostics) {
      supportDiagnostics.push(credentialDiagnostic);
    }
  }

  return {
    pointerIndex: freezePointerIndex(pointerEntries),
    structuralDiagnostics,
    supportDiagnostics: Object.freeze(supportDiagnostics),
  };
}

function analyzeDocument(text: string): DocumentSnapshot {
  const documentByteLength = utf8ByteLength(text);
  if (documentByteLength > authoringLimits.max_utf8_bytes) {
    const bytesDiagnostic = diagnostic(
      'error',
      'document_bytes_exceeded',
      '',
      {
        byte_offset: authoringLimits.max_utf8_bytes,
        byte_length: documentByteLength - authoringLimits.max_utf8_bytes,
      },
      'blocked',
    );
    return {
      text,
      syntaxTree: undefined,
      pointerIndex: EMPTY_POINTER_INDEX,
      diagnostics: Object.freeze([bytesDiagnostic]),
      patchBlockers: Object.freeze([bytesDiagnostic]),
      utf8Offsets: undefined,
    };
  }

  const utf8Offsets = new Utf8OffsetTable(text);
  const parseErrors: ParseError[] = [];
  const syntaxTree = parseTree(text, parseErrors, STRICT_PARSE_OPTIONS);
  freezeSyntaxTree(syntaxTree);

  if (!syntaxTree || parseErrors.length > 0) {
    const firstError = parseErrors.reduce<ParseError | undefined>(
      (earliest, error) => (!earliest || error.offset < earliest.offset ? error : earliest),
      undefined,
    );
    const invalidDiagnostics = firstError
      ? [
          diagnostic(
            'error',
            'invalid_json',
            nearestSyntaxParentPointer(syntaxTree, Math.min(firstError.offset, text.length)),
            utf8Offsets.span(firstError.offset, firstError.length),
            'blocked',
          ),
        ]
      : [diagnostic('error', 'invalid_json', '', { byte_offset: 0, byte_length: 0 }, 'blocked')];
    const frozenDiagnostics = sortDiagnostics(invalidDiagnostics);
    return {
      text,
      syntaxTree,
      pointerIndex: EMPTY_POINTER_INDEX,
      diagnostics: frozenDiagnostics,
      patchBlockers: frozenDiagnostics,
      utf8Offsets,
    };
  }

  if (syntaxTree.type !== 'object') {
    const rootDiagnostic = diagnostic(
      'error',
      'invalid_root',
      '',
      nodeSpan(syntaxTree, utf8Offsets),
      'blocked',
    );
    return {
      text,
      syntaxTree,
      pointerIndex: freezePointerIndex(
        new Map([
          ['', [Object.freeze({ pointer: '', path: Object.freeze([]), node: syntaxTree })]],
        ]),
      ),
      diagnostics: Object.freeze([rootDiagnostic]),
      patchBlockers: Object.freeze([rootDiagnostic]),
      utf8Offsets,
    };
  }
  const inspection = inspectValidTree(syntaxTree, utf8Offsets);

  const diagnostics = sortDiagnostics(
    inspection.structuralDiagnostics.length > 0
      ? inspection.structuralDiagnostics
      : inspection.supportDiagnostics,
  );
  return {
    text,
    syntaxTree,
    pointerIndex: inspection.pointerIndex,
    diagnostics,
    patchBlockers: inspection.structuralDiagnostics,
    utf8Offsets,
  };
}

function isJsonValue(value: unknown, ancestors = new WeakSet<object>()): value is JsonValue {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if (typeof value !== 'object') return false;
  if (ancestors.has(value)) return false;

  ancestors.add(value);
  let valid = true;
  if (Array.isArray(value)) {
    for (let index = 0; index < value.length; index += 1) {
      const item = Object.getOwnPropertyDescriptor(value, String(index));
      if (!item?.enumerable || !('value' in item) || !isJsonValue(item.value, ancestors)) {
        valid = false;
        break;
      }
    }
  } else {
    const prototype = Object.getPrototypeOf(value);
    valid = prototype === Object.prototype || prototype === null;
    if (valid) {
      for (const key of Reflect.ownKeys(value)) {
        const property =
          typeof key === 'string' ? Object.getOwnPropertyDescriptor(value, key) : undefined;
        if (
          !property?.enumerable ||
          !('value' in property) ||
          !isJsonValue(property.value, ancestors)
        ) {
          valid = false;
          break;
        }
      }
    }
  }
  ancestors.delete(value);
  return valid;
}

function detectFormattingOptions(text: string): FormattingOptions {
  const eol = text.includes('\r\n') ? '\r\n' : '\n';
  const indentation = text.match(/(?:^|\r?\n)([ \t]+)(?=["}\]])/)?.[1];
  if (indentation?.startsWith('\t')) return { eol, insertSpaces: false, tabSize: 1 };
  return { eol, insertSpaces: true, tabSize: indentation?.length ?? 2 };
}

function describeEdits(
  text: string,
  edits: readonly Edit[],
  utf8Offsets: Utf8OffsetTable | undefined,
): readonly AuthoringTextEdit[] {
  return Object.freeze(
    edits.map((edit) => {
      const span = utf8Offsets?.span(edit.offset, edit.length) ?? {
        byte_offset: utf8ByteLength(text.slice(0, edit.offset)),
        byte_length: utf8ByteLength(text.slice(edit.offset, edit.offset + edit.length)),
      };
      return Object.freeze({
        utf16_offset: edit.offset,
        utf16_length: edit.length,
        ...span,
        text: edit.content,
      });
    }),
  );
}

/**
 * Legado 作者文档的唯一文本、syntax tree、pointer index 与 epoch owner。
 * typed patch 从不修复非法文本；原文编辑必须显式调用 replaceText。
 */
export class SourceAuthoringDocument {
  private snapshot: DocumentSnapshot;
  private documentEpoch: number;

  constructor(text: string, epoch = 0) {
    if (!Number.isSafeInteger(epoch) || epoch < 0) {
      throw new RangeError('SourceAuthoringDocument epoch must be a non-negative safe integer');
    }
    this.snapshot = analyzeDocument(text);
    this.documentEpoch = epoch;
  }

  static open(text: string, epoch = 0): SourceAuthoringDocument {
    return new SourceAuthoringDocument(text, epoch);
  }

  get text(): string {
    return this.snapshot.text;
  }

  get epoch(): number {
    return this.documentEpoch;
  }

  get syntaxTree(): Node | undefined {
    return this.snapshot.syntaxTree;
  }

  get pointerIndex(): ReadonlyMap<string, readonly AuthoringPointerNode[]> {
    return this.snapshot.pointerIndex;
  }

  get diagnostics(): readonly AuthoringDiagnostic[] {
    return this.snapshot.diagnostics;
  }

  lookupPointer(pointer: string): PointerLookupResult {
    if (!parseJsonPointer(pointer).ok) return { kind: 'invalid', pointer };
    const entries = this.snapshot.pointerIndex.get(pointer);
    if (!entries || entries.length === 0) return { kind: 'missing', pointer };
    if (entries.length > 1) return { kind: 'ambiguous', pointer, entries };
    return { kind: 'found', entry: entries[0] };
  }

  applyPatch(patch: SourceAuthoringPatch): SourceAuthoringEditResult {
    const stale = this.rejectStaleEpoch(patch.expectedEpoch);
    if (stale) return stale;

    const parsedPointer = parseJsonPointer(patch.pointer);
    if (!parsedPointer.ok) {
      return this.reject(this.pointerDiagnostic('pointer_invalid', '', undefined));
    }

    const nonDuplicateBlocker = this.snapshot.patchBlockers.find(
      (blocker) => blocker.code !== 'duplicate_key',
    );
    if (nonDuplicateBlocker) return this.reject(nonDuplicateBlocker);

    let path: JSONPath;
    let modificationOptions: { readonly isArrayInsertion?: boolean } = {};

    if (patch.operation === 'insert') {
      if (!isJsonValue(patch.value) || patch.pointer === '') {
        return this.reject(this.pointerDiagnostic('patch_operation_invalid', patch.pointer));
      }

      const parentPointer = parentJsonPointer(patch.pointer);
      const parentLookup = this.lookupPointer(parentPointer ?? '');
      if (parentLookup.kind !== 'found')
        return this.rejectLookup(parentLookup, parentPointer ?? '');
      if (parentLookup.entry.node.type !== 'array') {
        return this.reject(
          this.pointerDiagnostic('patch_operation_invalid', patch.pointer, parentLookup.entry),
        );
      }

      const segment = parsedPointer.segments.at(-1);
      const arrayPath =
        segment === undefined
          ? null
          : jsonPathFromPointerSegments(parentLookup.entry.path, 'array', segment);
      const index = arrayPath?.at(-1);
      if (
        !arrayPath ||
        typeof index !== 'number' ||
        index > (parentLookup.entry.node.children?.length ?? 0)
      ) {
        return this.reject(
          this.pointerDiagnostic('pointer_missing', patch.pointer, parentLookup.entry),
        );
      }
      path = [...arrayPath];
      modificationOptions = { isArrayInsertion: true };
    } else {
      const lookup = this.lookupPointer(patch.pointer);
      if (lookup.kind === 'ambiguous' || lookup.kind === 'invalid') {
        return this.rejectLookup(lookup, patch.pointer);
      }

      if (patch.operation === 'remove') {
        if (patch.pointer === '') {
          return this.reject(this.pointerDiagnostic('patch_operation_invalid', patch.pointer));
        }
        if (lookup.kind === 'missing') return this.rejectLookup(lookup, patch.pointer);
        path = [...lookup.entry.path];
      } else {
        if (!isJsonValue(patch.value)) {
          return this.reject(this.pointerDiagnostic('patch_operation_invalid', patch.pointer));
        }

        if (lookup.kind === 'found') {
          path = [...lookup.entry.path];
        } else {
          const parentPointer = parentJsonPointer(patch.pointer);
          const parentLookup = this.lookupPointer(parentPointer ?? '');
          if (parentLookup.kind !== 'found') {
            return this.rejectLookup(parentLookup, parentPointer ?? patch.pointer);
          }
          if (parentLookup.entry.node.type !== 'object') {
            return this.reject(
              this.pointerDiagnostic('pointer_missing', patch.pointer, parentLookup.entry),
            );
          }
          const segment = parsedPointer.segments.at(-1);
          if (segment === undefined) {
            return this.reject(this.pointerDiagnostic('patch_operation_invalid', patch.pointer));
          }
          const objectPath = jsonPathFromPointerSegments(
            parentLookup.entry.path,
            'object',
            segment,
          );
          if (!objectPath) {
            return this.reject(
              this.pointerDiagnostic('patch_operation_invalid', patch.pointer, parentLookup.entry),
            );
          }
          path = [...objectPath];
        }
      }
    }

    const duplicateBlocker = this.snapshot.patchBlockers.find(
      (blocker) => blocker.code === 'duplicate_key',
    );
    if (duplicateBlocker) return this.reject(duplicateBlocker);

    try {
      const value = patch.operation === 'remove' ? undefined : patch.value;
      const edits = modify(this.text, path, value, modificationOptions);
      return this.applyComputedEdits(edits);
    } catch {
      return this.reject(this.pointerDiagnostic('patch_operation_invalid', patch.pointer));
    }
  }

  /** 原文编辑是显式全量替换；允许暂时非法文本并立即重建 diagnostics。 */
  replaceText(replacement: SourceAuthoringTextReplacement): SourceAuthoringEditResult {
    const stale = this.rejectStaleEpoch(replacement.expectedEpoch);
    if (stale) return stale;
    if (replacement.text === this.text) return this.unchanged();

    const edit: Edit = { offset: 0, length: this.text.length, content: replacement.text };
    const describedEdits = describeEdits(this.text, [edit], this.snapshot.utf8Offsets);
    this.snapshot = analyzeDocument(replacement.text);
    this.documentEpoch += 1;
    return this.applied(describedEdits);
  }

  /** 格式化只在显式调用时产生全局 text edit，open/patch/mode 切换均不触发。 */
  format(request: SourceAuthoringFormatRequest): SourceAuthoringEditResult {
    const stale = this.rejectStaleEpoch(request.expectedEpoch);
    if (stale) return stale;
    const blocker = this.snapshot.patchBlockers[0];
    if (blocker) return this.reject(blocker);

    try {
      const edits = computeFormattingEdits(this.text, undefined, {
        ...detectFormattingOptions(this.text),
        ...request.options,
      });
      return this.applyComputedEdits(edits);
    } catch {
      return this.reject(this.pointerDiagnostic('patch_operation_invalid', ''));
    }
  }

  private applyComputedEdits(edits: Edit[]): SourceAuthoringEditResult {
    if (edits.length === 0) return this.unchanged();

    const nextText = applyEdits(this.text, edits);
    if (nextText === this.text) return this.unchanged();

    const nextSnapshot = analyzeDocument(nextText);
    const blocker = nextSnapshot.patchBlockers[0];
    if (blocker) return this.reject(blocker);

    const describedEdits = describeEdits(this.text, edits, this.snapshot.utf8Offsets);
    this.snapshot = nextSnapshot;
    this.documentEpoch += 1;
    return this.applied(describedEdits);
  }

  private rejectStaleEpoch(expectedEpoch: number): SourceAuthoringEditResult | null {
    if (expectedEpoch === this.documentEpoch) return null;
    return this.reject(
      diagnostic('error', 'epoch_stale', '', { byte_offset: 0, byte_length: 0 }, 'blocked'),
    );
  }

  private rejectLookup(lookup: PointerLookupResult, pointer: string): SourceAuthoringEditResult {
    if (lookup.kind === 'ambiguous') {
      return this.reject(this.pointerDiagnostic('pointer_ambiguous', pointer, lookup.entries[1]));
    }
    if (lookup.kind === 'invalid') {
      return this.reject(this.pointerDiagnostic('pointer_invalid', '', undefined));
    }
    return this.reject(this.pointerDiagnostic('pointer_missing', pointer));
  }

  private pointerDiagnostic(
    code: Extract<
      AuthoringDiagnosticCode,
      'pointer_missing' | 'pointer_ambiguous' | 'pointer_invalid' | 'patch_operation_invalid'
    >,
    pointer: string,
    entry?: AuthoringPointerNode,
  ): AuthoringDiagnostic {
    const span =
      entry && this.snapshot.utf8Offsets
        ? nodeSpan(entry.keyNode ?? entry.node, this.snapshot.utf8Offsets)
        : { byte_offset: 0, byte_length: 0 };
    return diagnostic('error', code, pointer, span, 'blocked');
  }

  private applied(edits: readonly AuthoringTextEdit[]): SourceAuthoringEditResult {
    return {
      kind: 'applied',
      text: this.text,
      epoch: this.documentEpoch,
      edits,
      diagnostics: this.diagnostics,
    };
  }

  private unchanged(): SourceAuthoringEditResult {
    return {
      kind: 'unchanged',
      text: this.text,
      epoch: this.documentEpoch,
      edits: [],
      diagnostics: this.diagnostics,
    };
  }

  private reject(rejection: AuthoringDiagnostic): SourceAuthoringEditResult {
    return {
      kind: 'rejected',
      text: this.text,
      epoch: this.documentEpoch,
      diagnostic: rejection,
      diagnostics: this.diagnostics,
    };
  }
}
