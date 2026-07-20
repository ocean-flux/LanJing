import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

import { getMediaItem, getMediaItems, listMediaAssets, listMediaUnits } from './media-api';

describe('media projection RuleSystem wire', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('loads a single media item by stable resource_id', async () => {
    const item = {
      id: 'item:one',
      source_id: 'source:test',
      media_kind: 'text',
      title: '标题',
      subtitle: null,
      creators: [],
      description: null,
      cover_asset_id: null,
      completeness: 'complete',
      updated_at: null,
    };
    invoke.mockResolvedValue(item);

    await expect(getMediaItem('item:one')).resolves.toBe(item);
    expect(invoke).toHaveBeenCalledWith('get_media_item', {
      request: { resource_id: 'item:one' },
    });
  });

  it('batches media items and skips missing on the backend contract', async () => {
    invoke.mockResolvedValue([]);

    await getMediaItems(['item:a', 'item:b']);
    expect(invoke).toHaveBeenCalledWith('get_media_items', {
      request: { resource_ids: ['item:a', 'item:b'] },
    });
  });

  it('lists units with offset/limit payload', async () => {
    const page = {
      items: [],
      offset: 10,
      limit: 20,
      has_more: false,
      parent_found: false,
    };
    invoke.mockResolvedValue(page);

    await expect(listMediaUnits('item:one', 10, 20)).resolves.toBe(page);
    expect(invoke).toHaveBeenCalledWith('list_media_units', {
      request: { item_id: 'item:one', offset: 10, limit: 20 },
    });
  });

  it('lists assets and omits limit when caller uses default', async () => {
    const page = {
      items: [
        {
          id: 'asset:1',
          source_id: 'source:test',
          unit_id: 'unit:1',
          asset_kind: 'text',
          locator: { type: 'text', value: '正文' },
          completeness: 'complete',
        },
      ],
      offset: 0,
      limit: 50,
      has_more: false,
      parent_found: true,
    };
    invoke.mockResolvedValue(page);

    await expect(listMediaAssets('unit:1')).resolves.toBe(page);
    expect(invoke).toHaveBeenCalledWith('list_media_assets', {
      request: { unit_id: 'unit:1', offset: 0, limit: null },
    });
  });
});
