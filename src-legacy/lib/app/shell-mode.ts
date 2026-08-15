/**
 * 壳模式解析：由 pathname / 视口 / 显式平台输入推导产品上下文、前台活动与平台能力。
 * 纯函数；不读写 store，供 ModeShell 装配契约。
 */
import type {
  ForegroundActivity,
  HoverKind,
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

/** 平台能力解析输入；OS 必须由 runtime owner 显式提供。 */
export type PlatformInput = {
  kind: PlatformKind;
  width: number;
  height: number;
  hover: HoverKind;
  pointer: PointerKind;
  tauri?: boolean;
};

/** 由路径解析顶层产品上下文（境场/应用/来源/资料库）。 */
export function resolveProductContext(pathname: string): ProductContext {
  if (pathname.startsWith('/apps')) return 'apps';
  if (pathname.startsWith('/sources')) return 'sources';
  if (pathname.startsWith('/library')) return 'library';
  return 'realm';
}

/** 路由默认前台活动（可被会话 override 覆盖）。 */
export function resolveForegroundActivity(pathname: string): ForegroundActivity {
  return { kind: 'browse', id: resolveProductContext(pathname) };
}

function resolveWindowControls(kind: PlatformKind, tauri: boolean): NativeWindowControlMode {
  if (!tauri) return 'browser-preview';
  // macOS：交通灯由平台覆盖配置提供，不重复渲染 HTML 控件。
  if (kind === 'macos') return 'macos-overlay';
  // Windows / Linux：无边框窗口使用 AppTitlebar HTML 标题控件。
  if (kind === 'windows' || kind === 'linux') return 'windows-overlay';
  return 'system-decorated';
}

/** 汇总平台能力：OS 类、朝向、指针与窗口控件模式。 */
export function resolvePlatformCapabilities(input: PlatformInput): PlatformCapabilities {
  const tauri = input.tauri ?? (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window);
  const orientation: Orientation = input.width >= input.height ? 'landscape' : 'portrait';

  return {
    kind: input.kind,
    orientation,
    viewportWidth: input.width,
    viewportHeight: input.height,
    hover: input.hover,
    pointer: input.pointer,
    keyboard: input.pointer === 'fine',
    touch: input.pointer === 'coarse',
    windowControls: resolveWindowControls(input.kind, tauri),
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

/** 主导航 chrome 族：桌面标题栏 vs 移动底栏（互斥）。 */
export type PrimaryChromeFamily = 'titlebar' | 'bottom';

/**
 * 由壳断点解析主导航族。
 * mobile / tablet-portrait → bottom；其余 → titlebar。
 */
export function resolvePrimaryChromeFamily(mode: ShellMode): PrimaryChromeFamily {
  return mode === 'mobile' || mode === 'tablet-portrait' ? 'bottom' : 'titlebar';
}

/** 设置路由：非四境，不点亮 productContext active。 */
export function isSettingsPathname(pathname: string): boolean {
  return pathname.startsWith('/settings');
}
