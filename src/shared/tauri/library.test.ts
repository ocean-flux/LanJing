import { describe, expect, it, vi } from 'vitest';
import {
  loadMediaItems,
  projectLibrary,
  updateLibraryEntry,
  type LibraryProjectionResponse,
} from './library';

const tauriBoundary = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: unknown) => Promise<unknown>>(),
  isTauri: vi.fn<() => boolean>(() => true),
}));

vi.mock('@tauri-apps/api/core', () => tauriBoundary);

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

  it('uses the current Tauri request envelopes for enrichment and updates', async () => {
    tauriBoundary.invoke
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce({ global_seq: 8, revision: 2 });

    await loadMediaItems(['item:one', 'item:two']);
    await updateLibraryEntry({
      resource_id: 'item:one',
      favorite: true,
      pinned: false,
      last_opened_at: null,
      progress: null,
      expected_version: 1,
    });

    expect(tauriBoundary.invoke).toHaveBeenNthCalledWith(1, 'get_media_items', {
      request: { resource_ids: ['item:one', 'item:two'] },
    });
    expect(tauriBoundary.invoke).toHaveBeenNthCalledWith(2, 'update_library_entry', {
      request: {
        resource_id: 'item:one',
        favorite: true,
        pinned: false,
        last_opened_at: null,
        progress: null,
        expected_version: 1,
      },
    });
  });

  it('orders favorite entries before entries that are only recently opened', () => {
    const result = projectLibrary({
      global_seq: 1,
      entries: [
        {
          resource_id: 'recent-only',
          favorite: false,
          pinned: false,
          last_opened_at: '2026-08-20T00:00:00Z',
          progress: null,
          revision: 1,
          updated_global_seq: 1,
        },
        {
          resource_id: 'favorite-only',
          favorite: true,
          pinned: false,
          last_opened_at: null,
          progress: null,
          revision: 1,
          updated_global_seq: 1,
        },
      ],
    });
    expect(result.map((entry) => entry.resource_id)).toEqual(['favorite-only', 'recent-only']);
  });
});
