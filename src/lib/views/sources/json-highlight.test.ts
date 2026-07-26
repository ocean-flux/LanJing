import { describe, expect, it } from 'vitest';
import { highlightJsonHtml, tokenizeJson, tokensToHtml } from './json-highlight';

describe('tokenizeJson', () => {
  it('labels object keys and string values distinctly', () => {
    const tokens = tokenizeJson('{"bookSourceName":"Demo"}');
    const kinds = tokens.filter((t) => t.kind !== 'ws' && t.kind !== 'punct').map((t) => t.kind);
    expect(kinds).toEqual(['key', 'string']);
  });

  it('tokenizes numbers, booleans, and null', () => {
    const tokens = tokenizeJson('{"n":12.5,"ok":true,"x":null}');
    const kinds = tokens.filter((t) => t.kind !== 'ws' && t.kind !== 'punct').map((t) => t.kind);
    expect(kinds).toEqual(['key', 'number', 'key', 'boolean', 'key', 'null']);
  });

  it('treats array strings as values, not keys', () => {
    const tokens = tokenizeJson('["a","b"]');
    const strings = tokens.filter((t) => t.kind === 'string' || t.kind === 'key');
    expect(strings.every((t) => t.kind === 'string')).toBe(true);
  });

  it('does not throw on broken JSON and still paints tokens', () => {
    expect(() => tokenizeJson('{broken: "still",')).not.toThrow();
    const tokens = tokenizeJson('{broken: "still",');
    expect(tokens.length).toBeGreaterThan(0);
    expect(tokens.some((t) => t.text.includes('still'))).toBe(true);
  });
});

describe('tokensToHtml / highlightJsonHtml', () => {
  it('escapes HTML in values', () => {
    const html = tokensToHtml(tokenizeJson('{"a":"<script>"}'));
    expect(html).not.toContain('<script>');
    expect(html).toContain('&lt;script&gt;');
  });

  it('never throws on arbitrary input', () => {
    expect(() => highlightJsonHtml('}}}{{{')).not.toThrow();
    expect(highlightJsonHtml('}}}{{{').length).toBeGreaterThan(0);
  });
});
