import { invoke } from '@tauri-apps/api/core';
import type { LibraryEntry, LibraryProjectionResponse } from './library-projection';

export function loadLibraryProjection(): Promise<LibraryProjectionResponse> {
  return invoke<LibraryProjectionResponse>('get_library_projection');
}

export function updateLibraryEntry(entry: LibraryEntry): Promise<void> {
  return invoke('update_library_entry', {
    request: entry,
  });
}
