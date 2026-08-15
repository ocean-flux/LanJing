import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

import {
  getInstalledSources,
  getError,
  getLoading,
  installCandidate,
  loadInstalledSources,
  prepareInstall,
  prepareMaccmsInstall,
  refreshInstalledSources,
} from './rules.svelte';

describe('rules RuleSystem wire', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('uses import-only prepare_install staging for the Legado source input', async () => {
    const candidate = {
      id: 'candidate:one',
      expected_installed_revision: 0,
      profile: {},
      diagnostics: [],
    };
    invoke.mockResolvedValue(candidate);

    const result = await prepareInstall('{"bookSourceUrl":"https://example.test"}');
    expect(result).toBe(candidate);
    expect(result).toMatchObject({ expected_installed_revision: 0 });
    expect(invoke).toHaveBeenCalledWith('prepare_install', {
      request: {
        kind: 'legado',
        source_json: '{"bookSourceUrl":"https://example.test"}',
      },
    });
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('uses import-only prepare_install staging for the Maccms JSON URL input', async () => {
    const candidate = {
      id: 'candidate:maccms',
      expected_installed_revision: 0,
      profile: {},
      diagnostics: [],
    };
    invoke.mockResolvedValue(candidate);

    const result = await prepareMaccmsInstall('https://api.example.test/provide/vod');
    expect(result).toBe(candidate);
    expect(result).toMatchObject({ expected_installed_revision: 0 });
    expect(invoke).toHaveBeenCalledWith('prepare_install', {
      request: {
        kind: 'maccms_json',
        url: 'https://api.example.test/provide/vod',
      },
    });
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('installs an opaque candidate and refreshes installed sources', async () => {
    const source = { source_id: 'source:one', profile: {}, revision: 1, version: 'v1' };
    invoke.mockResolvedValueOnce(source).mockResolvedValueOnce([source]);

    await expect(installCandidate('candidate:one', 'network_only')).resolves.toBe(source);
    expect(invoke).toHaveBeenNthCalledWith(1, 'install', {
      request: { candidate_id: 'candidate:one', grant: 'network_only' },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, 'list_installed_sources');
    expect(getInstalledSources()).toEqual([source]);
  });

  it('loads only installed source projections', async () => {
    invoke.mockResolvedValue([
      { source_id: 'source:two', profile: {}, revision: 2, version: 'v2' },
    ]);

    await loadInstalledSources();

    expect(invoke).toHaveBeenCalledWith('list_installed_sources');
  });

  it('rejects coordinator refresh failures while retaining observable store state', async () => {
    let rejectRefresh!: (reason?: unknown) => void;
    invoke.mockImplementationOnce(
      () =>
        new Promise<never>((_resolve, reject) => {
          rejectRefresh = reject;
        }),
    );

    const refresh = refreshInstalledSources();
    expect(getLoading()).toBe(true);
    expect(getError()).toBeNull();

    rejectRefresh(new Error('installed list unavailable'));
    await expect(refresh).rejects.toThrow('installed list unavailable');
    expect(getLoading()).toBe(false);
    expect(getError()).toBe('Error: installed list unavailable');
  });
});
