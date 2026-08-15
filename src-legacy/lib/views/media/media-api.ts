/**
 * 标准媒体投影查询 wire：按稳定 ID 恢复 Item → Unit → Asset。
 *
 * 只读持久投影，不订阅 live execution，不暴露 Plan/effect/secret。
 */

import { invoke } from '@tauri-apps/api/core';

export type MediaAssetLocator =
  | { type: 'text'; value: string }
  | { type: 'url'; value: string }
  | { type: 'file_path'; value: string }
  | { type: 'bytes'; value: number[] }
  | { type: 'unresolved' };

export interface MediaItem {
  id: string;
  source_id: string;
  media_kind: string;
  title: string;
  subtitle: string | null;
  creators: string[];
  description: string | null;
  cover_asset_id: string | null;
  metadata: Record<string, unknown>;
  completeness: string;
  updated_at: string | null;
}

export interface MediaUnit {
  id: string;
  source_id: string;
  item_id: string;
  title: string;
  position: number | null;
  metadata: Record<string, unknown>;
  completeness: string;
}

export interface MediaAsset {
  id: string;
  source_id: string;
  unit_id: string | null;
  asset_kind: string;
  locator: MediaAssetLocator;
  metadata: Record<string, unknown>;
  completeness: string;
}

export interface MediaUnitPage {
  items: MediaUnit[];
  offset: number;
  limit: number;
  has_more: boolean;
  parent_found: boolean;
}

export interface MediaAssetPage {
  items: MediaAsset[];
  offset: number;
  limit: number;
  has_more: boolean;
  parent_found: boolean;
}

export function getMediaItem(resourceId: string): Promise<MediaItem | null> {
  return invoke<MediaItem | null>('get_media_item', {
    request: { resource_id: resourceId },
  });
}

export function getMediaItems(resourceIds: string[]): Promise<MediaItem[]> {
  return invoke<MediaItem[]>('get_media_items', {
    request: { resource_ids: resourceIds },
  });
}

export function listMediaUnits(itemId: string, offset = 0, limit?: number): Promise<MediaUnitPage> {
  return invoke<MediaUnitPage>('list_media_units', {
    request: {
      item_id: itemId,
      offset,
      limit: limit ?? null,
    },
  });
}

export function listMediaAssets(
  unitId: string,
  offset = 0,
  limit?: number,
): Promise<MediaAssetPage> {
  return invoke<MediaAssetPage>('list_media_assets', {
    request: {
      unit_id: unitId,
      offset,
      limit: limit ?? null,
    },
  });
}
