import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import LibraryHome from './LibraryHome.svelte';
import type { LibraryEntry, LibraryProjectionResponse } from './library-projection';

const emptyProjection: LibraryProjectionResponse = {
  global_seq: 0,
  entries: [],
};

const readyProjection: LibraryProjectionResponse = {
  global_seq: 8,
  entries: [
    {
      resource_id: 'item:one',
      favorite: true,
      pinned: true,
      last_opened_at: null,
      progress: null,
      revision: 3,
      updated_global_seq: 8,
    },
  ],
};

describe('LibraryHome', () => {
  it('shows loading on the first frame when projection is absent', () => {
    render(LibraryHome, {
      props: { load: () => Promise.withResolvers<LibraryProjectionResponse>().promise },
    });

    expect(screen.getByRole('status').textContent).toContain('正在加载资料库');
    expect(screen.queryByTestId('library-empty')).toBeNull();
  });

  it('shows one source next step only after a successful empty projection', () => {
    render(LibraryHome, { props: { projection: emptyProjection } });

    expect(screen.getByRole('heading', { level: 1, name: '资料库' })).toBeTruthy();
    expect(screen.getByRole('status').textContent).toContain('资料库还没有资源');
    const links = screen.getAllByRole('link');
    expect(links).toHaveLength(1);
    expect(screen.getByRole('link', { name: '管理来源' }).getAttribute('href')).toBe('/sources');
  });

  it('keeps load errors separate from empty state and retries', async () => {
    const load = vi
      .fn<() => Promise<LibraryProjectionResponse>>()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce(emptyProjection);
    render(LibraryHome, { props: { load } });

    expect((await screen.findByRole('alert')).textContent).toContain('资料库加载失败');
    expect(screen.queryByTestId('library-empty')).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: '重试加载' }));
    expect(await screen.findByTestId('library-empty')).toBeTruthy();
    expect(load).toHaveBeenCalledTimes(2);
  });

  it('serializes writes per resource and advances the successful revision', async () => {
    const { promise: firstWrite, resolve: resolveFirst } = Promise.withResolvers<{
      global_seq: number;
      revision: number;
    }>();
    const update = vi
      .fn<(entry: LibraryEntry) => Promise<{ global_seq: number; revision: number }>>()
      .mockReturnValueOnce(firstWrite)
      .mockResolvedValueOnce({ global_seq: 10, revision: 5 });

    render(LibraryHome, { props: { projection: readyProjection, update } });

    const favorite = screen.getByRole('button', { name: '取消收藏' });
    const pin = screen.getByRole('button', { name: '取消固定' });
    expect(screen.getByText('修订 3')).toBeTruthy();
    await fireEvent.click(favorite);
    await fireEvent.click(pin);

    expect(update).toHaveBeenCalledTimes(1);
    expect((favorite as HTMLButtonElement).disabled).toBe(true);
    expect((pin as HTMLButtonElement).disabled).toBe(true);
    expect(favorite.getAttribute('aria-busy')).toBe('true');

    resolveFirst?.({ global_seq: 9, revision: 4 });
    await waitFor(() => expect(screen.getByText('修订 4')).toBeTruthy());
    await fireEvent.click(screen.getByRole('button', { name: '取消固定' }));

    await waitFor(() => expect(update).toHaveBeenCalledTimes(2));
    expect(update.mock.calls[1]?.[0]).toMatchObject({
      resource_id: 'item:one',
      favorite: false,
      pinned: false,
      revision: 4,
    });
  });

  it('reports update errors and clears them on a successful retry', async () => {
    const update = vi
      .fn<(entry: LibraryEntry) => Promise<{ global_seq: number; revision: number }>>()
      .mockRejectedValueOnce(new Error('conflict'))
      .mockResolvedValueOnce({ global_seq: 9, revision: 4 });
    render(LibraryHome, { props: { projection: readyProjection, update } });

    await fireEvent.click(screen.getByRole('button', { name: '取消收藏' }));
    expect((await screen.findByRole('alert')).textContent).toContain('资料库状态更新失败');

    await fireEvent.click(screen.getByRole('button', { name: '取消收藏' }));
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
    expect(await screen.findByRole('button', { name: '收藏' })).toBeTruthy();
    expect(update).toHaveBeenCalledTimes(2);
  });
});
