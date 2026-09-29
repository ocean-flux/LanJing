import { describe, expect, it } from 'vitest';
import { createLibraryWorkflow, type LibraryWorkflowAdapter } from '@/features/library/workflow';
import type { LibraryEntry, LibraryProjectionResponse } from '@/shared/tauri/library';
import type { MediaItem } from '@/shared/types/media';

function entry(resourceId: string, overrides: Partial<LibraryEntry> = {}): LibraryEntry {
  return {
    resource_id: resourceId,
    favorite: true,
    pinned: false,
    last_opened_at: null,
    progress: null,
    revision: 1,
    updated_global_seq: 1,
    ...overrides,
  };
}

function projection(entries: LibraryEntry[]): LibraryProjectionResponse {
  return { global_seq: 1, entries };
}

function media(id: string): MediaItem {
  return { id, title: `标题 ${id}`, creator: '作者', kind: 'text' };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((_resolve, _reject) => {
    resolve = _resolve;
    reject = _reject;
  });
  return { promise, resolve, reject };
}

describe('library workflow', () => {
  it('批量 enrichment 后保留没有媒体投影的组织条目', async () => {
    const adapter: LibraryWorkflowAdapter = {
      loadProjection: async () => projection([entry('item:known'), entry('item:missing')]),
      loadMediaItems: async (resourceIds) => {
        expect(resourceIds).toEqual(['item:known', 'item:missing']);
        return [media('item:known')];
      },
      updateLibraryEntry: async () => ({ global_seq: 2, revision: 2 }),
    };
    const workflow = createLibraryWorkflow(adapter);

    await workflow.refresh();

    expect(workflow.getState()).toMatchObject({
      kind: 'ready',
      rows: [
        { entry: { resource_id: 'item:known' }, media: { title: '标题 item:known' } },
        { entry: { resource_id: 'item:missing' }, media: null },
      ],
    });
  });

  it('更新写入 receipt，失败不伪造成功状态', async () => {
    const nextError: { value?: Error } = {};
    const adapter: LibraryWorkflowAdapter = {
      loadProjection: async () => projection([entry('item:one')]),
      loadMediaItems: async () => [media('item:one')],
      updateLibraryEntry: async (request) => {
        if (nextError.value) throw nextError.value;
        expect(request).toMatchObject({
          resource_id: 'item:one',
          favorite: true,
          pinned: true,
          expected_version: 1,
        });
        return { global_seq: 8, revision: 2 };
      },
    };
    const workflow = createLibraryWorkflow(adapter);
    await workflow.refresh();

    await workflow.updateOwnership('item:one', true, true);
    expect(workflow.getState()).toMatchObject({
      kind: 'ready',
      rows: [{ entry: { favorite: true, pinned: true, revision: 2 } }],
      receipts: { 'item:one': { global_seq: 8, revision: 2 } },
    });

    nextError.value = new Error('conflict');
    await workflow.updateOwnership('item:one', false, false);
    expect(workflow.getState()).toMatchObject({
      kind: 'ready',
      rows: [{ entry: { favorite: true, pinned: true, revision: 2 } }],
      failures: { 'item:one': 'conflict' },
    });
  });

  it('分别跟踪多个条目的并发更新', async () => {
    const one = deferred<{ global_seq: number; revision: number }>();
    const two = deferred<{ global_seq: number; revision: number }>();
    const adapter: LibraryWorkflowAdapter = {
      loadProjection: async () => projection([entry('item:one'), entry('item:two')]),
      loadMediaItems: async () => [media('item:one'), media('item:two')],
      updateLibraryEntry: async (request) =>
        request.resource_id === 'item:one' ? one.promise : two.promise,
    };
    const workflow = createLibraryWorkflow(adapter);
    await workflow.refresh();

    const updateOne = workflow.updateOwnership('item:one', true, true);
    const updateTwo = workflow.updateOwnership('item:two', false, false);
    expect(workflow.getState().pendingResourceIds).toEqual(['item:one', 'item:two']);

    one.resolve({ global_seq: 2, revision: 2 });
    await updateOne;
    expect(workflow.getState().pendingResourceIds).toEqual(['item:two']);

    two.resolve({ global_seq: 3, revision: 2 });
    await updateTwo;
    expect(workflow.getState().pendingResourceIds).toEqual([]);
  });
});
