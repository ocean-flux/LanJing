import { describe, expect, it, vi } from 'vitest';
import { listSourceRevisions, prepareSourceInstall, prepareSourceRollback } from './sources';

const tauriBoundary = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: unknown) => Promise<unknown>>(),
  isTauri: vi.fn<() => boolean>(() => true),
}));

vi.mock('@tauri-apps/api/core', () => tauriBoundary);

describe('source IPC adapter', () => {
  it('preserves the explicit current-contract input kind', async () => {
    tauriBoundary.invoke.mockResolvedValueOnce({});
    await prepareSourceInstall({ kind: 'maccms_json', url: 'https://example.test/vod/' });
    expect(tauriBoundary.invoke).toHaveBeenCalledWith('prepare_install', {
      request: { kind: 'maccms_json', url: 'https://example.test/vod/' },
    });

    tauriBoundary.invoke.mockClear();
    tauriBoundary.invoke.mockResolvedValueOnce({});
    await prepareSourceInstall('{"bookSourceName":"本地来源"}');
    expect(tauriBoundary.invoke).toHaveBeenCalledWith('prepare_install', {
      request: { kind: 'legado', source_json: '{"bookSourceName":"本地来源"}' },
    });
  });

  it('uses request envelopes for source history and rollback', async () => {
    tauriBoundary.invoke.mockResolvedValueOnce([]);
    await listSourceRevisions('source.example');
    expect(tauriBoundary.invoke).toHaveBeenCalledWith('list_source_revisions', {
      request: { source_id: 'source.example' },
    });

    tauriBoundary.invoke.mockClear();
    tauriBoundary.invoke.mockResolvedValueOnce({});
    await prepareSourceRollback('source.example', 4);
    expect(tauriBoundary.invoke).toHaveBeenCalledWith('prepare_source_rollback', {
      request: { source_id: 'source.example', revision: 4 },
    });
  });
});
