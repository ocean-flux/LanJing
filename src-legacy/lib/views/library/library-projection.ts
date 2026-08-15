export interface LibraryProgress {
  unit_id: string | null;
  position: number;
  total: number | null;
}

/** RuleSystem library projection entry；仅包含用户状态与并发版本。 */
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

export interface LibraryProjectionItem {
  resource_id: string;
  state: LibraryEntry;
}

function sortItems(left: LibraryProjectionItem, right: LibraryProjectionItem): number {
  if (left.state.pinned !== right.state.pinned) return left.state.pinned ? -1 : 1;
  if (left.state.last_opened_at !== right.state.last_opened_at) {
    return (right.state.last_opened_at ?? '').localeCompare(left.state.last_opened_at ?? '');
  }
  return left.resource_id.localeCompare(right.resource_id);
}

/** 资料库视图只投影 RuleSystem 的安全 library entry，不重新读取媒体图。 */
export function projectLibrary(
  projection: LibraryProjectionResponse | null | undefined,
): LibraryProjectionItem[] {
  if (!projection) return [];

  return projection.entries
    .filter(
      (entry) =>
        entry.favorite || entry.pinned || entry.last_opened_at !== null || entry.progress !== null,
    )
    .map((state) => ({ resource_id: state.resource_id, state }))
    .sort(sortItems);
}
