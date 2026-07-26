/**
 * 资料库列表媒体 enrichment：投影 entry 与标准 MediaItem 按 resource_id 合并。
 * 批量上限对齐后端 get_media_items（1..=64）。
 */

import { getMediaItems, type MediaItem } from '$lib/views/media/media-api';
import type { LibraryEntry, LibraryProjectionItem } from './library-projection';

/** 与 RuleSystem `MAX_MEDIA_BATCH_IDS` 对齐。 */
export const MEDIA_BATCH_SIZE = 64;

export type LoadMediaItems = (resourceIds: string[]) => Promise<MediaItem[]>;

/** denselist 行 DTO：媒体存在时用标准 title/kind；缺失时诚实降级。 */
export interface LibraryEntryRowModel {
  resource_id: string;
  state: LibraryEntry;
  media: MediaItem | null;
  /** 有媒体时为标准 title；否则为 resource_id（配合降级文案）。 */
  title: string;
  kind: string | null;
  cover_asset_id: string | null;
  mediaMissing: boolean;
}

/** 按 1..=64 分批拉取，缺项由后端省略；整批失败由调用方决定降级。 */
export async function loadMediaByResourceIds(
  resourceIds: string[],
  fetchItems: LoadMediaItems = getMediaItems,
): Promise<Map<string, MediaItem>> {
  const map = new Map<string, MediaItem>();
  if (resourceIds.length === 0) return map;

  for (let offset = 0; offset < resourceIds.length; offset += MEDIA_BATCH_SIZE) {
    const chunk = resourceIds.slice(offset, offset + MEDIA_BATCH_SIZE);
    if (chunk.length === 0) continue;
    const items = await fetchItems(chunk);
    for (const item of items) {
      map.set(item.id, item);
    }
  }

  return map;
}

/** 将投影行与媒体 map 合并；无 item 时不造假数据。 */
export function enrichLibraryItems(
  items: LibraryProjectionItem[],
  mediaById: ReadonlyMap<string, MediaItem>,
): LibraryEntryRowModel[] {
  return items.map((item) => {
    const media = mediaById.get(item.resource_id) ?? null;
    const displayTitle = media?.title?.trim() ?? '';
    const hasDisplayTitle = displayTitle.length > 0;
    return {
      resource_id: item.resource_id,
      state: item.state,
      media,
      title: hasDisplayTitle ? displayTitle : item.resource_id,
      kind: media?.media_kind ?? null,
      cover_asset_id: media?.cover_asset_id ?? null,
      // 空标题同样无法提供可用媒体元数据，沿用缺媒体的诚实降级路径。
      mediaMissing: media === null || !hasDisplayTitle,
    };
  });
}

/** 从媒体数组建 map（测试与同步 prop 注入）。 */
export function mediaItemsToMap(items: readonly MediaItem[]): Map<string, MediaItem> {
  const map = new Map<string, MediaItem>();
  for (const item of items) {
    map.set(item.id, item);
  }
  return map;
}
