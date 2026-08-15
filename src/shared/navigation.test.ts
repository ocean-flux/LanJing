import { describe, expect, it } from 'vitest';
import { getNavigationItems, isNavigationActive } from './navigation';

describe('navigation', () => {
  it('keeps nested source routes highlighted', () => {
    expect(isNavigationActive('/sources/rules', '/sources')).toBe(true);
    expect(isNavigationActive('/sources', '/')).toBe(false);
  });

  it('exposes the five primary workspaces', () => {
    expect(getNavigationItems().map((item) => item.href)).toEqual([
      '/',
      '/apps',
      '/sources',
      '/library',
      '/settings',
    ]);
  });
});
