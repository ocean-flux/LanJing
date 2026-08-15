import { describe, expect, it } from 'vitest';
import { projectLibrary, type LibraryProjectionResponse } from './library-projection';

const projection: LibraryProjectionResponse = {
  global_seq: 12,
  entries: [
    {
      resource_id: 'item:one',
      favorite: true,
      pinned: false,
      last_opened_at: '2026-07-15T10:00:00Z',
      progress: null,
      revision: 2,
      updated_global_seq: 12,
    },
    {
      resource_id: 'item:unowned',
      favorite: false,
      pinned: false,
      last_opened_at: null,
      progress: null,
      revision: 1,
      updated_global_seq: 11,
    },
  ],
};

describe('projectLibrary', () => {
  it('projects only owned safe library entries without reading a media graph', () => {
    const items = projectLibrary(projection);

    expect(items).toEqual([
      {
        resource_id: 'item:one',
        state: projection.entries[0],
      },
    ]);
  });

  it('orders pinned entries ahead of most recently opened entries', () => {
    const items = projectLibrary({
      ...projection,
      entries: [
        ...projection.entries,
        {
          resource_id: 'item:pinned',
          favorite: false,
          pinned: true,
          last_opened_at: null,
          progress: null,
          revision: 1,
          updated_global_seq: 12,
        },
      ],
    });

    expect(items.map((item) => item.resource_id)).toEqual(['item:pinned', 'item:one']);
  });
});
