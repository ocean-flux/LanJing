import { invoke, isTauri } from '@tauri-apps/api/core';
import type { MediaItem } from '@/shared/types/media';

export interface LibraryEntryUpdateRequest {
  resource_id: string;
  favorite: boolean;
  pinned: boolean;
  last_opened_at: string | null;
  progress: LibraryProgress | null;
  expected_version: number;
}

export interface LibraryUpdateReceipt {
  global_seq: number;
  revision: number;
}

export interface LibraryProgress {
  unit_id: string | null;
  position: number;
  total: number | null;
}

export interface LibraryEntry {
  resource_id: string;
  favorite: boolean;
  pinned: boolean;
  last_opened_at: string | null;
  progress: LibraryProgress | null;
  revision: number;
  updated_global_seq: number;
}

export interface LibraryProjectionResponse {
  global_seq: number;
  entries: LibraryEntry[];
}

export function loadLibraryProjection(): Promise<LibraryProjectionResponse> {
  if (!isTauri()) return Promise.resolve({ global_seq: 0, entries: [] });
  return invoke<LibraryProjectionResponse>('get_library_projection');
}

export function loadMediaItems(resourceIds: string[]): Promise<MediaItem[]> {
  if (resourceIds.length === 0 || !isTauri()) return Promise.resolve([]);
  return invoke<MediaItem[]>('get_media_items', {
    request: { resource_ids: resourceIds },
  });
}

export function updateLibraryEntry(
  request: LibraryEntryUpdateRequest,
): Promise<LibraryUpdateReceipt> {
  if (!isTauri()) return Promise.reject(new Error('library_update_unavailable'));
  return invoke<LibraryUpdateReceipt>('update_library_entry', { request });
}

export function projectLibrary(projection: LibraryProjectionResponse): LibraryEntry[] {
  return projection.entries
    .filter(
      (entry) =>
        entry.favorite || entry.pinned || entry.last_opened_at !== null || entry.progress !== null,
    )
    .sort((left: LibraryEntry, right: LibraryEntry) => {
      if (left.pinned !== right.pinned) return left.pinned ? -1 : 1;
      if (left.favorite !== right.favorite) return left.favorite ? -1 : 1;
      if (left.last_opened_at !== right.last_opened_at) {
        return (right.last_opened_at ?? '').localeCompare(left.last_opened_at ?? '');
      }
      return left.resource_id.localeCompare(right.resource_id);
    });
}
