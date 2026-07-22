import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import NovelDetail from './NovelDetail.svelte';

describe('NovelDetail', () => {
  it('keeps an honest detail placeholder without fake chapters or reader entry', () => {
    render(NovelDetail);

    expect(screen.getByRole('heading', { name: '作品详情尚未接入' })).toBeTruthy();
    expect(screen.getByText('媒体体验稍后')).toBeTruthy();
    expect(screen.getByText(/不提供假目录或沉浸阅读入口/)).toBeTruthy();
    expect(screen.getByRole('link', { name: '返回小说' }).getAttribute('href')).toBe('/apps/novel');
    expect(screen.getByRole('link', { name: '添加来源' }).getAttribute('href')).toBe('/sources');
    expect(screen.queryByRole('heading', { name: '目录' })).toBeNull();
    expect(screen.queryByRole('link', { name: '打开阅读器' })).toBeNull();
  });
});
