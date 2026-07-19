import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import LibraryHome from './LibraryHome.svelte';
import type { LibraryEntry, LibraryProjectionResponse } from './library-projection';

const emptyProjection: LibraryProjectionResponse = {
  global_seq: 0,
  entries: [],
};

describe('LibraryHome', () => {
  it('shows empty-first actions instead of a button grid', () => {
    render(LibraryHome, { props: { projection: emptyProjection } });

    expect(screen.getAllByRole('heading', { name: '资料库' })[0]).toBeTruthy();
    expect(screen.getAllByText('收藏、历史和缓存会在这里汇聚。').length).toBeGreaterThan(0);
    expect(screen.getByRole('status').textContent).toContain('资料库还没有资源');
    expect(screen.getAllByRole('link', { name: /添加来源/ })[0]?.getAttribute('href')).toBe(
      '/sources',
    );
    expect(screen.getAllByRole('link', { name: /导入本地文件/ })[0]?.getAttribute('href')).toBe(
      '/sources',
    );
    expect(screen.getByRole('link', { name: /搜索内容/ }).getAttribute('href')).toBe('/apps');
  });

  it('renders safe library entries and writes their current revision', async () => {
    const projection: LibraryProjectionResponse = {
      global_seq: 8,
      entries: [
        {
          resource_id: 'item:one',
          favorite: true,
          pinned: false,
          last_opened_at: null,
          progress: null,
          revision: 3,
          updated_global_seq: 8,
        },
      ],
    };
    let updatedEntry: LibraryEntry | null = null;

    render(LibraryHome, {
      props: {
        projection,
        update: async (entry: LibraryEntry) => {
          updatedEntry = entry;
          return { global_seq: 9, revision: 4 };
        },
      },
    });

    expect(screen.getByText('item:one')).toBeTruthy();
    await screen.getByRole('button', { name: '取消收藏' }).click();
    expect(updatedEntry).toMatchObject({
      resource_id: 'item:one',
      favorite: false,
      revision: 3,
    });
  });
});
