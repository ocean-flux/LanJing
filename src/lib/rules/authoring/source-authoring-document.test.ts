import { describe, expect, it } from 'vitest';
import {
  SourceAuthoringDocument,
  authoringLimits,
  legadoFieldCatalog,
  parseJsonPointer,
  toJsonPointer,
  type SourceAuthoringEditResult,
} from './index';

const CREDENTIAL_SENTINEL = '__LANJING_CREDENTIAL_SLOT_V1__:11111111-1111-4111-8111-111111111111';
const REQUIRED_SOURCE_FIELDS =
  '"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"源"';
const encoder = new TextEncoder();

function diagnosticCodes(document: SourceAuthoringDocument): string[] {
  return document.diagnostics.map((entry) => entry.code);
}

function expectApplied(result: SourceAuthoringEditResult) {
  expect(result.kind).toBe('applied');
  if (result.kind !== 'applied') throw new Error(`Expected applied edit, received ${result.kind}`);
  return result;
}

function documentWithExactUtf8Bytes(targetBytes: number): string {
  const prefix = '{"chunks":[';
  const suffix = ']}';

  for (let chunkCount = 1; chunkCount <= 64; chunkCount += 1) {
    const fixedBytes = prefix.length + suffix.length + (chunkCount - 1) + chunkCount * 2;
    const payloadBytes = targetBytes - fixedBytes;
    if (payloadBytes < 0 || payloadBytes > chunkCount * authoringLimits.max_string_utf8_bytes) {
      continue;
    }

    const baseLength = Math.floor(payloadBytes / chunkCount);
    const remainder = payloadBytes % chunkCount;
    const chunks = Array.from({ length: chunkCount }, (_, index) => {
      const length = baseLength + (index < remainder ? 1 : 0);
      return `"${'x'.repeat(length)}"`;
    });
    const text = `${prefix}${chunks.join(',')}${suffix}`;
    expect(encoder.encode(text)).toHaveLength(targetBytes);
    return text;
  }

  throw new Error(`Unable to construct ${targetBytes} UTF-8 bytes within catalog limits`);
}

function documentWithDepth(maximumDepth: number): string {
  const arrayCount = maximumDepth - 2;
  return `{"value":${'['.repeat(arrayCount)}0${']'.repeat(arrayCount)}}`;
}

function documentWithNodeCount(nodeCount: number): string {
  const scalarCount = nodeCount - 2;
  return `{"items":[${Array.from({ length: scalarCount }, () => '0').join(',')}]}`;
}

function documentWithPropertyCount(propertyCount: number): string {
  const properties = Array.from({ length: propertyCount }, (_, index) => `"p${index}":0`);
  return `{${properties.join(',')}}`;
}

describe('SourceAuthoringDocument catalog limits', () => {
  it('reads the generated catalog as the only limits owner', () => {
    expect(authoringLimits).toBe(legadoFieldCatalog.limits);
    expect(legadoFieldCatalog.schema_version).toBe(1);
    expect(legadoFieldCatalog.generator_version).toBe(1);
  });

  it('accepts the document byte boundary and rejects one UTF-8 byte beyond it', () => {
    const base = SourceAuthoringDocument.open(
      documentWithExactUtf8Bytes(authoringLimits.max_utf8_bytes),
    );
    expect(diagnosticCodes(base)).not.toContain('document_bytes_exceeded');
    expect(diagnosticCodes(base)).not.toContain('string_utf8_bytes_exceeded');

    const bad = SourceAuthoringDocument.open(
      documentWithExactUtf8Bytes(authoringLimits.max_utf8_bytes + 1),
    );
    expect(bad.diagnostics).toEqual([
      {
        severity: 'error',
        code: 'document_bytes_exceeded',
        path: '',
        byte_offset: authoringLimits.max_utf8_bytes,
        byte_length: 1,
        support: 'blocked',
      },
    ]);
  });

  it('accepts the depth boundary and rejects the next nested JSON value', () => {
    const base = SourceAuthoringDocument.open(documentWithDepth(authoringLimits.max_depth));
    expect(diagnosticCodes(base)).not.toContain('depth_exceeded');

    const bad = SourceAuthoringDocument.open(documentWithDepth(authoringLimits.max_depth + 1));
    expect(diagnosticCodes(bad)).toContain('depth_exceeded');
  });

  it('accepts the node boundary and rejects the next JSON value', () => {
    const base = SourceAuthoringDocument.open(documentWithNodeCount(authoringLimits.max_nodes));
    expect(diagnosticCodes(base)).not.toContain('node_count_exceeded');

    const bad = SourceAuthoringDocument.open(documentWithNodeCount(authoringLimits.max_nodes + 1));
    expect(diagnosticCodes(bad)).toContain('node_count_exceeded');
  });

  it('accepts the total property boundary and rejects the next occurrence', () => {
    const base = SourceAuthoringDocument.open(
      documentWithPropertyCount(authoringLimits.max_properties),
    );
    expect(diagnosticCodes(base)).not.toContain('property_count_exceeded');

    const bad = SourceAuthoringDocument.open(
      documentWithPropertyCount(authoringLimits.max_properties + 1),
    );
    expect(diagnosticCodes(bad)).toContain('property_count_exceeded');
  });

  it('counts decoded UTF-8 bytes for property names', () => {
    const baseName = '界'.repeat(Math.floor(authoringLimits.max_property_name_utf8_bytes / 3));
    const basePadding = 'a'.repeat(
      authoringLimits.max_property_name_utf8_bytes - encoder.encode(baseName).length,
    );
    const base = SourceAuthoringDocument.open(`{"${baseName}${basePadding}":0}`);
    expect(diagnosticCodes(base)).not.toContain('property_name_utf8_bytes_exceeded');

    const bad = SourceAuthoringDocument.open(`{"${baseName}${basePadding}a":0}`);
    expect(diagnosticCodes(bad)).toContain('property_name_utf8_bytes_exceeded');
  });

  it('counts decoded UTF-8 bytes for string values', () => {
    const baseValue = '界'.repeat(Math.floor(authoringLimits.max_string_utf8_bytes / 3));
    const basePadding = 'a'.repeat(
      authoringLimits.max_string_utf8_bytes - encoder.encode(baseValue).length,
    );
    const base = SourceAuthoringDocument.open(`{"value":"${baseValue}${basePadding}"}`);
    expect(diagnosticCodes(base)).not.toContain('string_utf8_bytes_exceeded');

    const bad = SourceAuthoringDocument.open(`{"value":"${baseValue}${basePadding}a"}`);
    expect(diagnosticCodes(bad)).toContain('string_utf8_bytes_exceeded');
  });
});

describe('SourceAuthoringDocument snapshot boundary', () => {
  it('exposes a frozen syntax tree and a map view without mutators', () => {
    const document = SourceAuthoringDocument.open(`{${REQUIRED_SOURCE_FIELDS}}`);
    const root = document.syntaxTree;

    expect(root).toBeDefined();
    if (!root) throw new Error('Expected syntax tree');
    expect(Object.isFrozen(root)).toBe(true);
    expect(Object.isFrozen(root.children)).toBe(true);
    expect(Reflect.set(root, 'type', 'array')).toBe(false);
    expect('set' in document.pointerIndex).toBe(false);

    const sourceUrl = document.lookupPointer('/bookSourceUrl');
    expect(sourceUrl.kind).toBe('found');
    if (sourceUrl.kind !== 'found') throw new Error('Expected /bookSourceUrl pointer');
    expect(Object.isFrozen(sourceUrl.entry)).toBe(true);
    expect(Object.isFrozen(sourceUrl.entry.node)).toBe(true);
  });
});

describe('SourceAuthoringDocument diagnostics', () => {
  it('rejects non-object roots with a root pointer and raw UTF-8 span', () => {
    for (const text of ['[]', 'null', '"value"']) {
      const document = SourceAuthoringDocument.open(text);
      expect(document.diagnostics).toEqual([
        {
          severity: 'error',
          code: 'invalid_root',
          path: '',
          byte_offset: 0,
          byte_length: encoder.encode(text).length,
          support: 'blocked',
        },
      ]);
    }
  });

  it('rejects comments and trailing commas instead of accepting JSONC', () => {
    for (const text of ['{"value":1,}', '{/* comment */"value":1}']) {
      const document = SourceAuthoringDocument.open(text);
      expect(diagnosticCodes(document)).toContain('invalid_json');
    }
  });

  it('reports the repeated key token and RFC 6901 property pointer', () => {
    const text = '{"标题":"🙂","duplicate":1,"duplicate":2}';
    const document = SourceAuthoringDocument.open(text);
    const duplicate = document.diagnostics.find((entry) => entry.code === 'duplicate_key');
    const secondKeyOffset = text.lastIndexOf('"duplicate"');

    expect(duplicate).toEqual({
      severity: 'error',
      code: 'duplicate_key',
      path: '/duplicate',
      byte_offset: encoder.encode(text.slice(0, secondKeyOffset)).length,
      byte_length: encoder.encode('"duplicate"').length,
      support: 'blocked',
    });
    expect(document.lookupPointer('/duplicate').kind).toBe('ambiguous');
  });

  it('converts parser UTF-16 offsets to original UTF-8 byte spans', () => {
    const text = '{"标题":"🙂","broken": }';
    const document = SourceAuthoringDocument.open(text);
    const invalid = document.diagnostics.find((entry) => entry.code === 'invalid_json');
    const closingBraceOffset = text.indexOf('}');

    expect(invalid?.byte_offset).toBe(encoder.encode(text.slice(0, closingBraceOffset)).length);
    expect(invalid?.byte_length).toBe(1);
    expect(invalid?.path).toBe('');
  });

  it('indexes escaped object names with RFC 6901 escaping', () => {
    const document = SourceAuthoringDocument.open('{"a/b":{"~key":1}}');

    expect(parseJsonPointer('/a~1b/~0key')).toEqual({ ok: true, segments: ['a/b', '~key'] });
    expect(toJsonPointer(['a/b', '~key'])).toBe('/a~1b/~0key');
    expect(document.lookupPointer('/a~1b/~0key').kind).toBe('found');
    expect(document.lookupPointer('/a/b/~key').kind).toBe('invalid');
  });

  it('classifies properties from the generated catalog without a TypeScript field list', () => {
    const unknownPointer = '/__lanjing_authoring_unknown_fixture__';
    expect(legadoFieldCatalog.fields.some((field) => field.pointer === unknownPointer)).toBe(false);

    const unknownKeyToken = '"__lanjing_authoring_unknown_fixture__"';
    const text = `{${REQUIRED_SOURCE_FIELDS},${unknownKeyToken}:true}`;
    const document = SourceAuthoringDocument.open(text);
    const keyOffset = text.indexOf(unknownKeyToken);
    expect(document.diagnostics).toEqual([
      {
        severity: 'warning',
        code: 'unknown_field',
        path: unknownPointer,
        byte_offset: encoder.encode(text.slice(0, keyOffset)).length,
        byte_length: unknownKeyToken.length,
        support: 'unknown',
      },
    ]);
  });

  it('preserves valid credential sentinels and diagnoses malformed slot UUIDs', () => {
    const valid = SourceAuthoringDocument.open(
      `{${REQUIRED_SOURCE_FIELDS},"header":"${CREDENTIAL_SENTINEL}"}`,
    );
    expect(valid.diagnostics).toEqual([]);

    const malformedValue = '__LANJING_CREDENTIAL_SLOT_V1__:not-a-uuid';
    const malformedText = `{${REQUIRED_SOURCE_FIELDS},"header":"${malformedValue}"}`;
    const malformed = SourceAuthoringDocument.open(malformedText);
    const valueOffset = malformedText.indexOf(`"${malformedValue}"`);

    expect(malformed.diagnostics).toEqual([
      {
        severity: 'error',
        code: 'credential_sentinel_invalid',
        path: '/header',
        byte_offset: encoder.encode(malformedText.slice(0, valueOffset)).length,
        byte_length: encoder.encode(`"${malformedValue}"`).length,
        support: 'blocked',
      },
    ]);
  });

  it('reports missing required root fields at the root opening token', () => {
    const document = SourceAuthoringDocument.open('{}');
    expect(document.diagnostics).toEqual(
      ['/bookSourceName', '/bookSourceType', '/bookSourceUrl'].map((path) => ({
        severity: 'error',
        code: 'required_field_missing',
        path,
        byte_offset: 0,
        byte_length: 1,
        support: 'blocked',
      })),
    );
  });

  it('blocks invalid catalog types and compact rule-container strings on value spans', () => {
    const invalidTypeText =
      '{"bookSourceType":"0","bookSourceUrl":"https://example.test","bookSourceName":"源"}';
    const invalidType = SourceAuthoringDocument.open(invalidTypeText);
    const invalidValueOffset = invalidTypeText.indexOf('"0"');
    expect(invalidType.diagnostics).toEqual([
      {
        severity: 'error',
        code: 'invalid_field_type',
        path: '/bookSourceType',
        byte_offset: invalidValueOffset,
        byte_length: 3,
        support: 'blocked',
      },
    ]);

    const compactText = `{${REQUIRED_SOURCE_FIELDS},"ruleSearch":"name"}`;
    const compact = SourceAuthoringDocument.open(compactText);
    const compactValueOffset = compactText.indexOf('"name"');
    expect(compact.diagnostics).toEqual([
      {
        severity: 'error',
        code: 'known_field_blocked',
        path: '/ruleSearch',
        byte_offset: encoder.encode(compactText.slice(0, compactValueOffset)).length,
        byte_length: 6,
        support: 'blocked',
      },
    ]);
  });

  it('allows an explicitly disabled CookieJar but blocks enabling it', () => {
    const disabled = SourceAuthoringDocument.open(
      `{${REQUIRED_SOURCE_FIELDS},"enabledCookieJar":false}`,
    );
    expect(disabled.diagnostics.some((entry) => entry.path === '/enabledCookieJar')).toBe(false);

    const enabled = SourceAuthoringDocument.open(
      `{${REQUIRED_SOURCE_FIELDS},"enabledCookieJar":true}`,
    );
    expect(enabled.diagnostics).toEqual([
      expect.objectContaining({
        code: 'known_field_blocked',
        path: '/enabledCookieJar',
        support: 'blocked',
      }),
    ]);
  });

  it('preserves explicit null for optional upstream fields', () => {
    const document = SourceAuthoringDocument.open(
      `{${REQUIRED_SOURCE_FIELDS},"loginUrl":null,"ruleSearch":null,"lastUpdateTime":null}`,
    );

    expect(
      document.diagnostics.filter((entry) =>
        ['/loginUrl', '/ruleSearch', '/lastUpdateTime'].includes(entry.path),
      ),
    ).toEqual([]);
  });

  it('blocks unsupported source type, plain explore behavior and opaque rule languages', () => {
    for (const [text, path] of [
      [
        '{"bookSourceType":1,"bookSourceUrl":"https://example.test","bookSourceName":"源"}',
        '/bookSourceType',
      ],
      [`{${REQUIRED_SOURCE_FIELDS},"exploreUrl":"/discover?page={{page}}"}`, '/exploreUrl'],
      [`{${REQUIRED_SOURCE_FIELDS},"ruleSearch":{"name":"@js:return secret"}}`, '/ruleSearch/name'],
      [
        `{${REQUIRED_SOURCE_FIELDS},"ruleSearch":{"name":"@xpath://h1/text()"}}`,
        '/ruleSearch/name',
      ],
    ] as const) {
      expect(SourceAuthoringDocument.open(text).diagnostics).toEqual([
        expect.objectContaining({
          code: 'known_field_blocked',
          path,
          support: 'blocked',
        }),
      ]);
    }
  });
});

describe('SourceAuthoringDocument typed patches', () => {
  it('sets an intermediate value with one minimal edit and preserves untouched text', () => {
    const text = [
      '{',
      '  "unknown": { "untouched": "  spaced  " },',
      '  "nested": {',
      '    "value": 1,',
      `    "credential": "${CREDENTIAL_SENTINEL}"`,
      '  }',
      '}',
      '',
    ].join('\r\n');
    const document = SourceAuthoringDocument.open(text);

    const result = expectApplied(
      document.applyPatch({
        operation: 'set',
        pointer: '/nested/value',
        value: 2,
        expectedEpoch: 0,
      }),
    );

    expect(result.text).toBe(text.replace('"value": 1', '"value": 2'));
    expect(result.edits).toHaveLength(1);
    expect(result.edits[0]?.text).toBe('2');
    expect(result.epoch).toBe(1);
    expect(result.text).toContain('"unknown": { "untouched": "  spaced  " }');
    expect(result.text).toContain(CREDENTIAL_SENTINEL);
  });

  it('sets the root while preserving trailing text outside the root span', () => {
    const document = SourceAuthoringDocument.open('{"old":1}\n');
    const result = expectApplied(
      document.applyPatch({
        operation: 'set',
        pointer: '',
        value: { replacement: true },
        expectedEpoch: 0,
      }),
    );

    expect(result.text).toBe('{"replacement":true}\n');
    expect(result.edits).toHaveLength(1);
    expect(result.epoch).toBe(1);
  });

  it('adds and removes object properties without reordering untouched properties', () => {
    const document = SourceAuthoringDocument.open('{"first":1, "last":3}');
    const added = expectApplied(
      document.applyPatch({ operation: 'set', pointer: '/middle', value: 2, expectedEpoch: 0 }),
    );

    expect(added.text.indexOf('"first"')).toBeLessThan(added.text.indexOf('"last"'));
    expect(added.text.indexOf('"last"')).toBeLessThan(added.text.indexOf('"middle"'));
    expect(added.text).toContain('"first":1, "last":3');

    const removed = expectApplied(
      document.applyPatch({ operation: 'remove', pointer: '/middle', expectedEpoch: 1 }),
    );
    expect(removed.text).toBe('{"first":1, "last":3}');
    expect(removed.epoch).toBe(2);
  });

  it('sets, inserts and removes array values with monotonically increasing epochs', () => {
    const document = SourceAuthoringDocument.open('{"items":[1,  2,3],"tail":"keep"}');

    const set = expectApplied(
      document.applyPatch({ operation: 'set', pointer: '/items/1', value: 20, expectedEpoch: 0 }),
    );
    expect(set.text).toBe('{"items":[1,  20,3],"tail":"keep"}');

    const inserted = expectApplied(
      document.applyPatch({
        operation: 'insert',
        pointer: '/items/1',
        value: 15,
        expectedEpoch: 1,
      }),
    );
    expect(inserted.text).toBe('{"items":[1,15,  20,3],"tail":"keep"}');

    const removed = expectApplied(
      document.applyPatch({ operation: 'remove', pointer: '/items/2', expectedEpoch: 2 }),
    );
    expect(removed.text).toBe('{"items":[1,15,  3],"tail":"keep"}');
    expect(removed.epoch).toBe(3);
    expect(removed.text).toContain('"tail":"keep"');
  });

  it('patches RFC 6901 escaped pointers without touching sibling text', () => {
    const document = SourceAuthoringDocument.open('{"a/b":{"~key":1,"keep":" exact "}}');
    const result = expectApplied(
      document.applyPatch({
        operation: 'set',
        pointer: '/a~1b/~0key',
        value: 2,
        expectedEpoch: 0,
      }),
    );

    expect(result.text).toBe('{"a/b":{"~key":2,"keep":" exact "}}');
  });

  it('rejects stale epochs without changing text or epoch', () => {
    const document = SourceAuthoringDocument.open('{"value":1}');
    expectApplied(
      document.applyPatch({ operation: 'set', pointer: '/value', value: 2, expectedEpoch: 0 }),
    );
    const currentText = document.text;

    const stale = document.applyPatch({
      operation: 'set',
      pointer: '/value',
      value: 3,
      expectedEpoch: 0,
    });

    expect(stale).toMatchObject({
      kind: 'rejected',
      text: currentText,
      epoch: 1,
      diagnostic: { code: 'epoch_stale' },
    });
    expect(document.text).toBe(currentText);
    expect(document.epoch).toBe(1);
  });

  it('rejects invalid, missing and ambiguous pointers without mutation', () => {
    const missingDocument = SourceAuthoringDocument.open('{"value":1}');
    const missing = missingDocument.applyPatch({
      operation: 'remove',
      pointer: '/missing',
      expectedEpoch: 0,
    });
    expect(missing).toMatchObject({ kind: 'rejected', diagnostic: { code: 'pointer_missing' } });

    const invalid = missingDocument.applyPatch({
      operation: 'set',
      pointer: 'value',
      value: 2,
      expectedEpoch: 0,
    });
    expect(invalid).toMatchObject({ kind: 'rejected', diagnostic: { code: 'pointer_invalid' } });
    expect(missingDocument.text).toBe('{"value":1}');
    expect(missingDocument.epoch).toBe(0);

    const ambiguousDocument = SourceAuthoringDocument.open('{"value":1,"value":2}');
    const ambiguous = ambiguousDocument.applyPatch({
      operation: 'set',
      pointer: '/value',
      value: 3,
      expectedEpoch: 0,
    });
    expect(ambiguous).toMatchObject({
      kind: 'rejected',
      diagnostic: { code: 'pointer_ambiguous', path: '/value' },
    });
    expect(ambiguousDocument.text).toBe('{"value":1,"value":2}');
    expect(ambiguousDocument.epoch).toBe(0);
  });

  it('rejects structurally invalid and over-limit documents without mutation', () => {
    const invalid = SourceAuthoringDocument.open('{"value":');
    const invalidPatch = invalid.applyPatch({
      operation: 'set',
      pointer: '',
      value: { value: 1 },
      expectedEpoch: 0,
    });
    expect(invalidPatch).toMatchObject({
      kind: 'rejected',
      diagnostic: { code: 'invalid_json' },
    });
    expect(invalid.text).toBe('{"value":');
    expect(invalid.epoch).toBe(0);

    const overLimitText = `{"value":"${'x'.repeat(authoringLimits.max_string_utf8_bytes + 1)}"}`;
    const overLimit = SourceAuthoringDocument.open(overLimitText);
    const overLimitPatch = overLimit.applyPatch({
      operation: 'remove',
      pointer: '/value',
      expectedEpoch: 0,
    });
    expect(overLimitPatch).toMatchObject({
      kind: 'rejected',
      diagnostic: { code: 'string_utf8_bytes_exceeded' },
    });
    expect(overLimit.text).toBe(overLimitText);
    expect(overLimit.epoch).toBe(0);
  });

  it('keeps raw text as truth and formats only after an explicit request', () => {
    const text = `{"unknown":{"sentinel":"${CREDENTIAL_SENTINEL}"},"value":1}`;
    const document = SourceAuthoringDocument.open(text);

    expect(document.text).toBe(text);
    expect(document.epoch).toBe(0);

    const formatted = expectApplied(document.format({ expectedEpoch: 0 }));
    expect(formatted.epoch).toBe(1);
    expect(formatted.text).toContain(CREDENTIAL_SENTINEL);
    expect(formatted.text).not.toBe(text);
  });

  it('accepts explicit raw edits, including temporarily invalid text, as a new epoch', () => {
    const document = SourceAuthoringDocument.open('{"value":1}');
    const replaced = expectApplied(document.replaceText({ expectedEpoch: 0, text: '{"value":' }));

    expect(replaced.text).toBe('{"value":');
    expect(replaced.epoch).toBe(1);
    expect(replaced.diagnostics.some((entry) => entry.code === 'invalid_json')).toBe(true);
  });
});
