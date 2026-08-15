import { invoke } from '@tauri-apps/api/core';
import type { LibraryEntry, LibraryProjectionResponse } from './library-projection';

export interface LibraryUpdateReceipt {
  global_seq: number;
  revision: number;
}

export function loadLibraryProjection(): Promise<LibraryProjectionResponse> {
  return invoke<LibraryProjectionResponse>('get_library_projection');
}

export function updateLibraryEntry(entry: LibraryEntry): Promise<LibraryUpdateReceipt> {
  return invoke<LibraryUpdateReceipt>('update_library_entry', {
    request: {
      resource_id: entry.resource_id,
      favorite: entry.favorite,
      pinned: entry.pinned,
      last_opened_at: entry.last_opened_at,
      progress: entry.progress,
      expected_version: entry.revision,
    },
  });
}
