import { describe, expect, it } from 'vitest';
import { projectLibrary, type LibraryProjectionResponse } from './library-projection';

const projection: LibraryProjectionResponse = {
  graph: {
    sources: [
      {
        id: 'source:one',
        title: '本地来源',
        icon_url: null,
        version: null,
        supported_intents: ['Search'],
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
        creators: ['作者'],
        description: null,
        cover_asset_id: null,
        metadata: {},
        completeness: 'partial',
        updated_at: null,
      },
      {
        id: 'item:unowned',
        source_id: 'source:one',
        media_kind: 'text',
        title: '未入库资源',
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
    relations: [
      {
        source_id: 'source:one',
        from_id: 'item:one',
        to_id: 'item:unowned',
        relation_kind: 'similar',
      },
    ],
    actions: [],
    hints: [],
  },
  entries: [
    {
      resource_id: 'item:one',
      favorite: true,
      pinned: false,
      last_opened_at: '2026-07-15T10:00:00Z',
      progress: null,
    },
  ],
};

describe('projectLibrary', () => {
  it('projects owned standard resources and source attribution without inventing routes', () => {
    const items = projectLibrary(projection);

    expect(items).toHaveLength(1);
    expect(items[0].item.title).toBe('真实资源');
    expect(items[0].source?.title).toBe('本地来源');
    expect(items[0].alternativeRoutes).toEqual([]);
  });

  it('deduplicates graph and state updates by stable resource identity', () => {
    const item = projection.graph.items?.[0];
    const entry = projection.entries[0];
    if (!item || !entry) throw new Error('测试资源缺失');
    const updated = projectLibrary({
      ...projection,
      graph: { ...projection.graph, items: [...(projection.graph.items ?? []), item] },
      entries: [entry, { ...entry, pinned: true }],
    });

    expect(updated).toHaveLength(1);
    expect(updated[0].state.pinned).toBe(true);
  });
});
