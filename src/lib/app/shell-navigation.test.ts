import { describe, expect, it } from 'vitest';
import { getPrimaryNavigationItems } from './shell-navigation';

describe('getPrimaryNavigationItems', () => {
  it('returns exactly four realms in product order', () => {
    const items = getPrimaryNavigationItems();
    expect(items.map((item) => item.key)).toEqual(['realm', 'apps', 'sources', 'library']);
    expect(items.map((item) => item.href)).toEqual(['/', '/apps', '/sources', '/library']);
  });

  it('excludes novel, settings, and search destinations', () => {
    const items = getPrimaryNavigationItems();
    const keys = items.map((item) => item.key);
    const hrefs = items.map((item) => item.href).join(' ');
    expect(keys).not.toContain('settings' as never);
    expect(keys).not.toContain('novel' as never);
    expect(hrefs).not.toMatch(/settings|novel|search/);
    expect(items).toHaveLength(4);
  });
});
