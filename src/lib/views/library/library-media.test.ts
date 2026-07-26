import { describe, expect, it, vi } from 'vitest';
import type { MediaItem } from '$lib/views/media/media-api';
import {
  MEDIA_BATCH_SIZE,
  enrichLibraryItems,
  loadMediaByResourceIds,
  mediaItemsToMap,
} from './library-media';
import type { LibraryProjectionItem } from './library-projection';

const sampleMedia = (id: string, title: string): MediaItem => ({
  id,
  source_id: 'source:test',
  media_kind: 'text',
  title,
  subtitle: null,
  creators: [],
  description: null,
  cover_asset_id: null,
  completeness: 'complete',
  updated_at: null,
});

const projectionItem = (resourceId: string): LibraryProjectionItem => ({
  resource_id: resourceId,
  state: {
    resource_id: resourceId,
    favorite: true,
    pinned: false,
    last_opened_at: null,
    progress: null,
    revision: 1,
    updated_global_seq: 1,
  },
});

describe('library media enrichment', () => {
  it('batches getMediaItems requests within the backend 1..=64 limit', async () => {
    const ids = Array.from({ length: MEDIA_BATCH_SIZE + 3 }, (_, index) => `item:${index}`);
    const fetchItems = vi.fn(async (chunk: string[]) =>
      chunk.map((id) => sampleMedia(id, `Title ${id}`)),
    );

    const map = await loadMediaByResourceIds(ids, fetchItems);

    expect(fetchItems).toHaveBeenCalledTimes(2);
    expect(fetchItems.mock.calls[0]?.[0]).toHaveLength(MEDIA_BATCH_SIZE);
    expect(fetchItems.mock.calls[1]?.[0]).toHaveLength(3);
    expect(map.size).toBe(ids.length);
  });

  it('uses media title when present and degrades without inventing data', () => {
    const items = [projectionItem('item:one'), projectionItem('item:missing')];
    const mediaById = mediaItemsToMap([sampleMedia('item:one', '标准标题')]);

    const rows = enrichLibraryItems(items, mediaById);

    expect(rows[0]).toMatchObject({
      resource_id: 'item:one',
      title: '标准标题',
      kind: 'text',
      mediaMissing: false,
    });
    expect(rows[1]).toMatchObject({
      resource_id: 'item:missing',
      title: 'item:missing',
      kind: null,
      mediaMissing: true,
      media: null,
    });
  });
  it('degrades when the media item has no usable title', () => {
    const items = [projectionItem('item:blank')];
    const mediaById = mediaItemsToMap([sampleMedia('item:blank', '   ')]);

    const rows = enrichLibraryItems(items, mediaById);

    expect(rows[0]).toMatchObject({
      resource_id: 'item:blank',
      title: 'item:blank',
      mediaMissing: true,
    });
  });
});
