import type { CapabilityKey, MediaAppKey } from '$lib/brand';

export type ShellRoute = 'realm' | 'apps' | 'sources' | 'library';
export type ProductContext = ShellRoute;
export type MediaSpace = MediaAppKey | null;

export type ForegroundActivity = {
  kind: 'browse' | 'reader' | 'player';
  id?: string;
};

export type PlatformKind = 'browser' | 'windows' | 'macos' | 'linux' | 'ios' | 'android';
export type Orientation = 'portrait' | 'landscape';
export type NativeWindowControlMode =
  | 'browser-preview'
  | 'macos-overlay'
  | 'system-decorated'
  | 'windows-overlay';

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

export type ShellThemeState = {
  mode: 'light' | 'dark' | 'system';
  /** L2 appearance pack id; production always default `paper-lantern-precision`. */
  appearancePack: 'paper-lantern-precision';
  reducedMotion: boolean;
  reducedTransparency: boolean;
};

export type AmbientAudioSession = {
  id: string;
  state: 'playing' | 'paused';
  focus: 'none' | 'ambient' | 'foreground';
  label: string;
} | null;

export type ModeShellContract = {
  productContext: ProductContext;
  mediaSpace: MediaSpace;
  foregroundActivity: ForegroundActivity;
  presentation: ShellPresentationMode;
  platform: PlatformCapabilities;
  theme: ShellThemeState;
  ambientAudio: AmbientAudioSession;
};

export type ShellMode =
  | 'mobile'
  | 'tablet-portrait'
  | 'tablet-landscape'
  | 'narrow-desktop'
  | 'desktop';

export type PointerKind = 'coarse' | 'fine';
export type HoverKind = 'none' | 'hover';

export type ShellPresentationMode = 'normal' | 'reader' | 'player';

export type SourceUiStatus = 'ready' | 'partial' | 'failed' | 'disabled' | 'unchecked';

export type MediaAppStatus = 'unconnected' | 'explorable' | 'has-content' | 'failed';

export type SourceTrustFact = {
  label: string;
  value: string;
};

export type SourceCardState = {
  id: string;
  name: string;
  kind: string;
  status: SourceUiStatus;
  summary: string;
  capabilities: Partial<Record<CapabilityKey, boolean>>;
  trustFacts: SourceTrustFact[];
  actions: string[];
  checkedAt?: string;
};

export type MediaAppCardState = {
  key: MediaAppKey;
  label: string;
  description: string;
  status: MediaAppStatus;
  statusLabel: string;
  primaryAction: string;
  href?: string;
};

export type RealmStateKind = 'no-source' | 'source-no-resource' | 'source-warning' | 'has-content';

export type RealmState = {
  kind: RealmStateKind;
  title: string;
  description: string;
  primaryAction: string;
  secondaryAction: string;
  sourceSummary?: string;
};

export type TextReaderThemePreference = {
  colorScheme: 'paper' | 'white' | 'gray' | 'dark' | 'black';
  fontFamily: 'system' | 'serif' | 'sans' | 'fangsong';
  fontSize: number;
  lineHeight: number;
  paragraphSpacing: string;
  contentWidth: 'narrow' | 'standard' | 'wide';
  indentFirstLine: boolean;
  pageMode: 'scroll' | 'paged';
};

export type MiniPlayerSlotState = {
  reserved: boolean;
  visible: boolean;
  label: string;
};
