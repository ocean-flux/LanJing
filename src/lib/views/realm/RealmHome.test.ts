import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import RealmHome from './RealmHome.svelte';

describe('RealmHome', () => {
  it('shows one honest discovery state and one real next step', () => {
    render(RealmHome);

    expect(screen.getByRole('heading', { level: 1, name: '境场' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '当前没有已接线的发现结果' })).toBeTruthy();
    expect(screen.getByRole('status').textContent).toContain('当前没有已接线的发现结果');

    const links = screen.getAllByRole('link');
    expect(links).toHaveLength(1);
    expect(screen.getByRole('link', { name: '管理来源' }).getAttribute('href')).toBe('/sources');
    expect(screen.queryByRole('button')).toBeNull();
  });
});
