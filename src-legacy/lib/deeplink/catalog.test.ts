import { describe, expect, it } from 'vitest';
import {
  CATALOG_DISPLAY_CAP,
  CATALOG_INSTALL_CAP,
  groupCatalogItems,
  parseBookSourceCatalog,
} from './catalog';

function source(name: string, group?: string, extra: Record<string, unknown> = {}) {
  return {
    bookSourceName: name,
    bookSourceUrl: `https://example.test/${encodeURIComponent(name)}`,
    ...(group ? { bookSourceGroup: group } : {}),
    ...extra,
  };
}

describe('parseBookSourceCatalog', () => {
  it('maps a single object into one pickable item', () => {
    const result = parseBookSourceCatalog(JSON.stringify(source('一号源', '玄幻')));
    expect(result).toMatchObject({
      ok: true,
      totalCount: 1,
      truncated: false,
    });
    if (!result.ok) throw new Error('expected ok');
    expect(result.items[0]).toMatchObject({
      id: 'catalog:0',
      name: '一号源',
      group: '玄幻',
    });
    expect(JSON.parse(result.items[0]!.rawJson).bookSourceUrl).toContain(
      encodeURIComponent('一号源'),
    );
  });

  it('accepts arrays and preserves stable local ids by source index', () => {
    const body = JSON.stringify([source('A', '甲'), { notASource: true }, source('B')]);
    const result = parseBookSourceCatalog(body);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.items.map((item) => item.id)).toEqual(['catalog:0', 'catalog:2']);
    expect(result.items.map((item) => item.name)).toEqual(['A', 'B']);
    expect(result.items[1]?.group).toBeNull();
  });

  it('rejects invalid JSON without throwing the body', () => {
    expect(parseBookSourceCatalog('{oops')).toEqual({ ok: false, reason: 'invalid-json' });
  });

  it('rejects non-bookSource shaped payloads', () => {
    expect(parseBookSourceCatalog(JSON.stringify({ rssSourceName: 'x' }))).toEqual({
      ok: false,
      reason: 'not-book-source',
    });
    expect(parseBookSourceCatalog(JSON.stringify([{ bookSourceName: '  ' }]))).toEqual({
      ok: false,
      reason: 'not-book-source',
    });
    expect(parseBookSourceCatalog('[]')).toEqual({ ok: false, reason: 'empty' });
  });

  it(`hard-caps display at ${CATALOG_DISPLAY_CAP} while reporting totalCount`, () => {
    const many = Array.from({ length: CATALOG_DISPLAY_CAP + 25 }, (_, index) =>
      source(`源${index}`, index % 2 === 0 ? '奇偶' : undefined),
    );
    const result = parseBookSourceCatalog(JSON.stringify(many));
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.items).toHaveLength(CATALOG_DISPLAY_CAP);
    expect(result.totalCount).toBe(CATALOG_DISPLAY_CAP + 25);
    expect(result.truncated).toBe(true);
  });

  it('exposes install selection cap constant', () => {
    expect(CATALOG_INSTALL_CAP).toBe(50);
  });
});

describe('groupCatalogItems', () => {
  it('groups by bookSourceGroup and keeps ungrouped last', () => {
    const result = parseBookSourceCatalog(
      JSON.stringify([source('A', '乙'), source('B'), source('C', '甲'), source('D', '乙')]),
    );
    if (!result.ok) throw new Error('expected ok');
    const groups = groupCatalogItems(result.items);
    expect(groups.map((group) => group.label)).toEqual(['甲', '乙', null]);
    expect(groups[1]?.items.map((item) => item.name)).toEqual(['A', 'D']);
    expect(groups[2]?.items.map((item) => item.name)).toEqual(['B']);
  });
});
