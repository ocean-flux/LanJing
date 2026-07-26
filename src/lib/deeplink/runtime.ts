/**
 * 深链运行时：冷启动 getCurrent + 热启动 onOpenUrl。
 * 浏览器 / vitest 无 Tauri 时安全 no-op。
 */

import { browser } from '$app/environment';
import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { parseDeepLinks } from './parse';
import {
  configureDeepLinkSession,
  enqueueDeepLinkIntents,
  type DeepLinkFetchSrc,
  type DeepLinkNavigate,
} from './session.svelte';
import { fetchImportSrc } from './import-api';

export type DeepLinkPlugin = {
  getCurrent: () => Promise<string[] | null>;
  onOpenUrl: (handler: (urls: string[]) => void) => Promise<() => void> | (() => void);
};

export type StartDeepLinkRuntimeOptions = {
  plugin?: DeepLinkPlugin | null;
  fetchImportSrc?: DeepLinkFetchSrc;
  navigate?: DeepLinkNavigate;
  /** 强制启动（测试）；默认仅 browser。 */
  force?: boolean;
};

function defaultNavigate(path: string): Promise<void> {
  // SvelteKit：带 query 的路径用 resolve 处理 base；库条目需 encode。
  if (path.startsWith('/library/item/')) {
    const encoded = path.slice('/library/item/'.length);
    return goto(resolve(`/library/item/${encoded}` as '/'));
  }
  if (path.startsWith('/sources')) {
    // resolve 需覆盖完整 path+query，否则 eslint no-navigation-without-resolve 会误报拼接。
    return goto(resolve(path as '/'));
  }
  return goto(resolve(path as '/'));
}

async function loadPlugin(): Promise<DeepLinkPlugin | null> {
  try {
    const mod = await import('@tauri-apps/plugin-deep-link');
    return {
      getCurrent: () => mod.getCurrent(),
      onOpenUrl: (handler) => mod.onOpenUrl(handler),
    };
  } catch {
    return null;
  }
}

/** 启动深链监听；返回取消函数。 */
export async function startDeepLinkRuntime(
  options: StartDeepLinkRuntimeOptions = {},
): Promise<() => void> {
  if (!options.force && !browser) {
    return () => undefined;
  }

  configureDeepLinkSession({
    fetchImportSrc: options.fetchImportSrc ?? fetchImportSrc,
    navigate: options.navigate ?? defaultNavigate,
  });

  const plugin = options.plugin === undefined ? await loadPlugin() : options.plugin;
  if (!plugin) {
    return () => undefined;
  }

  const deliver = (urls: string[] | null | undefined) => {
    if (!urls || urls.length === 0) return;
    enqueueDeepLinkIntents(parseDeepLinks(urls));
  };

  try {
    deliver(await plugin.getCurrent());
  } catch {
    // 冷启动读取失败不阻断热启动订阅
  }

  let unlisten: (() => void) | undefined;
  try {
    const stop = await plugin.onOpenUrl((urls) => {
      deliver(urls);
    });
    unlisten = typeof stop === 'function' ? stop : undefined;
  } catch {
    unlisten = undefined;
  }

  return () => {
    unlisten?.();
  };
}
