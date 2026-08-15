import { isTauri } from '@tauri-apps/api/core';
import { parseDeepLinks, type DeepLinkIntent } from './deep-link-parse';

export type DeepLinkHandler = (intent: DeepLinkIntent) => void;
export interface DeepLinkPlugin {
  getCurrent: () => Promise<string[] | null>;
  onOpenUrl: (handler: (urls: string[]) => void) => Promise<() => void> | (() => void);
}

const NOOP = () => undefined;

async function loadPlugin(): Promise<DeepLinkPlugin | null> {
  try {
    const module = await import('@tauri-apps/plugin-deep-link');
    return {
      getCurrent: module.getCurrent,
      onOpenUrl: module.onOpenUrl,
    };
  } catch {
    return null;
  }
}

export async function startDeepLinkRuntime(
  handler: DeepLinkHandler,
  plugin?: DeepLinkPlugin | null,
): Promise<() => void> {
  if (!isTauri()) return NOOP;
  const activePlugin = plugin === undefined ? await loadPlugin() : plugin;
  if (!activePlugin) return NOOP;

  const deliver = (urls: string[] | null | undefined) => {
    for (const intent of parseDeepLinks(urls ?? [])) handler(intent);
  };

  let unlisten: () => void = NOOP;
  try {
    const stop = await activePlugin.onOpenUrl(deliver);
    if (typeof stop === 'function') unlisten = stop;
  } catch {
    // 热启动订阅失败时仍尝试消费冷启动链接。
  }

  try {
    deliver(await activePlugin.getCurrent());
  } catch {
    // 冷启动读取失败不应撤销已建立的热启动订阅。
  }

  return unlisten;
}
