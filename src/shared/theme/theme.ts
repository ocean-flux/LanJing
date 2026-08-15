/**
 * 主题与外观包。
 *
 * 职责边界：本模块只负责「选了哪套」以及把选择写到 <html> 的属性上，
 * 不持有任何色值。色值唯一来源是 src/index.css 第 4 段的
 * `:root[data-appearance-pack='...']` 四个块。
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

const STORAGE_KEY = 'lanjing-theme';
const PREFERENCES_KEY = 'lanjing-preferences-v1';
const LEGACY_PACK_KEY = 'appearance-pack';

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

const DARK_QUERY = '(prefers-color-scheme: dark)';

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

function readPreferences(): ThemePreferences {
  let theme: Theme = 'system';
  let lightThemeId = DEFAULT_LIGHT;
  let darkThemeId = DEFAULT_DARK;
  try {
    const legacyMode = window.localStorage.getItem(STORAGE_KEY);
    if (isTheme(legacyMode)) theme = legacyMode;

    const raw = window.localStorage.getItem(PREFERENCES_KEY);
    if (raw) {
      const stored = JSON.parse(raw) as Record<string, unknown>;
      const { mode, lightThemeId: storedLight, darkThemeId: storedDark } = stored;
      if (isTheme(mode)) theme = mode;
      if (isPack(storedLight) && PACK_FACE[storedLight] === 'light') {
        lightThemeId = storedLight;
      }
      if (isPack(storedDark) && PACK_FACE[storedDark] === 'dark') {
        darkThemeId = storedDark;
      }
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

let preferences: ThemePreferences =
  typeof window === 'undefined'
    ? { theme: 'system', lightThemeId: DEFAULT_LIGHT, darkThemeId: DEFAULT_DARK }
    : readPreferences();

const listeners = new Set<() => void>();

function persist() {
  try {
    window.localStorage.setItem(STORAGE_KEY, preferences.theme);
    window.localStorage.setItem(
      PREFERENCES_KEY,
      JSON.stringify({
        mode: preferences.theme,
        lightThemeId: preferences.lightThemeId,
        darkThemeId: preferences.darkThemeId,
      }),
    );
  } catch {
    // 无法持久化时保留当前会话内的选择。
  }
}

function emit() {
  for (const listener of listeners) listener();
}

export function activePackId(theme: Theme = preferences.theme): AppearancePackId {
  return resolveTheme(theme) === 'dark' ? preferences.darkThemeId : preferences.lightThemeId;
}

/**
 * 把当前选择写到 <html>：
 * - `.dark` 供 Tailwind 的 dark: 变体与 registry 组件使用
 * - `data-appearance-pack` 供 index.css 的色值块匹配
 * - `color-scheme` 供原生控件与滚动条配色
 * theme-color 从计算样式读取，避免在 JS 里维护第二份色值。
 */
export function applyTheme(theme: Theme = preferences.theme): ResolvedTheme {
  const resolved = resolveTheme(theme);
  const pack = activePackId(theme);
  const root = document.documentElement;
  root.classList.toggle('dark', resolved === 'dark');
  root.dataset.theme = theme;
  root.dataset.appearancePack = pack;
  root.style.colorScheme = resolved;

  const meta = document.head.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
  if (meta) {
    const canvas = getComputedStyle(root).getPropertyValue('--canvas').trim();
    if (canvas) meta.setAttribute('content', canvas);
  }
  return resolved;
}

export function getPreferences(): ThemePreferences {
  return preferences;
}

export function setTheme(theme: Theme) {
  if (preferences.theme === theme) return;
  preferences = { ...preferences, theme };
  applyTheme();
  persist();
  emit();
}

export function setAppearancePack(id: AppearancePackId) {
  const face = PACK_FACE[id];
  if (face === 'dark') {
    if (preferences.darkThemeId === id) return;
    preferences = { ...preferences, darkThemeId: id };
  } else {
    if (preferences.lightThemeId === id) return;
    preferences = { ...preferences, lightThemeId: id };
  }
  applyTheme();
  persist();
  emit();
}

export function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * 跟随系统：theme 为 system 时，系统配色变化要实时重算。
 * 在模块加载时挂载一次，整个应用生命周期内有效。
 */
if (typeof window !== 'undefined') {
  const query = window.matchMedia(DARK_QUERY);
  query.addEventListener('change', () => {
    if (preferences.theme !== 'system') return;
    applyTheme();
    emit();
  });
  applyTheme();
}
