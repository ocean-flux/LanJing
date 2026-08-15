/**
 * 解析 legado/yuedu/lanjing 深链，只产出前端可消费的安全意图。
 *
 * 对齐 `.tmp/legado` OnLineImportActivity：
 * - `legado|yuedu://import/{path}?src=`（path 大小写不敏感）
 * - 旧式 `legado|yuedu://booksource/importonline?src=`
 * - 三斜线 `legado:///import/bookSource?src=`
 *
 * 解析阶段不访问网络；`src` 的协议、SSRF 与响应大小由 Tauri command 再次校验。
 */

export type DeepLinkScheme = 'legado' | 'yuedu' | 'lanjing';

export type DeepLinkIntent =
  | {
      kind: 'install';
      scheme: DeepLinkScheme;
      sourceKind: 'bookSource';
      src: string;
    }
  | {
      kind: 'item';
      scheme: 'lanjing';
      resourceId: string;
    }
  | {
      kind: 'source';
      scheme: 'lanjing';
      sourceId: string;
    }
  | {
      kind: 'reject';
      scheme: DeepLinkScheme | null;
      reason: 'invalid-url' | 'unsupported-scheme' | 'unsupported-import' | 'missing-src';
      subject?: string;
    };

const LEGADO_FAMILY: Record<'legado' | 'yuedu', true> = { legado: true, yuedu: true };
const SUPPORTED_SCHEMES: Record<DeepLinkScheme, true> = {
  legado: true,
  yuedu: true,
  lanjing: true,
};
const HTTP_SCHEMES: Record<'http:' | 'https:', true> = { 'http:': true, 'https:': true };

function decodeSegment(value: string): string | null {
  try {
    const decoded = decodeURIComponent(value).trim();
    return decoded.length > 0 ? decoded : null;
  } catch {
    return null;
  }
}

function reject(
  scheme: DeepLinkScheme | null,
  reason: Extract<DeepLinkIntent, { kind: 'reject' }>['reason'],
  subject?: string,
): DeepLinkIntent {
  return subject ? { kind: 'reject', scheme, reason, subject } : { kind: 'reject', scheme, reason };
}

function pathSegments(url: URL): string[] {
  return url.pathname
    .split('/')
    .map((segment) => decodeSegment(segment))
    .filter((segment): segment is string => segment !== null);
}

/**
 * 从 Legado/Yuedu URI 取出 import path 类型（保留原始大小写供 i18n subject）。
 * 无法识别为 import 形状时返回 null。
 */
export function extractLegadoImportPath(url: URL): string | null {
  const host = url.hostname.trim().toLowerCase();
  const segments = pathSegments(url);
  const lower = segments.map((s) => s.toLowerCase());

  // legado://import/bookSource  → host=import, path=/bookSource
  if (host === 'import') {
    return segments[0] ?? null;
  }

  // legado:///import/bookSource → host 空, path=/import/bookSource
  if (!host && lower[0] === 'import') {
    return segments[1] ?? null;
  }

  // 旧阅读：legado://booksource/importonline?src=
  if (host === 'booksource' && lower[0] === 'importonline') {
    return 'bookSource';
  }

  return null;
}

function isBookSourcePath(pathKind: string): boolean {
  return pathKind.trim().toLowerCase() === 'booksource';
}

function parseInstall(url: URL, scheme: DeepLinkScheme, sourceKind: string | null): DeepLinkIntent {
  if (!sourceKind) {
    return reject(scheme, 'unsupported-import', 'unknown');
  }
  if (!isBookSourcePath(sourceKind)) {
    return reject(scheme, 'unsupported-import', sourceKind);
  }

  const src = url.searchParams.get('src')?.trim() ?? '';
  if (!src) return reject(scheme, 'missing-src');

  let srcUrl: URL;
  try {
    srcUrl = new URL(src);
  } catch {
    return reject(scheme, 'missing-src');
  }
  if (!(srcUrl.protocol in HTTP_SCHEMES) || srcUrl.username || srcUrl.password) {
    return reject(scheme, 'missing-src');
  }

  return { kind: 'install', scheme, sourceKind: 'bookSource', src };
}

/** 解析单个 OS/浏览器传入的深链。 */
export function parseDeepLink(raw: string): DeepLinkIntent {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return reject(null, 'invalid-url');
  }

  const schemeValue = url.protocol.slice(0, -1).toLowerCase();
  if (!(schemeValue in SUPPORTED_SCHEMES)) {
    return reject(null, 'unsupported-scheme', schemeValue || 'unknown');
  }
  const scheme = schemeValue as DeepLinkScheme;
  if (url.username || url.password) {
    return reject(scheme, 'invalid-url');
  }

  // Legado 家族：legado + yuedu（阅读旧 scheme）
  if (scheme in LEGADO_FAMILY) {
    const importPath = extractLegadoImportPath(url);
    if (importPath === null) {
      return reject(scheme, 'invalid-url', url.hostname || url.pathname || 'unknown');
    }
    return parseInstall(url, scheme, importPath);
  }

  // lanjing 自有 scheme
  const host = url.hostname.trim().toLowerCase();
  const segments = pathSegments(url);

  if (host === 'install') {
    return parseInstall(url, scheme, segments[0] ?? url.searchParams.get('kind') ?? 'bookSource');
  }
  if (host === 'item') {
    const resourceId = segments.join('/');
    return resourceId
      ? { kind: 'item', scheme: 'lanjing', resourceId }
      : reject(scheme, 'invalid-url', 'item');
  }
  if (host === 'source') {
    const sourceId = segments.join('/');
    return sourceId
      ? { kind: 'source', scheme: 'lanjing', sourceId }
      : reject(scheme, 'invalid-url', 'source');
  }

  return reject(scheme, 'invalid-url', host || 'unknown');
}

/** 保持 OS 一次传入多个 URL 时的顺序，并丢弃空字符串。 */
export function parseDeepLinks(rawUrls: readonly string[]): DeepLinkIntent[] {
  return rawUrls.filter((raw) => raw.trim().length > 0).map(parseDeepLink);
}
