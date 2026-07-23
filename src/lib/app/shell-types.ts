/**
 * 壳契约与 UI 状态类型：ModeShell / AppShell 的唯一数据形状。
 * 不含实现；字段名与 DOM data-* / 测试断言对齐。
 */
import type { MediaAppKey } from '$lib/brand';
import type { AppearancePackId } from '$lib/stores/appearance-packs';

/** 主导航四境路由键。 */
export type ShellRoute = 'realm' | 'apps' | 'sources' | 'library';
/** 产品上下文；与 ShellRoute 同构。 */
export type ProductContext = ShellRoute;
/** 当前媒体空间；非媒体页为 null。 */
export type MediaSpace = MediaAppKey | null;

/** 前台活动：浏览 / 阅读 / 播放（可带会话 id）。 */
export type ForegroundActivity = {
  kind: 'browse' | 'reader' | 'player';
  id?: string;
};

/** 运行平台粗分类。 */
export type PlatformKind = 'browser' | 'windows' | 'macos' | 'linux' | 'ios' | 'android';
export type Orientation = 'portrait' | 'landscape';
/** 原生窗口控件策略（含浏览器预览与各 OS overlay）。 */
export type NativeWindowControlMode =
  'browser-preview' | 'macos-overlay' | 'system-decorated' | 'windows-overlay';

/** 平台能力快照：视口、指针与窗口控件模式。 */
export type PlatformCapabilities = {
  kind: PlatformKind;
  orientation: Orientation;
  viewportWidth: number;
  viewportHeight: number;
  hover: HoverKind;
  pointer: PointerKind;
  keyboard: boolean;
  touch: boolean;
  windowControls: NativeWindowControlMode;
};

/** 壳主题状态（L0 模式 + L2 pack + a11y 标志）。 */
export type ShellThemeState = {
  mode: 'light' | 'dark' | 'system';
  /** L2 主题 id；默认暗 `obsidian-void`，亮轨常用 `porcelain-day`。 */
  appearancePack: AppearancePackId;
  reducedMotion: boolean;
  reducedTransparency: boolean;
};

/** 环境音频会话；无会话时为 null。 */
export type AmbientAudioSession = {
  id: string;
  state: 'playing' | 'paused';
  focus: 'none' | 'ambient' | 'foreground';
  label: string;
} | null;

/** ModeShell 下发给 AppShell 的完整契约（单一真相源）。 */
export type ModeShellContract = {
  productContext: ProductContext;
  settingsActive: boolean;
  mediaSpace: MediaSpace;
  foregroundActivity: ForegroundActivity;
  presentation: ShellPresentationMode;
  platform: PlatformCapabilities;
  theme: ShellThemeState;
  ambientAudio: AmbientAudioSession;
};

/** 响应式壳断点。 */
export type ShellMode =
  'mobile' | 'tablet-portrait' | 'tablet-landscape' | 'narrow-desktop' | 'desktop';

export type PointerKind = 'coarse' | 'fine';
export type HoverKind = 'none' | 'hover';

/** 呈现层：普通 / 沉浸阅读 / 播放。 */
export type ShellPresentationMode = 'normal' | 'reader' | 'player';
