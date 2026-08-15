import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import AppsHome from './AppsHome.svelte';

describe('AppsHome', () => {
  it('shows one honest empty state without demo cards or dead actions', () => {
    render(AppsHome);

    expect(screen.getByRole('heading', { level: 1, name: '应用' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '当前没有已接线的媒体应用' })).toBeTruthy();
    expect(screen.getByRole('status').textContent).toContain('当前没有已接线的媒体应用');

    const links = screen.getAllByRole('link');
    expect(links).toHaveLength(1);
    expect(screen.getByRole('link', { name: '管理来源' }).getAttribute('href')).toBe('/sources');
    expect(screen.queryByRole('button')).toBeNull();
    expect(screen.queryByText('小说')).toBeNull();
  });
});
