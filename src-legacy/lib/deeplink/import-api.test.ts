import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fetchImportSrc } from './import-api';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('deep link import API', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it('uses the Rust SSRF-protected request wrapper', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('[{"bookSourceName":"示例"}]');

    await expect(fetchImportSrc('https://example.test/sources.json')).resolves.toContain('示例');
    expect(invoke).toHaveBeenCalledWith('fetch_import_src', {
      request: { url: 'https://example.test/sources.json' },
    });
  });
});
