import { isTauri } from '@tauri-apps/api/core';
import { createTauriStore } from '@tauri-store/zustand';
import { create } from 'zustand';

/**
 * 主题与外观包。
 *
 * 职责边界：本模块只负责「选了哪套」以及把选择写到 <html> 的属性上，
 * 不持有任何色值。色值唯一来源是 src/index.css 第 4 段的
 * `:root[data-appearance-pack='...']` 四个块。
 *
 * 存储分工：
 * - 真实来源是 @tauri-store/zustand 的 `ui-preferences`，由 Rust 侧持久化到
 *   应用数据目录，能跨窗口同步，也不会被 WebView 清除存储时丢掉。
 * - 同时把同一份偏好镜像写进 localStorage，仅供 index.html 的 bootstrap
 *   在首帧绘制前同步读取。tauri-store 的 start() 是异步的，若只依赖它，
 *   暗色主题冷启动会闪白。镜像是写入的派生副作用，不是第二个真实来源。
 */

export type Theme = 'light' | 'dark' | 'system';
export type ResolvedTheme = 'light' | 'dark';
export type AppearancePackId =
  | 'obsidian-void'
  | 'graphite-atelier'
  | 'porcelain-day'
  | 'mist-studio';

export type ThemePreferences = {
  theme: Theme;
  lightThemeId: AppearancePackId;
  darkThemeId: AppearancePackId;
};

/** 与 index.html 的 bootstrap 共用这个 key，两边必须一致。 */
const PAINT_CACHE_KEY = 'lanjing-preferences-v1';
const LEGACY_MODE_KEY = 'lanjing-theme';
const LEGACY_PACK_KEY = 'appearance-pack';
const TAURI_STORE_ID = 'ui-preferences';

const DARK_QUERY = '(prefers-color-scheme: dark)';

/** 每套外观包只有单面，这里决定它属于亮轨还是暗轨。 */
export const PACK_FACE: Record<AppearancePackId, ResolvedTheme> = {
  'obsidian-void': 'dark',
  'graphite-atelier': 'dark',
  'porcelain-day': 'light',
  'mist-studio': 'light',
};

export const LIGHT_PACK_IDS: readonly AppearancePackId[] = ['porcelain-day', 'mist-studio'];
export const DARK_PACK_IDS: readonly AppearancePackId[] = ['obsidian-void', 'graphite-atelier'];

const DEFAULT_LIGHT: AppearancePackId = 'porcelain-day';
const DEFAULT_DARK: AppearancePackId = 'obsidian-void';

function isTheme(value: unknown): value is Theme {
  return value === 'light' || value === 'dark' || value === 'system';
}

function isPack(value: unknown): value is AppearancePackId {
  return typeof value === 'string' && value in PACK_FACE;
}

function prefersDark(): boolean {
  return typeof window !== 'undefined' && window.matchMedia(DARK_QUERY).matches;
}

export function resolveTheme(theme: Theme): ResolvedTheme {
  if (theme !== 'system') return theme;
  return prefersDark() ? 'dark' : 'light';
}

/** 读绘制前缓存，同时兼容两个历史 key。 */
function readPaintCache(): ThemePreferences {
  let theme: Theme = 'system';
  let lightThemeId = DEFAULT_LIGHT;
  let darkThemeId = DEFAULT_DARK;
  try {
    const legacyMode = window.localStorage.getItem(LEGACY_MODE_KEY);
    if (isTheme(legacyMode)) theme = legacyMode;

    const raw = window.localStorage.getItem(PAINT_CACHE_KEY);
    if (raw) {
      const stored = JSON.parse(raw) as Record<string, unknown>;
      const { mode, lightThemeId: storedLight, darkThemeId: storedDark } = stored;
      if (isTheme(mode)) theme = mode;
      if (isPack(storedLight) && PACK_FACE[storedLight] === 'light') lightThemeId = storedLight;
      if (isPack(storedDark) && PACK_FACE[storedDark] === 'dark') darkThemeId = storedDark;
    }

    const legacyPack = window.localStorage.getItem(LEGACY_PACK_KEY);
    if (isPack(legacyPack)) {
      if (PACK_FACE[legacyPack] === 'light') lightThemeId = legacyPack;
      else darkThemeId = legacyPack;
    }
  } catch {
    // 受限 WebView 会拒绝 localStorage，此时保持默认值可用。
  }
  return { theme, lightThemeId, darkThemeId };
}

function writePaintCache(preferences: ThemePreferences) {
  try {
    window.localStorage.setItem(LEGACY_MODE_KEY, preferences.theme);
    window.localStorage.setItem(
      PAINT_CACHE_KEY,
      JSON.stringify({
        mode: preferences.theme,
        lightThemeId: preferences.lightThemeId,
        darkThemeId: preferences.darkThemeId,
      }),
    );
  } catch {
    // 无法写缓存时只影响下次冷启动的首帧，不影响当前会话。
  }
}

type PreferencesStore = ThemePreferences & {
  setTheme: (theme: Theme) => void;
  setAppearancePack: (id: AppearancePackId) => void;
};

export const usePreferencesStore = create<PreferencesStore>((set) => ({
  ...(typeof window === 'undefined'
    ? { theme: 'system' as Theme, lightThemeId: DEFAULT_LIGHT, darkThemeId: DEFAULT_DARK }
    : readPaintCache()),
  setTheme: (theme) => set({ theme }),
  setAppearancePack: (id) =>
    set(PACK_FACE[id] === 'dark' ? { darkThemeId: id } : { lightThemeId: id }),
}));

/** Rust 侧持久化句柄。start() 由 startPreferencesSync() 调用，未启动时前端仍可正常工作。 */
export const preferencesTauriStore = createTauriStore(TAURI_STORE_ID, usePreferencesStore, {
  autoStart: false,
  saveOnChange: true,
  // 动作不进持久化，只存偏好本身。
  filterKeys: ['setTheme', 'setAppearancePack'],
  filterKeysStrategy: 'omit',
});

/**
 * 启动 Rust 侧持久化。只在 Tauri 运行时下有意义；浏览器里跑 vite dev 时
 * 直接跳过，偏好退化为仅 localStorage，页面功能不受影响。
 *
 * 失败原因原样返回给调用方去提示，这里既不吞掉也不代为决定提示形态。
 */
export async function startPreferencesPersistence(): Promise<Error | null> {
  if (!isTauri()) return null;
  try {
    await preferencesTauriStore.start();
    return null;
  } catch (error) {
    return error instanceof Error ? error : new Error(String(error));
  }
}

export function getPreferences(): ThemePreferences {
  const { theme, lightThemeId, darkThemeId } = usePreferencesStore.getState();
  return { theme, lightThemeId, darkThemeId };
}

export function activePackId(preferences: ThemePreferences = getPreferences()): AppearancePackId {
  return resolveTheme(preferences.theme) === 'dark'
    ? preferences.darkThemeId
    : preferences.lightThemeId;
}

/**
 * 把当前选择写到 <html>：
 * - `.dark` 供 Tailwind 的 dark: 变体与 registry 组件使用
 * - `data-appearance-pack` 供 index.css 的色值块匹配
 * - `color-scheme` 供原生控件与滚动条配色
 * theme-color 从计算样式读取，避免在 JS 里维护第二份色值。
 */
export function applyTheme(preferences: ThemePreferences = getPreferences()): ResolvedTheme {
  const resolved = resolveTheme(preferences.theme);
  const pack = activePackId(preferences);
  const root = document.documentElement;
  root.classList.toggle('dark', resolved === 'dark');
  root.dataset.theme = preferences.theme;
  root.dataset.appearancePack = pack;
  root.style.colorScheme = resolved;

  const meta = document.head.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
  if (meta) {
    const canvas = getComputedStyle(root).getPropertyValue('--canvas').trim();
    if (canvas) meta.setAttribute('content', canvas);
  }
  return resolved;
}

export function setTheme(theme: Theme) {
  usePreferencesStore.getState().setTheme(theme);
}

export function setAppearancePack(id: AppearancePackId) {
  usePreferencesStore.getState().setAppearancePack(id);
}

if (typeof window !== 'undefined') {
  // 偏好变化时重放到 <html> 并刷新绘制前缓存。
  // 订阅同时覆盖本地操作与 tauri-store 从 Rust / 其他窗口推回来的变更。
  usePreferencesStore.subscribe((state) => {
    const preferences: ThemePreferences = {
      theme: state.theme,
      lightThemeId: state.lightThemeId,
      darkThemeId: state.darkThemeId,
    };
    applyTheme(preferences);
    writePaintCache(preferences);
  });

  // 跟随系统：theme 为 system 时，系统配色变化要实时重算。
  window.matchMedia(DARK_QUERY).addEventListener('change', () => {
    if (usePreferencesStore.getState().theme !== 'system') return;
    applyTheme();
  });

  applyTheme();
}
