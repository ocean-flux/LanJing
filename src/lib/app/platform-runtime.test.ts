import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { resolveRuntimePlatform } from './platform-runtime';

const platform = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/plugin-os', () => ({ platform }));

type TauriWindow = Window & { __TAURI_INTERNALS__?: object };

beforeEach(() => {
  platform.mockReset();
  delete (window as TauriWindow).__TAURI_INTERNALS__;
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('resolveRuntimePlatform', () => {
  it('returns unknown in a browser without requesting an OS value', async () => {
    await expect(resolveRuntimePlatform()).resolves.toBe('unknown');
    expect(platform).not.toHaveBeenCalled();
  });

  it('returns unknown during SSR', async () => {
    vi.stubGlobal('window', undefined);
    await expect(resolveRuntimePlatform()).resolves.toBe('unknown');
    expect(platform).not.toHaveBeenCalled();
  });

  it.each(['windows', 'macos', 'linux', 'ios', 'android'] as const)(
    'maps the explicit Tauri platform %s without a viewport or UA heuristic',
    async (value) => {
      (window as TauriWindow).__TAURI_INTERNALS__ = {};
      platform.mockReturnValue(value);
      await expect(resolveRuntimePlatform()).resolves.toBe(value);
    },
  );

  it('maps unsupported Tauri operating systems to unknown', async () => {
    (window as TauriWindow).__TAURI_INTERNALS__ = {};
    platform.mockReturnValue('freebsd');
    await expect(resolveRuntimePlatform()).resolves.toBe('unknown');
  });

  it('maps plugin failures to unknown', async () => {
    (window as TauriWindow).__TAURI_INTERNALS__ = {};
    platform.mockImplementation(() => {
      throw new Error('plugin unavailable');
    });
    await expect(resolveRuntimePlatform()).resolves.toBe('unknown');
  });
});
