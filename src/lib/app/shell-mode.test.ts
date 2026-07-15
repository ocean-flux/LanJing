import { describe, expect, it } from 'vitest';
import {
  resolveForegroundActivity,
  resolveMediaSpace,
  resolvePlatformCapabilities,
  resolveProductContext,
  resolveShellMode,
  usesBottomNav,
  usesDesktopRail,
  usesIconRail,
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

  it('keeps nav families distinct', () => {
    expect(usesBottomNav('mobile')).toBe(true);
    expect(usesBottomNav('tablet-portrait')).toBe(true);
    expect(usesIconRail('tablet-landscape')).toBe(true);
    expect(usesIconRail('narrow-desktop')).toBe(true);
    expect(usesDesktopRail('desktop')).toBe(true);
  });

  it('resolves product context, media space, and one foreground activity from route', () => {
    expect(resolveProductContext('/library')).toBe('library');
    expect(resolveMediaSpace('/apps/novel/read/7')).toBe('novel');
    expect(resolveForegroundActivity('/apps/novel/read/7')).toEqual({ kind: 'reader' });
    expect(resolveForegroundActivity('/apps/music')).toEqual({ kind: 'browse', id: 'music' });
  });

  it('keeps platform capabilities explicit across orientation changes', () => {
    expect(
      resolvePlatformCapabilities({
        width: 390,
        height: 844,
        hover: 'none',
        pointer: 'coarse',
        userAgent: 'Android',
        tauri: true,
      }),
    ).toMatchObject({
      kind: 'android',
      orientation: 'portrait',
      keyboard: false,
      touch: true,
    });
    expect(
      resolvePlatformCapabilities({
        width: 1440,
        height: 900,
        hover: 'hover',
        pointer: 'fine',
        userAgent: 'Windows NT',
        tauri: true,
      }),
    ).toMatchObject({
      kind: 'windows',
      orientation: 'landscape',
      keyboard: true,
      touch: false,
      windowControls: 'system-decorated',
    });
  });
});
