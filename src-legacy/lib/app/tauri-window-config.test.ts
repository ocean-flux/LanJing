import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

type TauriWindowConfig = {
  width?: number;
  height?: number;
  minWidth?: number;
  minHeight?: number;
  resizable?: boolean;
  decorations?: boolean;
  titleBarStyle?: string;
};

type TauriConfig = {
  app?: {
    windows?: TauriWindowConfig[];
  };
};

function loadConfig(path: string): TauriConfig {
  return JSON.parse(readFileSync(resolve(process.cwd(), path), 'utf8')) as TauriConfig;
}

describe('production Tauri window configuration', () => {
  it.each([
    ['base', 'src-tauri/tauri.conf.json'],
    ['macOS override', 'src-tauri/tauri.macos.conf.json'],
  ])('keeps the %s window usable at 768px', (_name, path) => {
    const config = loadConfig(path);
    const windows = config.app?.windows ?? [];

    expect(windows).toHaveLength(1);
    expect(windows[0]).toMatchObject({
      width: 1440,
      height: 960,
      minWidth: 768,
      minHeight: 720,
      resizable: true,
    });
  });

  it('preserves each platform chrome strategy', () => {
    const baseWindow = loadConfig('src-tauri/tauri.conf.json').app?.windows?.[0];
    const macWindow = loadConfig('src-tauri/tauri.macos.conf.json').app?.windows?.[0];

    expect(baseWindow?.decorations).toBe(false);
    expect(macWindow).toMatchObject({ decorations: true, titleBarStyle: 'Overlay' });
  });
});
