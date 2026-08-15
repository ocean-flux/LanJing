export type DeepLinkIntent =
  | { kind: 'item'; resourceId: string }
  | { kind: 'source'; sourceId: string }
  | { kind: 'install'; src: string }
  | {
      kind: 'reject';
      reason: 'invalid-url' | 'unsupported-scheme' | 'unsupported-import' | 'missing-src';
      subject?: string;
    };

const supportedSchemes = new Set(['legado', 'yuedu', 'lanjing']);
const importSchemes = new Set(['legado', 'yuedu']);

function segments(url: URL) {
  return url.pathname
    .split('/')
    .map((segment) => {
      try {
        return decodeURIComponent(segment).trim();
      } catch {
        return '';
      }
    })
    .filter(Boolean);
}

function reject(
  reason: Extract<DeepLinkIntent, { kind: 'reject' }>['reason'],
  subject?: string,
): DeepLinkIntent {
  return subject ? { kind: 'reject', reason, subject } : { kind: 'reject', reason };
}

export function parseDeepLink(raw: string): DeepLinkIntent {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return reject('invalid-url');
  }

  const scheme = url.protocol.slice(0, -1).toLowerCase();
  if (!supportedSchemes.has(scheme)) {
    return reject('unsupported-scheme', scheme || 'unknown');
  }
  if (url.username || url.password) {
    return reject('invalid-url');
  }

  if (importSchemes.has(scheme)) {
    const path = segments(url);
    const importPath =
      url.hostname.toLowerCase() === 'import'
        ? path[0]
        : url.hostname.toLowerCase() === 'booksource' && path[0]?.toLowerCase() === 'importonline'
          ? 'bookSource'
          : !url.hostname && path[0]?.toLowerCase() === 'import'
            ? path[1]
            : null;
    if (importPath?.toLowerCase() !== 'booksource') {
      return reject('unsupported-import', importPath ?? 'unknown');
    }
    const src = url.searchParams.get('src')?.trim();
    if (!src) {
      return reject('missing-src');
    }
    try {
      const source = new URL(src);
      if (!['http:', 'https:'].includes(source.protocol) || source.username || source.password) {
        return reject('missing-src');
      }
    } catch {
      return reject('missing-src');
    }
    return { kind: 'install', src };
  }

  const path = segments(url);
  if (url.hostname.toLowerCase() === 'item' && path.length > 0) {
    return { kind: 'item', resourceId: path.join('/') };
  }
  if (url.hostname.toLowerCase() === 'source' && path.length > 0) {
    return { kind: 'source', sourceId: path.join('/') };
  }
  return reject('invalid-url', url.hostname || 'unknown');
}

export function parseDeepLinks(rawUrls: readonly string[]): DeepLinkIntent[] {
  return rawUrls.filter((raw) => raw.trim().length > 0).map(parseDeepLink);
}
