import { describe, expect, it } from 'vitest';
import { parseDeepLink, parseDeepLinks } from './parse';

describe('deep link parser', () => {
  it('accepts Legado bookSource imports with an encoded remote src', () => {
    const src = 'https://example.test/sources.json?channel=stable';

    expect(parseDeepLink(`legado://import/bookSource?src=${encodeURIComponent(src)}`)).toEqual({
      kind: 'install',
      scheme: 'legado',
      sourceKind: 'bookSource',
      src,
    });
  });

  it('accepts the lanjing install equivalent', () => {
    expect(
      parseDeepLink('lanjing://install/bookSource?src=https%3A%2F%2Fexample.test%2Fa.json'),
    ).toEqual({
      kind: 'install',
      scheme: 'lanjing',
      sourceKind: 'bookSource',
      src: 'https://example.test/a.json',
    });
  });

  it('rejects unsupported Legado imports and names the future type', () => {
    expect(
      parseDeepLink('legado://import/rssSource?src=https%3A%2F%2Fexample.test%2Frss.json'),
    ).toMatchObject({
      kind: 'reject',
      scheme: 'legado',
      reason: 'unsupported-import',
      subject: 'rssSource',
    });
  });

  it('rejects missing, credentialed and non-http src values', () => {
    expect(parseDeepLink('legado://import/bookSource')).toMatchObject({
      kind: 'reject',
      reason: 'missing-src',
    });
    expect(
      parseDeepLink('legado://import/bookSource?src=file%3A%2F%2F%2Ftmp%2Fsources.json'),
    ).toMatchObject({ kind: 'reject', reason: 'missing-src' });
    expect(
      parseDeepLink('legado://import/bookSource?src=https%3A%2F%2Fuser%3Apass%40example.test%2Fa'),
    ).toMatchObject({ kind: 'reject', reason: 'missing-src' });
  });

  it('maps lanjing item and source links to stable IDs', () => {
    expect(parseDeepLink('lanjing://item/item%3Asource%3Aone')).toEqual({
      kind: 'item',
      scheme: 'lanjing',
      resourceId: 'item:source:one',
    });
    expect(parseDeepLink('lanjing://source/source%3Aone')).toEqual({
      kind: 'source',
      scheme: 'lanjing',
      sourceId: 'source:one',
    });
  });

  it('preserves OS delivery order while dropping empty arguments', () => {
    expect(
      parseDeepLinks(['', 'lanjing://item/item%3Aone', 'lanjing://source/source%3Aone']).map(
        (intent) => intent.kind,
      ),
    ).toEqual(['item', 'source']);
  });
});
