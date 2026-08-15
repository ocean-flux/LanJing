export const CATALOG_DISPLAY_CAP = 200;
export const CATALOG_INSTALL_CAP = 50;

export interface CatalogItem {
  id: string;
  name: string;
  group: string | null;
  rawJson: string;
}

export type CatalogResult =
  | { ok: true; items: CatalogItem[]; totalCount: number; truncated: boolean }
  | { ok: false; reason: 'invalid-json' | 'not-book-source' | 'empty' };

function readBookSourceName(value: unknown): string | null {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return null;
  const name = (value as Record<string, unknown>).bookSourceName;
  return typeof name === 'string' && name.trim().length > 0 ? name.trim() : null;
}

export function parseBookSourceCatalog(rawBody: string): CatalogResult {
  let parsed: unknown;
  try {
    parsed = JSON.parse(rawBody);
  } catch {
    return { ok: false, reason: 'invalid-json' };
  }

  const candidates = Array.isArray(parsed) ? parsed : [parsed];
  if (candidates.length === 0) return { ok: false, reason: 'empty' };

  const items: CatalogItem[] = [];
  for (const [index, candidate] of candidates.entries()) {
    const name = readBookSourceName(candidate);
    if (name && typeof candidate === 'object' && candidate !== null && !Array.isArray(candidate)) {
      const group = (candidate as Record<string, unknown>).bookSourceGroup;
      items.push({
        id: `catalog:${index}`,
        name,
        group: typeof group === 'string' && group.trim().length > 0 ? group.trim() : null,
        rawJson: JSON.stringify(candidate),
      });
    }
  }

  if (items.length === 0) return { ok: false, reason: 'not-book-source' };
  return {
    ok: true,
    items: items.slice(0, CATALOG_DISPLAY_CAP),
    totalCount: items.length,
    truncated: items.length > CATALOG_DISPLAY_CAP,
  };
}
