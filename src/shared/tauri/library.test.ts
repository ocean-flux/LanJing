import { describe, expect, it } from 'vitest';
import { projectLibrary, type LibraryProjectionResponse } from './library';

const projection: LibraryProjectionResponse = {
  global_seq: 4,
  entries: [
    {
      resource_id: 'ignored',
      favorite: false,
      pinned: false,
      last_opened_at: null,
      progress: null,
      revision: 1,
      updated_global_seq: 1,
    },
    {
      resource_id: 'recent',
      favorite: true,
      pinned: false,
      last_opened_at: '2026-08-15T00:00:00Z',
      progress: null,
      revision: 1,
      updated_global_seq: 2,
    },
    {
      resource_id: 'pinned',
      favorite: false,
      pinned: true,
      last_opened_at: null,
      progress: null,
      revision: 2,
      updated_global_seq: 3,
    },
  ],
};

describe('library projection', () => {
  it('keeps meaningful entries and orders pinned content first', () => {
    expect(projectLibrary(projection).map((entry) => entry.resource_id)).toEqual([
      'pinned',
      'recent',
    ]);
    expect(projection.entries).toHaveLength(3);
  });
});
