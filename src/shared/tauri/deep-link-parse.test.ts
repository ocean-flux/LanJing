import { describe, expect, it } from 'vitest';
import { parseDeepLink, parseDeepLinks } from './deep-link-parse';

describe('deep-link parser', () => {
  it('parses a library item link', () => {
    expect(parseDeepLink('lanjing://item/book%2F42')).toEqual({
      kind: 'item',
      resourceId: 'book/42',
    });
  });

  it('parses legacy source imports without accepting credentials', () => {
    expect(
      parseDeepLink('legado://import/bookSource?src=https%3A%2F%2Fexample.test%2Fsources.json'),
    ).toEqual({ kind: 'install', src: 'https://example.test/sources.json' });
    expect(
      parseDeepLink('legado://import/bookSource?src=https%3A%2F%2Fuser%3Apass%40example.test'),
    ).toEqual(expect.objectContaining({ kind: 'reject', reason: 'missing-src' }));
  });

  it('preserves input order for multiple links', () => {
    expect(
      parseDeepLinks(['lanjing://source/main', '', 'lanjing://item/story-1']).map(
        (intent) => intent.kind,
      ),
    ).toEqual(['source', 'item']);
  });
});
