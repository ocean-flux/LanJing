import { describe, expect, it } from 'vitest';
import {
  isSettingsPathname,
  resolveForegroundActivity,
  resolvePlatformCapabilities,
  resolvePrimaryChromeFamily,
  resolveProductContext,
  resolveShellMode,
} from './shell-mode';

describe('resolveShellMode', () => {
  it.each([
    [{ width: 390, hover: 'none' as const, pointer: 'coarse' as const }, 'mobile'],
    [{ width: 900, hover: 'none' as const, pointer: 'coarse' as const }, 'tablet-portrait'],
    [{ width: 1100, hover: 'none' as const, pointer: 'coarse' as const }, 'tablet-landscape'],
    [{ width: 1100, hover: 'hover' as const, pointer: 'fine' as const }, 'narrow-desktop'],
    [{ width: 1440, hover: 'hover' as const, pointer: 'fine' as const }, 'desktop'],
  ])('maps %o to %s', (input, expected) => {
    expect(resolveShellMode(input)).toBe(expected);
  });

  it.each([
    ['mobile', 'bottom'],
    ['tablet-portrait', 'bottom'],
    ['tablet-landscape', 'titlebar'],
    ['narrow-desktop', 'titlebar'],
    ['desktop', 'titlebar'],
  ] as const)('maps shell mode %s to chrome family %s', (mode, family) => {
    expect(resolvePrimaryChromeFamily(mode)).toBe(family);
  });

  it('resolves product context and foreground activity from live routes', () => {
    expect(resolveProductContext('/library')).toBe('library');
    expect(resolveProductContext('/apps')).toBe('apps');
    expect(resolveForegroundActivity('/apps')).toEqual({ kind: 'browse', id: 'apps' });
  });

  it('identifies settings pathname without adding a fifth primary route', () => {
    expect(isSettingsPathname('/settings')).toBe(true);
    expect(isSettingsPathname('/settings/theme')).toBe(true);
    expect(isSettingsPathname('/library')).toBe(false);
  });

  it('keeps platform capabilities explicit across orientation changes', () => {
    expect(
      resolvePlatformCapabilities({
        kind: 'android',
        width: 390,
        height: 844,
        hover: 'none',
        pointer: 'coarse',
        tauri: true,
      }),
    ).toMatchObject({
      kind: 'android',
      orientation: 'portrait',
      keyboard: false,
      touch: true,
      windowControls: 'system-decorated',
    });
    expect(
      resolvePlatformCapabilities({
        kind: 'windows',
        width: 1440,
        height: 900,
        hover: 'hover',
        pointer: 'fine',
        tauri: true,
      }),
    ).toMatchObject({
      kind: 'windows',
      orientation: 'landscape',
      keyboard: true,
      touch: false,
      windowControls: 'windows-overlay',
    });
  });

  it('keeps explicit OS independent from viewport and pointer capabilities', () => {
    expect(
      resolvePlatformCapabilities({
        kind: 'ios',
        width: 1440,
        height: 900,
        hover: 'hover',
        pointer: 'fine',
        tauri: true,
      }),
    ).toMatchObject({
      kind: 'ios',
      orientation: 'landscape',
      keyboard: true,
      touch: false,
      windowControls: 'system-decorated',
    });
  });
});
