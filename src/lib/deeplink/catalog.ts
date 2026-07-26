/**
 * 深链拉取的书源 JSON → 可勾选目录（纯函数，无网络 / 无 IPC）。
 *
 * 硬顶：展示 ≤ {@link CATALOG_DISPLAY_CAP}；安装勾选上限由 UI 用
 * {@link CATALOG_INSTALL_CAP} 执行。解析失败不回传原始 body。
 */

/** 预览列表硬顶。 */
export const CATALOG_DISPLAY_CAP = 200;

/** 单次确认安装上限。 */
export const CATALOG_INSTALL_CAP = 50;

export type CatalogItem = {
  /** 本地稳定 id（会话内唯一，非服务端 id）。 */
  id: string;
  name: string;
  group: string | null;
  /** 单条 Legado 对象 JSON，供 prepareInstall 使用。 */
  rawJson: string;
};

export type CatalogOk = {
  ok: true;
  items: CatalogItem[];
  /** 合法条目总数（截断前）。 */
  totalCount: number;
  /** 是否因展示硬顶被截断。 */
  truncated: boolean;
};

export type CatalogError = {
  ok: false;
  reason: 'invalid-json' | 'not-book-source' | 'empty';
};

export type CatalogResult = CatalogOk | CatalogError;

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** 判定是否为可装书源形状：至少有非空 bookSourceName。 */
export function isBookSourceShape(value: unknown): value is Record<string, unknown> {
  if (!isPlainObject(value)) return false;
  const name = value.bookSourceName;
  return typeof name === 'string' && name.trim().length > 0;
}

function readGroup(value: Record<string, unknown>): string | null {
  const group = value.bookSourceGroup;
  if (typeof group !== 'string') return null;
  const trimmed = group.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function toItem(value: Record<string, unknown>, index: number): CatalogItem {
  const name = String(value.bookSourceName).trim();
  return {
    id: `catalog:${index}`,
    name,
    group: readGroup(value),
    rawJson: JSON.stringify(value),
  };
}

/**
 * 解析深链 src 正文：支持单对象或数组。
 * 非法条目跳过；全部非法则 `not-book-source`；空数组/无合法项为 `empty`。
 */
export function parseBookSourceCatalog(rawBody: string): CatalogResult {
  let parsed: unknown;
  try {
    parsed = JSON.parse(rawBody);
  } catch {
    return { ok: false, reason: 'invalid-json' };
  }

  const candidates: unknown[] = Array.isArray(parsed) ? parsed : [parsed];
  if (candidates.length === 0) {
    return { ok: false, reason: 'empty' };
  }

  const valid: CatalogItem[] = [];
  for (let index = 0; index < candidates.length; index += 1) {
    const entry = candidates[index];
    if (!isBookSourceShape(entry)) continue;
    valid.push(toItem(entry, index));
  }

  if (valid.length === 0) {
    // 有内容但无一合法：形状错误；空数组已在上面返回 empty。
    return candidates.some((entry) => entry !== null && entry !== undefined)
      ? { ok: false, reason: 'not-book-source' }
      : { ok: false, reason: 'empty' };
  }

  const truncated = valid.length > CATALOG_DISPLAY_CAP;
  return {
    ok: true,
    items: truncated ? valid.slice(0, CATALOG_DISPLAY_CAP) : valid,
    totalCount: valid.length,
    truncated,
  };
}

export type CatalogGroup = {
  id: string;
  label: string | null;
  items: CatalogItem[];
};

/** 按 bookSourceGroup 分组；未分组置于末尾。 */
export function groupCatalogItems(items: readonly CatalogItem[]): CatalogGroup[] {
  const groups = new Map<string, CatalogGroup>();
  const ungrouped: CatalogItem[] = [];

  for (const item of items) {
    if (!item.group) {
      ungrouped.push(item);
      continue;
    }
    const id = `group:${item.group}`;
    const existing = groups.get(id);
    if (existing) {
      existing.items.push(item);
    } else {
      groups.set(id, { id, label: item.group, items: [item] });
    }
  }

  const ordered = [...groups.values()].sort((a, b) =>
    (a.label ?? '').localeCompare(b.label ?? '', undefined, { sensitivity: 'base' }),
  );
  if (ungrouped.length > 0) {
    ordered.push({ id: 'group:__ungrouped__', label: null, items: ungrouped });
  }
  return ordered;
}
