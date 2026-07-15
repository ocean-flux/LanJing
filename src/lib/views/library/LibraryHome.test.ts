import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import LibraryHome from './LibraryHome.svelte';
import type { LibraryEntry, LibraryProjectionResponse } from './library-projection';

const emptyProjection: LibraryProjectionResponse = {
  graph: {
    sources: [],
    items: [],
    collections: [],
    units: [],
    assets: [],
    relations: [],
    actions: [],
    hints: [],
  },
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

  it('renders real graph resources and writes shared state through boundary callback', async () => {
    const projection: LibraryProjectionResponse = {
      graph: {
        sources: [
          {
            id: 'source:one',
            title: '本地来源',
            icon_url: null,
            version: null,
            supported_intents: [],
            risk_notes: [],
          },
        ],
        items: [
          {
            id: 'item:one',
            source_id: 'source:one',
            media_kind: 'text',
            title: '真实资源',
            subtitle: null,
            creators: [],
            description: null,
            cover_asset_id: null,
            metadata: {},
            completeness: 'partial',
            updated_at: null,
          },
        ],
        collections: [],
        units: [],
        assets: [],
        relations: [],
        actions: [],
        hints: [],
      },
      entries: [
        {
          resource_id: 'item:one',
          favorite: true,
          pinned: false,
          last_opened_at: null,
          progress: null,
        },
      ],
    };
    let updatedResource = '';

    render(LibraryHome, {
      props: {
        projection,
        update: async (entry: LibraryEntry) => {
          updatedResource = entry.resource_id;
        },
      },
    });

    expect(screen.getByText('真实资源')).toBeTruthy();
    expect(screen.getByText('来源：本地来源')).toBeTruthy();
    await screen.getByRole('button', { name: '取消收藏' }).click();
    expect(updatedResource).toBe('item:one');
  });
});
