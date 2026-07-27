import type { JSONPath } from 'jsonc-parser';

export type JsonPointerParseResult =
  | { readonly ok: true; readonly segments: readonly string[] }
  | { readonly ok: false; readonly reason: 'must_start_with_slash' | 'invalid_escape' };

function escapePointerSegment(segment: string): string {
  return segment.replaceAll('~', '~0').replaceAll('/', '~1');
}

function unescapePointerSegment(segment: string): string | null {
  let decoded = '';

  for (let index = 0; index < segment.length; index += 1) {
    const character = segment[index];
    if (character !== '~') {
      decoded += character;
      continue;
    }

    const escape = segment[index + 1];
    if (escape === '0') {
      decoded += '~';
    } else if (escape === '1') {
      decoded += '/';
    } else {
      return null;
    }
    index += 1;
  }

  return decoded;
}

/** 把 jsonc-parser path 转为唯一的 RFC 6901 JSON Pointer。 */
export function toJsonPointer(path: readonly (string | number)[]): string {
  if (path.length === 0) return '';
  return `/${path.map((segment) => escapePointerSegment(String(segment))).join('/')}`;
}

/** 严格解析 RFC 6901 pointer；数组索引的语义由实际父节点决定。 */
export function parseJsonPointer(pointer: string): JsonPointerParseResult {
  if (pointer === '') return { ok: true, segments: [] };
  if (!pointer.startsWith('/')) return { ok: false, reason: 'must_start_with_slash' };

  const segments: string[] = [];
  for (const encoded of pointer.slice(1).split('/')) {
    const decoded = unescapePointerSegment(encoded);
    if (decoded === null) return { ok: false, reason: 'invalid_escape' };
    segments.push(decoded);
  }

  return { ok: true, segments };
}

export function appendJsonPointer(pointer: string, segment: string | number): string {
  return `${pointer}/${escapePointerSegment(String(segment))}`;
}

export function parentJsonPointer(pointer: string): string | null {
  if (pointer === '') return null;
  const separator = pointer.lastIndexOf('/');
  return separator === 0 ? '' : pointer.slice(0, separator);
}

export function jsonPathFromPointerSegments(
  parentPath: readonly (string | number)[],
  parentType: 'object' | 'array',
  segment: string,
): JSONPath | null {
  if (parentType === 'object') return [...parentPath, segment];
  if (!/^(0|[1-9]\d*)$/.test(segment)) return null;

  const index = Number(segment);
  if (!Number.isSafeInteger(index)) return null;
  return [...parentPath, index];
}
