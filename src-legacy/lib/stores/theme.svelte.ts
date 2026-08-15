// 主题偏好：L0 明暗 + 双轨主题（亮/暗分选）+ 阅读器 + 材质。
// 桌面：@tauri-store/svelte RuneStore 落盘；Web/Vitest：localStorage 回退（不在模块顶层 new RuneStore）。
import { browser } from '$app/environment';
import { isTauri } from '@tauri-apps/api/core';
import { tick } from 'svelte';
import {
  DEFAULT_DARK_THEME_ID,
  DEFAULT_LIGHT_THEME_ID,
  THEME_REGISTRY,
  normalizeAppearancePackId,
  type AppearancePackId,
  type AppearanceTokenMap,
} from './appearance-packs';

export type ThemeMode = 'light' | 'dark' | 'system';
export type ResolvedTheme = 'light' | 'dark';
export type MaterialTransparency = 'standard' | 'low';

export type { AppearancePackId } from './appearance-packs';
export {
  BUILTIN_APPEARANCE_PACK_IDS,
  DEFAULT_DARK_THEME_ID,
  DEFAULT_LIGHT_THEME_ID,
  THEME_REGISTRY,
  listThemesForFace,
  normalizeAppearancePackId,
} from './appearance-packs';

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
 */
export type ThemePreferenceState = {
  mode: ThemeMode;
  lightThemeId: AppearancePackId;
  darkThemeId: AppearancePackId;
  materialTransparency: MaterialTransparency;
  textReaderTheme: TextReaderThemePreference;
};

/** Web/测试回退键；桌面由 RuneStore 文件承担 */
export const WEB_PREFERENCES_STORAGE_KEY = 'lanjing-preferences-v1';

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
  lightThemeId: DEFAULT_LIGHT_THEME_ID,
  darkThemeId: DEFAULT_DARK_THEME_ID,
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
    if (typeof o.lightThemeId === 'string') {
      next.lightThemeId = normalizeAppearancePackId(o.lightThemeId, 'light');
    }
    if (typeof o.darkThemeId === 'string') {
      next.darkThemeId = normalizeAppearancePackId(o.darkThemeId, 'dark');
    }
    if (isMaterial(o.materialTransparency)) next.materialTransparency = o.materialTransparency;
    if (isTextReaderTheme(o.textReaderTheme)) next.textReaderTheme = o.textReaderTheme;
    return next;
  } catch {
    return {};
  }
}

function buildInitialState(): ThemePreferenceState {
  const web = readWebFallback();
  return {
    mode: web.mode ?? DEFAULT_THEME_PREFERENCE_STATE.mode,
    lightThemeId: web.lightThemeId ?? DEFAULT_LIGHT_THEME_ID,
    darkThemeId: web.darkThemeId ?? DEFAULT_DARK_THEME_ID,
    materialTransparency: web.materialTransparency ?? DEFAULT_MATERIAL_TRANSPARENCY,
    textReaderTheme: {
      ...DEFAULT_TEXT_READER_THEME,
      ...(web.textReaderTheme ?? {}),
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
  lightThemeId: AppearancePackId,
  darkThemeId: AppearancePackId,
): AppearancePackId {
  return resolved === 'dark'
    ? normalizeAppearancePackId(darkThemeId, 'dark')
    : normalizeAppearancePackId(lightThemeId, 'light');
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

/** 应用单 face 主题 token（id 已按轨规范）。 */
function applyThemeFace(themeId: AppearancePackId, _resolved: ResolvedTheme): void {
  if (!browser) return;
  const face = _resolved;
  const normalized = normalizeAppearancePackId(themeId, face);
  document.documentElement.dataset.appearancePack = normalized;
  writePackTokens(THEME_REGISTRY[normalized].tokens);
}

function applyMaterialTransparency(value: MaterialTransparency): void {
  if (!browser) return;
  document.documentElement.dataset.materialTransparency = value;
  document.documentElement.classList.toggle('low-transparency', value === 'low');
}

function applyAllFromPreferenceState(): void {
  prefs.lightThemeId = normalizeAppearancePackId(prefs.lightThemeId, 'light');
  prefs.darkThemeId = normalizeAppearancePackId(prefs.darkThemeId, 'dark');
  const resolved = resolveTheme(prefs.mode);
  _currentTheme = resolved;
  applyTheme(resolved);
  applyThemeFace(resolveThemeIdForFace(resolved, prefs.lightThemeId, prefs.darkThemeId), resolved);
  applyMaterialTransparency(prefs.materialTransparency);
}

/** 将 RuneStore 首次载入或跨窗口 patch 回流到运行时与 DOM。 */
function applySyncedPreferencePatch(source: Partial<ThemePreferenceState>): void {
  if (isThemeMode(source.mode)) prefs.mode = source.mode;
  if (typeof source.lightThemeId === 'string') {
    prefs.lightThemeId = normalizeAppearancePackId(source.lightThemeId, 'light');
  }
  if (typeof source.darkThemeId === 'string') {
    prefs.darkThemeId = normalizeAppearancePackId(source.darkThemeId, 'dark');
  }
  if (isMaterial(source.materialTransparency)) {
    prefs.materialTransparency = source.materialTransparency;
  }
  if (isTextReaderTheme(source.textReaderTheme)) {
    prefs.textReaderTheme = { ...source.textReaderTheme };
  }
  applyAllFromPreferenceState();
}

function persistWebFallback(): void {
  if (!browser) return;
  localStorage.setItem(
    WEB_PREFERENCES_STORAGE_KEY,
    JSON.stringify({
      mode: prefs.mode,
      lightThemeId: prefs.lightThemeId,
      darkThemeId: prefs.darkThemeId,
      materialTransparency: prefs.materialTransparency,
      textReaderTheme: prefs.textReaderTheme,
    }),
  );
}

function syncPrefsToTauriRune(): void {
  if (!tauriRune) return;
  tauriRune.state.mode = prefs.mode;
  tauriRune.state.lightThemeId = prefs.lightThemeId;
  tauriRune.state.darkThemeId = prefs.darkThemeId;
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

  // 一次性清理旧版 localStorage key（新版本不再读取，仅卫生清理）。
  localStorage.removeItem('theme');
  localStorage.removeItem('appearance-pack');
  localStorage.removeItem('text-reader-theme');
  localStorage.removeItem('material-transparency');

  if (isTauri()) {
    try {
      const { RuneStore } = await import('@tauri-store/svelte');
      const rune = new RuneStore<ThemePreferenceState>(
        'lanjing-preferences',
        {
          mode: prefs.mode,
          lightThemeId: prefs.lightThemeId,
          darkThemeId: prefs.darkThemeId,
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
          hooks: {
            beforeFrontendSync: (state) => {
              if (tauriRune) applySyncedPreferencePatch(state as Partial<ThemePreferenceState>);
              // 宽容处理旧磁盘数据：过滤掉已知的旧字段
              const next = { ...(state as Record<string, unknown>) };
              delete next.appearancePackId;
              return next as ThemePreferenceState;
            },
          },
        },
      );
      await rune.start();
      tauriRune = rune;
      applySyncedPreferencePatch(rune.state);
      await tick();
      syncPrefsToTauriRune();
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
  const handleSystemThemeChange = (event: MediaQueryListEvent) => {
    if (prefs.mode !== 'system') return;
    const resolved: ResolvedTheme = event.matches ? 'dark' : 'light';
    _currentTheme = resolved;
    applyTheme(resolved);
    applyThemeFace(
      resolveThemeIdForFace(resolved, prefs.lightThemeId, prefs.darkThemeId),
      resolved,
    );
    if (tauriRune) syncPrefsToTauriRune();
    else persistWebFallback();
  };
  media.addEventListener('change', handleSystemThemeChange);
  import.meta.hot?.dispose(() => media.removeEventListener('change', handleSystemThemeChange));
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
export function getLightThemeId(): AppearancePackId {
  return normalizeAppearancePackId(prefs.lightThemeId, 'light');
}

/** 设置亮面主题（仅影响 resolved=light 时的 face） */
export function setLightThemeId(id: string): void {
  prefs.lightThemeId = normalizeAppearancePackId(id, 'light');
  afterStateMutation();
}

/** 暗面选用主题 */
export function getDarkThemeId(): AppearancePackId {
  return normalizeAppearancePackId(prefs.darkThemeId, 'dark');
}

/** 设置暗面主题（仅影响 resolved=dark 时的 face） */
export function setDarkThemeId(id: string): void {
  prefs.darkThemeId = normalizeAppearancePackId(id, 'dark');
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
