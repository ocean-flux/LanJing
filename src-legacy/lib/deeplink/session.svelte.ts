/**
 * 深链会话态：意图 FIFO 队列 + 可绑定 UI 表面。
 *
 * 不直接依赖 Tauri；由 runtime 注入 getCurrent / onOpenUrl / fetch / goto。
 */

import type { DeepLinkIntent } from './parse';
import { parseBookSourceCatalog, type CatalogError, type CatalogOk } from './catalog';
import { fetchImportSrc } from './import-api';

export type DeepLinkSurface =
  | { kind: 'idle' }
  | {
      kind: 'reject';
      reason: Extract<DeepLinkIntent, { kind: 'reject' }>['reason'];
      subject?: string;
    }
  | { kind: 'loading'; src: string }
  | { kind: 'fetch-error'; message: string }
  | { kind: 'catalog-error'; reason: CatalogError['reason'] }
  | { kind: 'pick'; catalog: CatalogOk; src: string };

export type DeepLinkNavigate = (path: string) => void | Promise<void>;
export type DeepLinkFetchSrc = (url: string) => Promise<string>;

export type DeepLinkSessionDeps = {
  fetchImportSrc: DeepLinkFetchSrc;
  navigate: DeepLinkNavigate;
};

const defaultDeps: DeepLinkSessionDeps = {
  fetchImportSrc,
  navigate: async () => {
    /* 浏览器 / 测试默认 no-op；runtime 注入 SvelteKit goto */
  },
};

let deps: DeepLinkSessionDeps = { ...defaultDeps };
let queue: DeepLinkIntent[] = [];
let processing = false;
let surface = $state<DeepLinkSurface>({ kind: 'idle' });
/** 来源页可选高亮的 source_id。 */
let highlightSourceId = $state<string | null>(null);

export function getDeepLinkSurface(): DeepLinkSurface {
  return surface;
}

export function getDeepLinkHighlightSourceId(): string | null {
  return highlightSourceId;
}

export function clearDeepLinkHighlightSourceId(): void {
  highlightSourceId = null;
}

/** 测试或宿主注入依赖（fetch / 导航）。 */
export function configureDeepLinkSession(partial: Partial<DeepLinkSessionDeps>): void {
  deps = { ...deps, ...partial };
}

/** 重置会话（仅测试）。 */
export function resetDeepLinkSessionForTests(): void {
  queue = [];
  processing = false;
  surface = { kind: 'idle' };
  highlightSourceId = null;
  deps = { ...defaultDeps };
}

export function enqueueDeepLinkIntents(intents: readonly DeepLinkIntent[]): void {
  if (intents.length === 0) return;
  queue.push(...intents);
  void pumpQueue();
}

/** 用户关闭当前表面后继续处理队列。 */
export function dismissDeepLinkSurface(): void {
  surface = { kind: 'idle' };
  void pumpQueue();
}

/** 安装成功或取消后调用，释放 pick 表面并处理下一条。 */
export function completeDeepLinkPick(): void {
  if (surface.kind === 'pick' || surface.kind === 'loading') {
    surface = { kind: 'idle' };
  }
  void pumpQueue();
}

function isBlockingSurface(current: DeepLinkSurface): boolean {
  return current.kind !== 'idle';
}

async function pumpQueue(): Promise<void> {
  if (processing) return;
  processing = true;
  try {
    while (queue.length > 0) {
      if (isBlockingSurface(surface)) return;
      const next = queue.shift();
      if (!next) return;
      await handleIntent(next);
    }
  } finally {
    processing = false;
    if (queue.length > 0 && !isBlockingSurface(surface)) {
      void pumpQueue();
    }
  }
}

async function handleIntent(intent: DeepLinkIntent): Promise<void> {
  switch (intent.kind) {
    case 'reject':
      surface = {
        kind: 'reject',
        reason: intent.reason,
        ...(intent.subject ? { subject: intent.subject } : {}),
      };
      return;
    case 'item': {
      const path = `/library/item/${encodeURIComponent(intent.resourceId)}`;
      await deps.navigate(path);
      return;
    }
    case 'source': {
      highlightSourceId = intent.sourceId;
      await deps.navigate(`/sources?highlight=${encodeURIComponent(intent.sourceId)}`);
      return;
    }
    case 'install':
      await handleInstall(intent.src);
      return;
  }
}

async function handleInstall(src: string): Promise<void> {
  surface = { kind: 'loading', src };
  try {
    const body = await deps.fetchImportSrc(src);
    const catalog = parseBookSourceCatalog(body);
    if (!catalog.ok) {
      surface = { kind: 'catalog-error', reason: catalog.reason };
      return;
    }
    surface = { kind: 'pick', catalog, src };
  } catch (caught) {
    // 不把 Plan/secret/原始 body 泄漏到 UI；仅字符串化错误摘要。
    const message = caught instanceof Error ? caught.message : String(caught);
    surface = { kind: 'fetch-error', message: message || 'fetch failed' };
  }
}
