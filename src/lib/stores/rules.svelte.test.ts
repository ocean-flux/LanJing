import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

import {
  getInstalledSources,
  installCandidate,
  loadInstalledSources,
  prepareInstall,
} from './rules.svelte';

describe('rules RuleSystem wire', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('uses prepare_install for the Legado source input', async () => {
    const candidate = { id: 'candidate:one', profile: {}, diagnostics: [] };
    invoke.mockResolvedValue(candidate);

    await expect(prepareInstall('{"bookSourceUrl":"https://example.test"}')).resolves.toBe(
      candidate,
    );
    expect(invoke).toHaveBeenCalledWith('prepare_install', {
      request: {
        kind: 'legado',
        source_json: '{"bookSourceUrl":"https://example.test"}',
      },
    });
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
});
