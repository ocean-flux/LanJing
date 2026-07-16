/**
 * 壳模式解析：由 pathname / 视口 / UA 推导产品上下文、媒体空间与平台能力。
 * 纯函数；不读写 store，供 ModeShell 装配契约。
 */
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

/** 壳断点输入：宽 + 指针/悬停能力。 */
export type ShellModeInput = {
  width: number;
  hover: HoverKind;
  pointer: PointerKind;
};

/** 平台能力解析输入；缺省从 navigator / window 补。 */
export type PlatformInput = {
  width: number;
  height: number;
  hover: HoverKind;
  pointer: PointerKind;
  userAgent?: string;
  tauri?: boolean;
};

/** 由路径解析顶层产品上下文（境场/应用/来源/资料库）。 */
export function resolveProductContext(pathname: string): ProductContext {
  if (pathname.startsWith('/apps')) return 'apps';
  if (pathname.startsWith('/sources')) return 'sources';
  if (pathname.startsWith('/library')) return 'library';
  return 'realm';
}

/** 由路径解析媒体空间；非媒体应用路由为 null。 */
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

/** 路由默认前台活动（可被会话 override 覆盖）。 */
export function resolveForegroundActivity(pathname: string): ForegroundActivity {
  if (pathname.startsWith('/apps/novel/read')) return { kind: 'reader' };
  if (pathname.startsWith('/apps'))
    return { kind: 'browse', id: resolveMediaSpace(pathname) ?? undefined };
  return { kind: 'browse', id: resolveProductContext(pathname) };
}

/** 呈现模式：阅读器路径为 reader，其余 normal。 */
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
  // macOS：交通灯由 Overlay 标题栏（tauri.conf）提供，不用 HTML 标题条。
  if (kind === 'macos') return 'macos-overlay';
  // Windows / Linux：无边框窗口 + AppTitlebar HTML 标题控件（官方 window API）。
  // `windows-overlay` 表示应用内自定义 chrome，不是第三方插件。
  if (kind === 'windows' || kind === 'linux') return 'windows-overlay';
  return 'windows-overlay';
}

/** 汇总平台能力：OS 类、朝向、指针与窗口控件模式。 */
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

/** 宽 + 触控启发 → 壳模式断点（mobile…desktop）。 */
export function resolveShellMode({ width, hover, pointer }: ShellModeInput): ShellMode {
  const touch = hover === 'none' || pointer === 'coarse';

  if (width < 768) return 'mobile';
  if (width < 1024 && touch) return 'tablet-portrait';
  if (width < 1200 && touch) return 'tablet-landscape';
  if (width < 1200) return 'narrow-desktop';
  return 'desktop';
}

/** 是否使用底部主导航。 */
export function usesBottomNav(mode: ShellMode): boolean {
  return mode === 'mobile' || mode === 'tablet-portrait';
}

/** 是否使用图标轨（窄桌面 / 横屏平板）。 */
export function usesIconRail(mode: ShellMode): boolean {
  return mode === 'tablet-landscape' || mode === 'narrow-desktop';
}

/** 是否使用展开桌面轨。 */
export function usesDesktopRail(mode: ShellMode): boolean {
  return mode === 'desktop';
}
