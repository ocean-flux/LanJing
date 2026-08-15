import { describe, expect, it } from 'vitest';
import { getNavigationItems, isNavigationActive } from './navigation';

describe('navigation', () => {
  it('keeps nested source routes highlighted', () => {
    expect(isNavigationActive('/sources/rules', '/sources')).toBe(true);
    expect(isNavigationActive('/sources', '/')).toBe(false);
  });

  it('exposes the four primary workspaces; settings lives in the sidebar footer', () => {
    expect(getNavigationItems().map((item) => item.href)).toEqual([
      '/',
      '/apps',
      '/sources',
      '/library',
    ]);
  });

  it('nests the rule workspace under sources', () => {
    const sources = getNavigationItems().find((item) => item.href === '/sources');
    expect(sources?.children?.map((child) => child.href)).toEqual(['/sources/rules']);
  });
});
