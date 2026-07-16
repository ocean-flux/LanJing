import type {
  ForegroundActivity,
  HoverKind,
  MediaSpace,
  NativeWindowControlMode,
  Orientation,
  PlatformCapabilities,
  PlatformKind,
  PointerKind,
  ProductContext,
  ShellMode,
} from './shell-types';

export type ShellModeInput = {
  width: number;
  hover: HoverKind;
  pointer: PointerKind;
};

export type PlatformInput = {
  width: number;
  height: number;
  hover: HoverKind;
  pointer: PointerKind;
  userAgent?: string;
  tauri?: boolean;
};

export function resolveProductContext(pathname: string): ProductContext {
  if (pathname.startsWith('/apps')) return 'apps';
  if (pathname.startsWith('/sources')) return 'sources';
  if (pathname.startsWith('/library')) return 'library';
  return 'realm';
}

export function resolveMediaSpace(pathname: string): MediaSpace {
  if (pathname.startsWith('/apps/novel')) return 'novel';
  if (pathname.startsWith('/apps/music')) return 'music';
  if (pathname.startsWith('/apps/comic')) return 'comic';
  if (pathname.startsWith('/apps/video')) return 'video';
  if (pathname.startsWith('/apps/images')) return 'images';
  if (pathname.startsWith('/apps/podcast')) return 'podcast';
  if (pathname.startsWith('/apps/article')) return 'article';
  if (pathname.startsWith('/apps/local')) return 'local';
  return null;
}

export function resolveForegroundActivity(pathname: string): ForegroundActivity {
  if (pathname.startsWith('/apps/novel/read')) return { kind: 'reader' };
  if (pathname.startsWith('/apps'))
    return { kind: 'browse', id: resolveMediaSpace(pathname) ?? undefined };
  return { kind: 'browse', id: resolveProductContext(pathname) };
}

export function resolvePresentation(pathname: string) {
  return pathname.startsWith('/apps/novel/read') ? ('reader' as const) : ('normal' as const);
}

function resolvePlatformKind(userAgent: string): PlatformKind {
  const value = userAgent.toLowerCase();
  if (value.includes('android')) return 'android';
  if (value.includes('iphone') || value.includes('ipad')) return 'ios';
  if (value.includes('windows')) return 'windows';
  if (value.includes('mac')) return 'macos';
  if (value.includes('linux')) return 'linux';
  return 'browser';
}

function resolveWindowControls(kind: PlatformKind, tauri: boolean): NativeWindowControlMode {
  if (!tauri) return 'browser-preview';
  // macOS: system traffic lights + Overlay title bar (tauri.conf).
  if (kind === 'macos') return 'macos-overlay';
  // Windows: tauri-plugin-window-controls injects native caption buttons.
  if (kind === 'windows') return 'windows-overlay';
  // Linux and others: keep system window decorations.
  return 'system-decorated';
}

export function resolvePlatformCapabilities(input: PlatformInput): PlatformCapabilities {
  const userAgent =
    input.userAgent ?? (typeof navigator === 'undefined' ? '' : navigator.userAgent);
  const tauri =
    input.tauri ??
    (typeof window !== 'undefined' &&
      ('__TAURI_INTERNALS__' in window || userAgent.toLowerCase().includes('tauri')));
  const kind = resolvePlatformKind(userAgent);
  const orientation: Orientation = input.width >= input.height ? 'landscape' : 'portrait';

  return {
    kind,
    orientation,
    viewportWidth: input.width,
    viewportHeight: input.height,
    hover: input.hover,
    pointer: input.pointer,
    keyboard: input.pointer === 'fine',
    touch: input.pointer === 'coarse',
    windowControls: resolveWindowControls(kind, tauri),
  };
}

export function resolveShellMode({ width, hover, pointer }: ShellModeInput): ShellMode {
  const touch = hover === 'none' || pointer === 'coarse';

  if (width < 768) return 'mobile';
  if (width < 1024 && touch) return 'tablet-portrait';
  if (width < 1200 && touch) return 'tablet-landscape';
  if (width < 1200) return 'narrow-desktop';
  return 'desktop';
}

export function usesBottomNav(mode: ShellMode): boolean {
  return mode === 'mobile' || mode === 'tablet-portrait';
}

export function usesIconRail(mode: ShellMode): boolean {
  return mode === 'tablet-landscape' || mode === 'narrow-desktop';
}

export function usesDesktopRail(mode: ShellMode): boolean {
  return mode === 'desktop';
}
