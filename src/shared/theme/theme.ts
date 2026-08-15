export type Theme = 'light' | 'dark' | 'system';
export type AppearancePackId =
  | 'obsidian-void'
  | 'graphite-atelier'
  | 'porcelain-day'
  | 'mist-studio';
export type ResolvedTheme = 'light' | 'dark';

type ThemeTokens = Record<string, string>;
type ThemeEvent = { theme: Theme; lightThemeId: AppearancePackId; darkThemeId: AppearancePackId };

const STORAGE_KEY = 'lanjing-theme';
const PREFERENCES_KEY = 'lanjing-preferences-v1';
const LEGACY_PACK_KEY = 'appearance-pack';
const THEME_EVENT = 'lanjing:theme-change';

export const BUILTIN_APPEARANCE_PACK_IDS: readonly AppearancePackId[] = [
  'obsidian-void',
  'graphite-atelier',
  'porcelain-day',
  'mist-studio',
];

const PACK_FACE: Record<AppearancePackId, ResolvedTheme> = {
  'obsidian-void': 'dark',
  'graphite-atelier': 'dark',
  'porcelain-day': 'light',
  'mist-studio': 'light',
};

const PACK_TOKENS: Record<AppearancePackId, ThemeTokens> = {
  'obsidian-void': {
    '--canvas': '#0b0e12',
    '--canvas-elevated': '#12171d',
    '--ink': '#e8eaed',
    '--ink-muted': '#9aa3ad',
    '--ink-subtle': '#6f7882',
    '--hairline': 'rgb(232 234 237 / 0.12)',
    '--hairline-strong': 'rgb(232 234 237 / 0.2)',
    '--surface-1': '#151a20',
    '--surface-2': '#1b2229',
    '--surface-3': '#252d36',
    '--lantern': '#6ec8d4',
    '--lantern-strong': '#3aa9b8',
    '--lantern-hover': '#2f96a4',
    '--lantern-soft': 'rgb(110 200 212 / 0.18)',
    '--lantern-tint': '#143038',
    '--on-lantern': '#061016',
    '--media-void': '#12171d',
    '--reader-canvas': '#1a1714',
    '--reader-ink': '#d8d2c4',
    '--ring': 'rgb(58 169 184 / 0.34)',
    '--focus-ring': '0 0 0 2px rgb(58 169 184 / 0.28)',
  },
  'graphite-atelier': {
    '--canvas': '#111416',
    '--canvas-elevated': '#181c1e',
    '--ink': '#eceff0',
    '--ink-muted': '#a4abad',
    '--ink-subtle': '#7d8588',
    '--hairline': 'rgb(236 239 240 / 0.1)',
    '--hairline-strong': 'rgb(236 239 240 / 0.16)',
    '--surface-1': '#1c2022',
    '--surface-2': '#252a2c',
    '--surface-3': '#303638',
    '--lantern': '#79a6ab',
    '--lantern-strong': '#4d777d',
    '--lantern-hover': '#456b71',
    '--lantern-soft': 'rgb(121 166 171 / 0.16)',
    '--lantern-tint': '#1b2b2e',
    '--on-lantern': '#f2f8f8',
    '--media-void': '#171d1f',
    '--reader-canvas': '#1a1714',
    '--reader-ink': '#d8d2c4',
    '--ring': 'rgb(77 119 125 / 0.34)',
    '--focus-ring': '0 0 0 2px rgb(77 119 125 / 0.28)',
  },
  'porcelain-day': {
    '--canvas': '#f3f4f6',
    '--canvas-elevated': '#ffffff',
    '--ink': '#1a1b1e',
    '--ink-muted': '#5c616a',
    '--ink-subtle': '#7a808a',
    '--hairline': 'rgb(26 27 30 / 0.1)',
    '--hairline-strong': 'rgb(26 27 30 / 0.16)',
    '--surface-1': '#ffffff',
    '--surface-2': '#f6f7f9',
    '--surface-3': '#e8eaee',
    '--lantern': '#0f6e7a',
    '--lantern-strong': '#0b5a64',
    '--lantern-hover': '#094c55',
    '--lantern-soft': 'rgb(15 110 122 / 0.12)',
    '--lantern-tint': '#d9e8ea',
    '--on-lantern': '#f4fcfd',
    '--media-void': '#e4e6ea',
    '--reader-canvas': '#f3efe6',
    '--reader-ink': '#211e1a',
    '--ring': 'rgb(11 90 100 / 0.34)',
    '--focus-ring': '0 0 0 2px rgb(11 90 100 / 0.24)',
  },
  'mist-studio': {
    '--canvas': '#eef1f5',
    '--canvas-elevated': '#f7f9fc',
    '--ink': '#171a1f',
    '--ink-muted': '#5a6370',
    '--ink-subtle': '#76808f',
    '--hairline': 'rgb(23 26 31 / 0.1)',
    '--hairline-strong': 'rgb(23 26 31 / 0.16)',
    '--surface-1': '#ffffff',
    '--surface-2': '#f1f4f8',
    '--surface-3': '#e4e9f0',
    '--lantern': '#0e7490',
    '--lantern-strong': '#0b5f75',
    '--lantern-hover': '#0a5568',
    '--lantern-soft': 'rgb(14 116 144 / 0.12)',
    '--lantern-tint': '#d9eef3',
    '--on-lantern': '#f3fbfd',
    '--media-void': '#e2e7ee',
    '--reader-canvas': '#f3efe6',
    '--reader-ink': '#211e1a',
    '--ring': 'rgb(11 95 117 / 0.34)',
    '--focus-ring': '0 0 0 2px rgb(11 95 117 / 0.24)',
  },
};

const DEFAULT_LIGHT: AppearancePackId = 'porcelain-day';
const DEFAULT_DARK: AppearancePackId = 'obsidian-void';

function isTheme(value: unknown): value is Theme {
  return value === 'light' || value === 'dark' || value === 'system';
}
function isPack(value: unknown): value is AppearancePackId {
  return typeof value === 'string' && value in PACK_TOKENS;
}
function resolveMode(mode: Theme): ResolvedTheme {
  return mode === 'system' &&
    typeof window !== 'undefined' &&
    window.matchMedia('(prefers-color-scheme: dark)').matches
    ? 'dark'
    : mode === 'system'
      ? 'light'
      : mode;
}
function readPreferences(): ThemeEvent {
  let mode: Theme = 'system';
  let lightThemeId = DEFAULT_LIGHT;
  let darkThemeId = DEFAULT_DARK;
  try {
    const legacyMode = window.localStorage.getItem(STORAGE_KEY);
    if (isTheme(legacyMode)) mode = legacyMode;
    const raw = window.localStorage.getItem(PREFERENCES_KEY);
    if (raw) {
      const value = JSON.parse(raw) as Record<string, unknown>;
      const {
        mode: storedMode,
        lightThemeId: storedLightThemeId,
        darkThemeId: storedDarkThemeId,
      } = value;
      if (isTheme(storedMode)) mode = storedMode;
      if (isPack(storedLightThemeId) && PACK_FACE[storedLightThemeId] === 'light')
        lightThemeId = storedLightThemeId;
      if (isPack(storedDarkThemeId) && PACK_FACE[storedDarkThemeId] === 'dark')
        darkThemeId = storedDarkThemeId;
    }
    const legacyPack = window.localStorage.getItem(LEGACY_PACK_KEY);
    if (isPack(legacyPack)) {
      if (PACK_FACE[legacyPack] === 'light') lightThemeId = legacyPack;
      else darkThemeId = legacyPack;
    }
  } catch {
    // Restricted WebViews can reject localStorage; defaults remain usable.
  }
  return { theme: mode, lightThemeId, darkThemeId };
}

let preferences = readPreferences();

function persistPreferences() {
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
    // Keep the current session preference when persistence is unavailable.
  }
}

function getAppearancePackId(face: ResolvedTheme): AppearancePackId {
  return face === 'dark' ? preferences.darkThemeId : preferences.lightThemeId;
}
export function getAppearancePackIds() {
  return { lightThemeId: preferences.lightThemeId, darkThemeId: preferences.darkThemeId };
}
export function setAppearancePack(id: AppearancePackId) {
  if (PACK_FACE[id] === 'dark') preferences.darkThemeId = id;
  else preferences.lightThemeId = id;
  applyTheme(preferences.theme);
  persistPreferences();
  window.dispatchEvent(new CustomEvent<ThemeEvent>(THEME_EVENT, { detail: preferences }));
}

export function applyTheme(theme: Theme) {
  const resolved = resolveMode(theme);
  const pack = getAppearancePackId(resolved);
  const root = document.documentElement;
  root.classList.toggle('dark', resolved === 'dark');
  root.dataset.theme = theme;
  root.dataset.appearancePack = pack;
  root.style.colorScheme = resolved;
  const themeColor = document.head.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
  themeColor?.setAttribute('content', PACK_TOKENS[pack]['--canvas']);
  return resolved;
}

export function readTheme(): Theme {
  return preferences.theme;
}
export function persistTheme(theme: Theme) {
  preferences = { ...preferences, theme };
  persistPreferences();
  applyTheme(theme);
  window.dispatchEvent(new CustomEvent<ThemeEvent>(THEME_EVENT, { detail: preferences }));
}
export function subscribeToTheme(listener: (theme: Theme) => void) {
  const handleChange = (event: Event) => {
    const { detail } = event as CustomEvent<ThemeEvent>;
    const { theme } = detail ?? {};
    if (detail && isTheme(theme)) {
      preferences = detail;
      listener(theme);
    }
  };
  window.addEventListener(THEME_EVENT, handleChange);
  return () => window.removeEventListener(THEME_EVENT, handleChange);
}

if (typeof document !== 'undefined') {
  const { theme: initialTheme } = preferences;
  applyTheme(initialTheme);
}
