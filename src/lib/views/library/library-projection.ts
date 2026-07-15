import type {
  MediaGraphDelta,
  MediaItem,
  MediaRelation,
  SourceProfile,
} from '$lib/stores/execution.svelte';

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
}

export interface LibraryProjectionResponse {
  graph: MediaGraphDelta;
  entries: LibraryEntry[];
}

export interface LibraryProjectionItem {
  item: MediaItem;
  source: SourceProfile | null;
  state: LibraryEntry;
  relations: MediaRelation[];
  alternativeRoutes: MediaItem[];
}

function ownsResource(entry: LibraryEntry): boolean {
  return entry.favorite || entry.pinned || entry.last_opened_at !== null || entry.progress !== null;
}

function latestEntries(entries: LibraryEntry[]): Map<string, LibraryEntry> {
  return new Map(entries.map((entry) => [entry.resource_id, entry]));
}

function latestItems(items: MediaItem[]): Map<string, MediaItem> {
  return new Map(items.map((item) => [item.id, item]));
}

function sortItems(left: LibraryProjectionItem, right: LibraryProjectionItem): number {
  if (left.state.pinned !== right.state.pinned) return left.state.pinned ? -1 : 1;
  if (left.state.last_opened_at !== right.state.last_opened_at) {
    return (right.state.last_opened_at ?? '').localeCompare(left.state.last_opened_at ?? '');
  }
  return left.item.title.localeCompare(right.item.title);
}

export function projectLibrary(
  projection: LibraryProjectionResponse | null | undefined,
  mediaKind?: string,
): LibraryProjectionItem[] {
  if (!projection) return [];

  const entries = latestEntries(projection.entries);
  const items = latestItems(projection.graph.items ?? []);
  const sources = new Map((projection.graph.sources ?? []).map((source) => [source.id, source]));

  return [...entries.values()]
    .filter(ownsResource)
    .map((state) => {
      const item = items.get(state.resource_id);
      if (!item || (mediaKind && item.media_kind !== mediaKind)) return null;
      const relations = (projection.graph.relations ?? []).filter(
        (relation) => relation.from_id === item.id,
      );
      const alternativeRoutes = relations
        .filter((relation) => relation.relation_kind === 'source_origin')
        .map((relation) => items.get(relation.to_id))
        .filter((related): related is MediaItem => related !== undefined);
      return {
        item,
        source: sources.get(item.source_id) ?? null,
        state,
        relations,
        alternativeRoutes,
      };
    })
    .filter((item): item is LibraryProjectionItem => item !== null)
    .sort(sortItems);
}
