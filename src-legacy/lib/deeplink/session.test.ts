import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  completeDeepLinkPick,
  configureDeepLinkSession,
  dismissDeepLinkSurface,
  enqueueDeepLinkIntents,
  getDeepLinkHighlightSourceId,
  getDeepLinkSurface,
  resetDeepLinkSessionForTests,
} from './session.svelte';
import type { DeepLinkIntent } from './parse';

const fetchImportSrc = vi.fn();
const navigate = vi.fn();

beforeEach(() => {
  resetDeepLinkSessionForTests();
  fetchImportSrc.mockReset();
  navigate.mockReset().mockResolvedValue(undefined);
  configureDeepLinkSession({ fetchImportSrc, navigate });
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

describe('deep link session', () => {
  it('opens pick surface after a successful install fetch + catalog parse', async () => {
    fetchImportSrc.mockResolvedValueOnce(
      JSON.stringify([
        { bookSourceName: '甲', bookSourceUrl: 'https://example.test/a', bookSourceGroup: '组' },
        { bookSourceName: '乙', bookSourceUrl: 'https://example.test/b' },
      ]),
    );

    const intent: DeepLinkIntent = {
      kind: 'install',
      scheme: 'legado',
      sourceKind: 'bookSource',
      src: 'https://example.test/sources.json',
    };
    enqueueDeepLinkIntents([intent]);

    await waitFor(() => getDeepLinkSurface().kind === 'pick');
    const surface = getDeepLinkSurface();
    expect(surface.kind).toBe('pick');
    if (surface.kind !== 'pick') return;
    expect(fetchImportSrc).toHaveBeenCalledWith('https://example.test/sources.json');
    expect(surface.catalog.items).toHaveLength(2);
    expect(surface.catalog.items[0]?.group).toBe('组');
  });

  it('surfaces reject intents without network', async () => {
    enqueueDeepLinkIntents([
      {
        kind: 'reject',
        scheme: 'legado',
        reason: 'unsupported-import',
        subject: 'rssSource',
      },
    ]);
    await waitFor(() => getDeepLinkSurface().kind === 'reject');
    expect(getDeepLinkSurface()).toMatchObject({
      kind: 'reject',
      reason: 'unsupported-import',
      subject: 'rssSource',
    });
    expect(fetchImportSrc).not.toHaveBeenCalled();
  });

  it('navigates item and source intents in FIFO order', async () => {
    enqueueDeepLinkIntents([
      { kind: 'item', scheme: 'lanjing', resourceId: 'item:one' },
      { kind: 'source', scheme: 'lanjing', sourceId: 'source:one' },
    ]);

    await waitFor(() => navigate.mock.calls.length >= 2);
    expect(navigate.mock.calls[0]?.[0]).toBe('/library/item/item%3Aone');
    expect(navigate.mock.calls[1]?.[0]).toContain('/sources?highlight=');
    expect(getDeepLinkHighlightSourceId()).toBe('source:one');
    expect(getDeepLinkSurface().kind).toBe('idle');
  });

  it('holds the queue while pick is open and resumes after complete', async () => {
    fetchImportSrc.mockResolvedValueOnce(
      JSON.stringify({ bookSourceName: '仅一', bookSourceUrl: 'https://example.test/x' }),
    );

    enqueueDeepLinkIntents([
      {
        kind: 'install',
        scheme: 'lanjing',
        sourceKind: 'bookSource',
        src: 'https://example.test/a.json',
      },
      { kind: 'item', scheme: 'lanjing', resourceId: 'item:two' },
    ]);

    await waitFor(() => getDeepLinkSurface().kind === 'pick');
    expect(navigate).not.toHaveBeenCalled();

    completeDeepLinkPick();
    await waitFor(() => navigate.mock.calls.length === 1);
    expect(navigate.mock.calls[0]?.[0]).toBe('/library/item/item%3Atwo');
  });

  it('maps fetch failures to a safe error surface', async () => {
    fetchImportSrc.mockRejectedValueOnce(new Error('ssrf blocked'));
    enqueueDeepLinkIntents([
      {
        kind: 'install',
        scheme: 'legado',
        sourceKind: 'bookSource',
        src: 'https://169.254.169.254/',
      },
    ]);
    await waitFor(() => getDeepLinkSurface().kind === 'fetch-error');
    expect(getDeepLinkSurface()).toMatchObject({
      kind: 'fetch-error',
      message: 'ssrf blocked',
    });
    dismissDeepLinkSurface();
    expect(getDeepLinkSurface().kind).toBe('idle');
  });

  it('rejects non-bookSource catalog payloads without exposing body', async () => {
    fetchImportSrc.mockResolvedValueOnce(JSON.stringify({ rssSourceName: 'x' }));
    enqueueDeepLinkIntents([
      {
        kind: 'install',
        scheme: 'legado',
        sourceKind: 'bookSource',
        src: 'https://example.test/rss.json',
      },
    ]);
    await waitFor(() => getDeepLinkSurface().kind === 'catalog-error');
    expect(getDeepLinkSurface()).toEqual({ kind: 'catalog-error', reason: 'not-book-source' });
  });
});
