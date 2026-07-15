import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import NovelDetail from './NovelDetail.svelte';

describe('NovelDetail', () => {
  it('separates metadata, directory placeholder, and reader entry', () => {
    render(NovelDetail);

    expect(screen.getByRole('heading', { name: '长安的荔枝' })).toBeTruthy();
    expect(screen.getByText('来源：待接入小说源 · 状态：骨架占位')).toBeTruthy();
    expect(screen.getByRole('heading', { name: '目录' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '阅读入口' })).toBeTruthy();
    expect(screen.getAllByRole('link', { name: '打开阅读器' })[0]?.getAttribute('href')).toBe(
      '/apps/novel/read',
    );
  });
});
