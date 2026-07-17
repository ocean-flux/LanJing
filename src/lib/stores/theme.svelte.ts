// 主题偏好：L0 明暗 + 双轨主题（亮/暗分选）+ 阅读器 + 材质。
// 桌面：@tauri-store/svelte RuneStore 落盘；Web/Vitest：localStorage 回退（不在模块顶层 new RuneStore）。
import { browser } from '$app/environment';
import { isTauri } from '@tauri-apps/api/core';
import {
  APPEARANCE_PACK_TOKENS,
  DEFAULT_APPEARANCE_PACK_ID,
  normalizeAppearancePackId,
  type AppearancePackId,
  type AppearanceTokenMap,
} from './appearance-packs';

export type ThemeMode = 'light' | 'dark' | 'system';
export type ResolvedTheme = 'light' | 'dark';
export type MaterialTransparency = 'standard' | 'low';

/** 产品层「主题」id；代码可与 appearance pack id 同构。 */
export type ThemeId = AppearancePackId;

export type { AppearancePackId } from './appearance-packs';
export {
  BUILTIN_APPEARANCE_PACK_IDS,
  DEFAULT_APPEARANCE_PACK_ID,
  LEGACY_APPEARANCE_PACK_MAP,
  normalizeAppearancePackId,
} from './appearance-packs';

/**
 * L2 Appearance pack：内置墨砚 / 冷银朱；只重绑 L1 角色值。
 * 纸灯 id 经 normalize 映射到默认墨砚。
 * 兼容读：当前 resolved 面所用主题。
 */
export type AppearancePack = {
  id: AppearancePackId;
  tokens?: Readonly<Record<string, string>>;
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

/**
 * 持久化偏好。
 * lightThemeId / darkThemeId：解析为亮/暗面时分别取用的手搓主题 face。
 * appearancePackId：兼容字段，始终同步为当前 resolved 主题（旧存储迁移源）。
 */
export type ThemePreferenceState = {
  mode: ThemeMode;
  lightThemeId: ThemeId;
  darkThemeId: ThemeId;
  /** @deprecated 兼容读/写；迁移后与 resolved 轨主题同步 */
  appearancePackId: AppearancePackId;
  materialTransparency: MaterialTransparency;
  textReaderTheme: TextReaderThemePreference;
};

const LEGACY_MODE_KEY = 'theme';
const LEGACY_PACK_KEY = 'appearance-pack';
const LEGACY_READER_KEY = 'text-reader-theme';
const LEGACY_MATERIAL_KEY = 'material-transparency';
/** Web/测试回退键；桌面由 RuneStore 文件承担 */
export const WEB_PREFERENCES_STORAGE_KEY = 'lanjing-preferences-v1';

export const DEFAULT_APPEARANCE_PACK: AppearancePack = {
  id: DEFAULT_APPEARANCE_PACK_ID,
};

export const DEFAULT_TEXT_READER_THEME: TextReaderThemePreference = {
  colorScheme: 'paper',
  fontFamily: 'serif',
  fontSize: 18,
  lineHeight: 1.75,
  paragraphSpacing: '0.85em',
  contentWidth: 'standard',
  indentFirstLine: false,
  pageMode: 'scroll',
};

export const DEFAULT_MATERIAL_TRANSPARENCY: MaterialTransparency = 'standard';

export const DEFAULT_THEME_PREFERENCE_STATE: ThemePreferenceState = {
  mode: 'system',
  lightThemeId: DEFAULT_APPEARANCE_PACK_ID,
  darkThemeId: DEFAULT_APPEARANCE_PACK_ID,
  appearancePackId: DEFAULT_APPEARANCE_PACK_ID,
  materialTransparency: DEFAULT_MATERIAL_TRANSPARENCY,
  textReaderTheme: { ...DEFAULT_TEXT_READER_THEME },
};

function isThemeMode(value: unknown): value is ThemeMode {
  return value === 'light' || value === 'dark' || value === 'system';
}

function isMaterial(value: unknown): value is MaterialTransparency {
  return value === 'standard' || value === 'low';
}

function isTextReaderTheme(value: unknown): value is TextReaderThemePreference {
  if (!value || typeof value !== 'object') return false;
  const theme = value as Record<string, unknown>;

  return (
    (theme.colorScheme === 'paper' ||
      theme.colorScheme === 'white' ||
      theme.colorScheme === 'gray' ||
      theme.colorScheme === 'dark' ||
      theme.colorScheme === 'black') &&
    (theme.fontFamily === 'system' ||
      theme.fontFamily === 'serif' ||
      theme.fontFamily === 'sans' ||
      theme.fontFamily === 'fangsong') &&
    typeof theme.fontSize === 'number' &&
    typeof theme.lineHeight === 'number' &&
    typeof theme.paragraphSpacing === 'string' &&
    (theme.contentWidth === 'narrow' ||
      theme.contentWidth === 'standard' ||
      theme.contentWidth === 'wide') &&
    typeof theme.indentFirstLine === 'boolean' &&
    (theme.pageMode === 'scroll' || theme.pageMode === 'paged')
  );
}

/** 从旧单 pack 或分轨字段解析亮/暗主题 id。 */
function resolveTrackThemeIds(source: {
  lightThemeId?: unknown;
  darkThemeId?: unknown;
  appearancePackId?: unknown;
}): { lightThemeId: ThemeId; darkThemeId: ThemeId } {
  const legacyPack =
    typeof source.appearancePackId === 'string'
      ? normalizeAppearancePackId(source.appearancePackId)
      : DEFAULT_APPEARANCE_PACK_ID;
  const light =
    typeof source.lightThemeId === 'string'
      ? normalizeAppearancePackId(source.lightThemeId)
      : legacyPack;
  const dark =
    typeof source.darkThemeId === 'string'
      ? normalizeAppearancePackId(source.darkThemeId)
      : legacyPack;
  return { lightThemeId: light, darkThemeId: dark };
}

function migrateLegacyLocalStorage(): Partial<ThemePreferenceState> {
  if (!browser) return {};
  const next: Partial<ThemePreferenceState> = {};

  const mode = localStorage.getItem(LEGACY_MODE_KEY);
  if (isThemeMode(mode)) next.mode = mode;

  const pack = localStorage.getItem(LEGACY_PACK_KEY);
  if (pack) {
    const id = normalizeAppearancePackId(pack);
    next.appearancePackId = id;
    next.lightThemeId = id;
    next.darkThemeId = id;
  }

  const material = localStorage.getItem(LEGACY_MATERIAL_KEY);
  if (isMaterial(material)) next.materialTransparency = material;

  try {
    const raw = localStorage.getItem(LEGACY_READER_KEY);
    if (raw) {
      const parsed: unknown = JSON.parse(raw);
      if (isTextReaderTheme(parsed)) next.textReaderTheme = parsed;
    }
  } catch {
    /* ignore */
  }

  return next;
}

function readWebFallback(): Partial<ThemePreferenceState> {
  if (!browser) return {};
  try {
    const raw = localStorage.getItem(WEB_PREFERENCES_STORAGE_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object') return {};
    const o = parsed as Record<string, unknown>;
    const next: Partial<ThemePreferenceState> = {};
    if (isThemeMode(o.mode)) next.mode = o.mode;
    const tracks = resolveTrackThemeIds(o);
    next.lightThemeId = tracks.lightThemeId;
    next.darkThemeId = tracks.darkThemeId;
    next.appearancePackId = tracks.lightThemeId;
    if (isMaterial(o.materialTransparency)) next.materialTransparency = o.materialTransparency;
    if (isTextReaderTheme(o.textReaderTheme)) next.textReaderTheme = o.textReaderTheme;
    return next;
  } catch {
    return {};
  }
}

function buildInitialState(): ThemePreferenceState {
  const legacy = migrateLegacyLocalStorage();
  const web = readWebFallback();
  const tracks = resolveTrackThemeIds({
    lightThemeId: web.lightThemeId ?? legacy.lightThemeId,
    darkThemeId: web.darkThemeId ?? legacy.darkThemeId,
    appearancePackId: web.appearancePackId ?? legacy.appearancePackId ?? DEFAULT_APPEARANCE_PACK_ID,
  });
  return {
    mode: web.mode ?? legacy.mode ?? DEFAULT_THEME_PREFERENCE_STATE.mode,
    lightThemeId: tracks.lightThemeId,
    darkThemeId: tracks.darkThemeId,
    appearancePackId: tracks.lightThemeId,
    materialTransparency:
      web.materialTransparency ?? legacy.materialTransparency ?? DEFAULT_MATERIAL_TRANSPARENCY,
    textReaderTheme: {
      ...DEFAULT_TEXT_READER_THEME,
      ...(web.textReaderTheme ?? legacy.textReaderTheme ?? {}),
    },
  };
}

/** 运行时偏好（权威内存态）；Tauri 启动后与 RuneStore 双向同步 */
const prefs = $state<ThemePreferenceState>(buildInitialState());

type PreferenceRuneStore = {
  state: ThemePreferenceState;
  start: () => Promise<void>;
};

let tauriRune: PreferenceRuneStore | null = null;
let _currentTheme = $state<ResolvedTheme>('light');
let _started = false;

function readSystemTheme(): ResolvedTheme {
  if (!browser) return 'light';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function resolveTheme(mode: ThemeMode): ResolvedTheme {
  return mode === 'system' ? readSystemTheme() : mode;
}

/** 按已解析明暗面选用对应轨主题 id（禁止从另一面算法反相）。 */
export function resolveThemeIdForFace(
  resolved: ResolvedTheme,
  lightThemeId: ThemeId,
  darkThemeId: ThemeId,
): ThemeId {
  return resolved === 'dark'
    ? normalizeAppearancePackId(darkThemeId)
    : normalizeAppearancePackId(lightThemeId);
}

function applyTheme(theme: ResolvedTheme): void {
  if (!browser) return;
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
  document.documentElement.classList.toggle('dark', theme === 'dark');
}

function writePackTokens(tokens: AppearanceTokenMap): void {
  if (!browser) return;
  const root = document.documentElement;
  for (const [key, value] of Object.entries(tokens) as [keyof AppearanceTokenMap, string][]) {
    root.style.setProperty(key, value);
  }
}

/** 应用指定主题的手搓 light/dark face token。 */
function applyThemeFace(themeId: ThemeId, resolved: ResolvedTheme): void {
  if (!browser) return;
  const normalized = normalizeAppearancePackId(themeId);
  document.documentElement.dataset.appearancePack = normalized;
  writePackTokens(APPEARANCE_PACK_TOKENS[normalized][resolved]);
}

function applyMaterialTransparency(value: MaterialTransparency): void {
  if (!browser) return;
  document.documentElement.dataset.materialTransparency = value;
  document.documentElement.classList.toggle('low-transparency', value === 'low');
}

function applyAllFromPreferenceState(): void {
  prefs.lightThemeId = normalizeAppearancePackId(prefs.lightThemeId);
  prefs.darkThemeId = normalizeAppearancePackId(prefs.darkThemeId);
  const resolved = resolveTheme(prefs.mode);
  _currentTheme = resolved;
  const activeThemeId = resolveThemeIdForFace(resolved, prefs.lightThemeId, prefs.darkThemeId);
  // 兼容 getter：当前生效主题写回 appearancePackId
  prefs.appearancePackId = activeThemeId;
  applyTheme(resolved);
  applyThemeFace(activeThemeId, resolved);
  applyMaterialTransparency(prefs.materialTransparency);
}

function persistWebFallback(): void {
  if (!browser) return;
  localStorage.setItem(
    WEB_PREFERENCES_STORAGE_KEY,
    JSON.stringify({
      mode: prefs.mode,
      lightThemeId: prefs.lightThemeId,
      darkThemeId: prefs.darkThemeId,
      // 兼容旧读路径
      appearancePackId: prefs.appearancePackId,
      materialTransparency: prefs.materialTransparency,
      textReaderTheme: prefs.textReaderTheme,
    }),
  );
}

function clearLegacyKeys(): void {
  if (!browser) return;
  localStorage.removeItem(LEGACY_MODE_KEY);
  localStorage.removeItem(LEGACY_PACK_KEY);
  localStorage.removeItem(LEGACY_READER_KEY);
  localStorage.removeItem(LEGACY_MATERIAL_KEY);
}

function syncPrefsToTauriRune(): void {
  if (!tauriRune) return;
  tauriRune.state.mode = prefs.mode;
  tauriRune.state.lightThemeId = prefs.lightThemeId;
  tauriRune.state.darkThemeId = prefs.darkThemeId;
  tauriRune.state.appearancePackId = prefs.appearancePackId;
  tauriRune.state.materialTransparency = prefs.materialTransparency;
  tauriRune.state.textReaderTheme = { ...prefs.textReaderTheme };
}

function afterStateMutation(): void {
  applyAllFromPreferenceState();
  if (tauriRune) {
    syncPrefsToTauriRune();
  } else {
    persistWebFallback();
  }
}

/**
 * 启动偏好持久化：Tauri 惰性创建 RuneStore 并 start；Web 写 localStorage。
 * 幂等；布局 onMount 应 await 一次。
 */
export async function startThemePreferences(): Promise<void> {
  if (!browser) {
    applyAllFromPreferenceState();
    return;
  }

  applyAllFromPreferenceState();

  if (_started) return;
  _started = true;

  if (isTauri()) {
    try {
      const { RuneStore } = await import('@tauri-store/svelte');
      const rune = new RuneStore<ThemePreferenceState>(
        'lanjing-preferences',
        {
          mode: prefs.mode,
          lightThemeId: prefs.lightThemeId,
          darkThemeId: prefs.darkThemeId,
          appearancePackId: prefs.appearancePackId,
          materialTransparency: prefs.materialTransparency,
          textReaderTheme: { ...prefs.textReaderTheme },
        },
        {
          autoStart: false,
          saveOnChange: true,
          saveStrategy: 'debounce',
          saveInterval: 250,
          syncStrategy: 'debounce',
          syncInterval: 250,
        },
      );
      await rune.start();
      // 磁盘态覆盖内存（并规范化双轨主题）
      prefs.mode = isThemeMode(rune.state.mode) ? rune.state.mode : prefs.mode;
      const tracks = resolveTrackThemeIds(rune.state as ThemePreferenceState);
      prefs.lightThemeId = tracks.lightThemeId;
      prefs.darkThemeId = tracks.darkThemeId;
      if (isMaterial(rune.state.materialTransparency)) {
        prefs.materialTransparency = rune.state.materialTransparency;
      }
      if (isTextReaderTheme(rune.state.textReaderTheme)) {
        prefs.textReaderTheme = { ...rune.state.textReaderTheme };
      }
      tauriRune = rune;
      applyAllFromPreferenceState();
      clearLegacyKeys();
    } catch (error) {
      console.warn('[theme] RuneStore 启动失败，回退 localStorage', error);
      persistWebFallback();
    }
  } else {
    persistWebFallback();
  }
}

if (browser) {
  applyAllFromPreferenceState();

  const media = window.matchMedia('(prefers-color-scheme: dark)');
  media.addEventListener('change', (event) => {
    if (prefs.mode !== 'system') return;
    const resolved: ResolvedTheme = event.matches ? 'dark' : 'light';
    _currentTheme = resolved;
    const activeThemeId = resolveThemeIdForFace(resolved, prefs.lightThemeId, prefs.darkThemeId);
    prefs.appearancePackId = activeThemeId;
    applyTheme(resolved);
    applyThemeFace(activeThemeId, resolved);
  });
}

/** 读取当前主题模式 */
export function getMode(): ThemeMode {
  return prefs.mode;
}

/** 读取当前已解析主题 */
export function getCurrentTheme(): ResolvedTheme {
  return _currentTheme;
}

/** 设置主题模式 */
export function setMode(value: ThemeMode): void {
  prefs.mode = value;
  afterStateMutation();
}

/** 在 light / dark 之间切换（基于当前已解析主题） */
export function toggle(): void {
  setMode(_currentTheme === 'dark' ? 'light' : 'dark');
}

/** 亮面选用主题 */
export function getLightThemeId(): ThemeId {
  return normalizeAppearancePackId(prefs.lightThemeId);
}

/** 设置亮面主题（仅影响 resolved=light 时的 face） */
export function setLightThemeId(id: string): void {
  prefs.lightThemeId = normalizeAppearancePackId(id);
  afterStateMutation();
}

/** 暗面选用主题 */
export function getDarkThemeId(): ThemeId {
  return normalizeAppearancePackId(prefs.darkThemeId);
}

/** 设置暗面主题（仅影响 resolved=dark 时的 face） */
export function setDarkThemeId(id: string): void {
  prefs.darkThemeId = normalizeAppearancePackId(id);
  afterStateMutation();
}

/** 读取文本阅读器主题 */
export function getTextReaderTheme(): TextReaderThemePreference {
  return { ...prefs.textReaderTheme };
}

/** 整体替换文本阅读器主题 */
export function setTextReaderTheme(value: TextReaderThemePreference): void {
  prefs.textReaderTheme = { ...value };
  afterStateMutation();
}

/** 局部更新文本阅读器主题 */
export function updateTextReaderTheme(patch: Partial<TextReaderThemePreference>): void {
  setTextReaderTheme({ ...prefs.textReaderTheme, ...patch });
}

/** 读取材质透明度 */
export function getMaterialTransparency(): MaterialTransparency {
  return prefs.materialTransparency;
}

/** 设置材质透明度 */
export function setMaterialTransparency(value: MaterialTransparency): void {
  prefs.materialTransparency = value;
  afterStateMutation();
}

/**
 * 按辅助偏好应用有效材质透明度，不改写用户存储偏好。
 */
export function syncMaterialTransparencyForA11y(reducedTransparency: boolean): void {
  const effective: MaterialTransparency =
    reducedTransparency || prefs.materialTransparency === 'low' ? 'low' : 'standard';
  applyMaterialTransparency(effective);
}

/**
 * 读取当前 resolved 面所用主题（兼容旧 getAppearancePack）。
 */
export function getAppearancePack(): AppearancePack {
  return {
    id: resolveThemeIdForFace(_currentTheme, prefs.lightThemeId, prefs.darkThemeId),
  };
}

/**
 * 兼容：将亮/暗两轨同时设为同一主题 id（旧「单 pack」语义）。
 */
export function setAppearancePack(
  pack: AppearancePack | { id: string; tokens?: AppearancePack['tokens'] },
): void {
  const id = normalizeAppearancePackId(pack.id);
  prefs.lightThemeId = id;
  prefs.darkThemeId = id;
  afterStateMutation();
}
