import { beforeEach, describe, expect, it, vi } from 'vitest';
import { startDeepLinkRuntime } from './runtime';
import { getDeepLinkSurface, resetDeepLinkSessionForTests } from './session.svelte';

vi.mock('$app/environment', () => ({ browser: true }));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/paths', () => ({
  resolve: (path: string) => path,
}));

beforeEach(() => {
  resetDeepLinkSessionForTests();
});

function waitFor(predicate: () => boolean, timeoutMs = 500): Promise<void> {
  const started = Date.now();
  return new Promise((resolve, reject) => {
    const tick = () => {
      if (predicate()) {
        resolve();
        return;
      }
      if (Date.now() - started > timeoutMs) {
        reject(new Error('waitFor timeout'));
        return;
      }
      queueMicrotask(tick);
    };
    tick();
  });
}

describe('startDeepLinkRuntime', () => {
  it('drains cold-start URLs and listens for hot opens', async () => {
    const handlers: Array<(urls: string[]) => void> = [];
    const unlisten = vi.fn();
    const plugin = {
      getCurrent: vi
        .fn()
        .mockResolvedValue(['legado://import/rssSource?src=https%3A%2F%2Fexample.test%2Frss.json']),
      onOpenUrl: vi.fn(async (handler: (urls: string[]) => void) => {
        handlers.push(handler);
        return unlisten;
      }),
    };

    const stop = await startDeepLinkRuntime({
      force: true,
      plugin,
      fetchImportSrc: vi.fn(),
      navigate: vi.fn(),
    });

    await waitFor(() => getDeepLinkSurface().kind === 'reject');
    expect(getDeepLinkSurface()).toMatchObject({
      kind: 'reject',
      reason: 'unsupported-import',
      subject: 'rssSource',
    });

    // 关闭拒绝面后热启动 item
    const { dismissDeepLinkSurface } = await import('./session.svelte');
    dismissDeepLinkSurface();

    handlers[0]?.(['lanjing://item/item%3Ahot']);
    // item 不阻塞 surface
    await waitFor(() => true);
    stop();
    expect(unlisten).toHaveBeenCalled();
  });

  it('is safe when the deep-link plugin is unavailable', async () => {
    const stop = await startDeepLinkRuntime({
      force: true,
      plugin: null,
    });
    expect(typeof stop).toBe('function');
    stop();
    expect(getDeepLinkSurface().kind).toBe('idle');
  });
});
