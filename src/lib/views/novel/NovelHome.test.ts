import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import NovelHome from './NovelHome.svelte';

describe('NovelHome', () => {
  it('shows an honest media-later placeholder without fake shelf actions', () => {
    render(NovelHome);

    expect(screen.getByRole('heading', { name: '小说' })).toBeTruthy();
    expect(screen.getByRole('status')).toBeTruthy();
    expect(screen.getByText('媒体体验稍后')).toBeTruthy();
    expect(screen.getByText(/阅读沉浸体验尚未接入/)).toBeTruthy();
    expect(screen.getByRole('link', { name: '添加来源' }).getAttribute('href')).toBe('/sources');
    expect(screen.getByRole('link', { name: '应用' }).getAttribute('href')).toBe('/apps');
    expect(screen.queryByRole('button', { name: /添加小说源/ })).toBeNull();
    expect(screen.queryByRole('link', { name: /查看详情页骨架/ })).toBeNull();
  });
});
