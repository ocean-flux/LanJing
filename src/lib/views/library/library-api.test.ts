import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

import { loadLibraryProjection, updateLibraryEntry } from './library-api';

describe('library RuleSystem wire', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('loads the safe library projection', async () => {
    const projection = { global_seq: 5, entries: [] };
    invoke.mockResolvedValue(projection);

    await expect(loadLibraryProjection()).resolves.toBe(projection);
    expect(invoke).toHaveBeenCalledWith('get_library_projection');
  });

  it('writes the entry with its current revision as expected_version', async () => {
    invoke.mockResolvedValue({ global_seq: 6, revision: 4 });

    await updateLibraryEntry({
      resource_id: 'item:one',
      favorite: true,
      pinned: false,
      last_opened_at: null,
      progress: null,
      revision: 3,
      updated_global_seq: 5,
    });

    expect(invoke).toHaveBeenCalledWith('update_library_entry', {
      request: {
        resource_id: 'item:one',
        favorite: true,
        pinned: false,
        last_opened_at: null,
        progress: null,
        expected_version: 3,
      },
    });
  });
});
