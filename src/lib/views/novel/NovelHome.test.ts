import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import NovelHome from './NovelHome.svelte';

describe('NovelHome', () => {
  it('offers novel source, local book import, and search skeleton actions', () => {
    render(NovelHome);

    expect(screen.getByRole('heading', { name: '小说' })).toBeTruthy();
    expect(screen.getByRole('button', { name: /添加小说源/ })).toBeTruthy();
    expect(screen.getByRole('button', { name: /导入本地书籍/ })).toBeTruthy();
    expect(screen.getByRole('button', { name: /搜索已有来源/ })).toBeTruthy();
    expect(screen.getByRole('link', { name: '查看详情页骨架' }).getAttribute('href')).toBe(
      '/apps/novel/item',
    );
  });
});
