// 主题 store：L0 light/dark/system + L2 内置 appearance pack（墨砚默认 / 冷银朱）
import { browser } from '$app/environment';
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

export type { AppearancePackId } from './appearance-packs';
export {
  BUILTIN_APPEARANCE_PACK_IDS,
  DEFAULT_APPEARANCE_PACK_ID,
  LEGACY_APPEARANCE_PACK_MAP,
  normalizeAppearancePackId,
} from './appearance-packs';

/**
 * L2 Appearance pack：内置墨砚 / 冷银朱；只重绑 L1 角色值。
 * 无用户自定义商店；纸灯 id 经 normalize 映射到默认墨砚。
 */
export type AppearancePack = {
  id: AppearancePackId;
  /** 预留覆盖；内置包主色表在 appearance-packs.ts */
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

const STORAGE_KEY = 'theme';
const APPEARANCE_PACK_STORAGE_KEY = 'appearance-pack';
const TEXT_READER_THEME_STORAGE_KEY = 'text-reader-theme';
const MATERIAL_TRANSPARENCY_STORAGE_KEY = 'material-transparency';

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

function readStoredMode(): ThemeMode {
  if (!browser) return 'system';
  const stored = localStorage.getItem(STORAGE_KEY);
  return stored === 'light' || stored === 'dark' || stored === 'system' ? stored : 'system';
}

function readStoredAppearancePackId(): AppearancePackId {
  if (!browser) return DEFAULT_APPEARANCE_PACK_ID;
  const stored = localStorage.getItem(APPEARANCE_PACK_STORAGE_KEY);
  if (!stored) return DEFAULT_APPEARANCE_PACK_ID;
  return normalizeAppearancePackId(stored);
}

function readSystemTheme(): ResolvedTheme {
  if (!browser) return 'light';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function resolveTheme(mode: ThemeMode): ResolvedTheme {
  return mode === 'system' ? readSystemTheme() : mode;
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

function applyAppearancePack(pack: AppearancePack, resolved: ResolvedTheme): void {
  if (!browser) return;
  const id = normalizeAppearancePackId(pack.id);
  document.documentElement.dataset.appearancePack = id;
  const table = APPEARANCE_PACK_TOKENS[id][resolved];
  writePackTokens(table);
  // 用户 tokens 覆盖（预留）
  if (pack.tokens) {
    for (const [key, value] of Object.entries(pack.tokens)) {
      if (key.startsWith('--') && typeof value === 'string') {
        document.documentElement.style.setProperty(key, value);
      }
    }
  }
}

function applyMaterialTransparency(value: MaterialTransparency): void {
  if (!browser) return;
  document.documentElement.dataset.materialTransparency = value;
  document.documentElement.classList.toggle('low-transparency', value === 'low');
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

function readStoredTextReaderTheme(): TextReaderThemePreference {
  if (!browser) return DEFAULT_TEXT_READER_THEME;

  try {
    const stored = localStorage.getItem(TEXT_READER_THEME_STORAGE_KEY);
    if (!stored) return DEFAULT_TEXT_READER_THEME;

    const parsed: unknown = JSON.parse(stored);
    return isTextReaderTheme(parsed) ? parsed : DEFAULT_TEXT_READER_THEME;
  } catch {
    return DEFAULT_TEXT_READER_THEME;
  }
}

function persistTextReaderTheme(value: TextReaderThemePreference): void {
  if (!browser) return;
  localStorage.setItem(TEXT_READER_THEME_STORAGE_KEY, JSON.stringify(value));
}

function readStoredMaterialTransparency(): MaterialTransparency {
  if (!browser) return DEFAULT_MATERIAL_TRANSPARENCY;

  const stored = localStorage.getItem(MATERIAL_TRANSPARENCY_STORAGE_KEY);
  return stored === 'standard' || stored === 'low' ? stored : DEFAULT_MATERIAL_TRANSPARENCY;
}

function persistMaterialTransparency(value: MaterialTransparency): void {
  if (!browser) return;
  localStorage.setItem(MATERIAL_TRANSPARENCY_STORAGE_KEY, value);
}

function persistAppearancePackId(id: AppearancePackId): void {
  if (!browser) return;
  localStorage.setItem(APPEARANCE_PACK_STORAGE_KEY, id);
}

const initialMode = readStoredMode();
const initialResolved = resolveTheme(initialMode);
const initialTextReaderTheme = readStoredTextReaderTheme();
const initialMaterialTransparency = readStoredMaterialTransparency();
const initialAppearancePack: AppearancePack = {
  id: readStoredAppearancePackId(),
};

let _mode = $state<ThemeMode>(initialMode);
let _currentTheme = $state<ResolvedTheme>(initialResolved);
let _textReaderTheme = $state<TextReaderThemePreference>(initialTextReaderTheme);
let _materialTransparency = $state<MaterialTransparency>(initialMaterialTransparency);
let _appearancePack = $state<AppearancePack>(initialAppearancePack);

applyTheme(initialResolved);
applyAppearancePack(initialAppearancePack, initialResolved);
applyMaterialTransparency(initialMaterialTransparency);

function syncMode(value: ThemeMode): void {
  _mode = value;
  if (browser) localStorage.setItem(STORAGE_KEY, value);
  const resolved = resolveTheme(value);
  _currentTheme = resolved;
  applyTheme(resolved);
  applyAppearancePack(_appearancePack, resolved);
}

if (browser) {
  const media = window.matchMedia('(prefers-color-scheme: dark)');
  media.addEventListener('change', (event) => {
    if (_mode !== 'system') return;
    const resolved: ResolvedTheme = event.matches ? 'dark' : 'light';
    _currentTheme = resolved;
    applyTheme(resolved);
    applyAppearancePack(_appearancePack, resolved);
  });
}

/** 读取当前主题模式 */
export function getMode(): ThemeMode {
  return _mode;
}

/** 读取当前已解析主题 */
export function getCurrentTheme(): ResolvedTheme {
  return _currentTheme;
}

/** 设置主题模式 */
export function setMode(value: ThemeMode): void {
  syncMode(value);
}

/** 在 light / dark 之间切换（基于当前已解析主题） */
export function toggle(): void {
  syncMode(_currentTheme === 'dark' ? 'light' : 'dark');
}

/** 读取文本阅读器主题 */
export function getTextReaderTheme(): TextReaderThemePreference {
  return { ..._textReaderTheme };
}

/** 整体替换文本阅读器主题 */
export function setTextReaderTheme(value: TextReaderThemePreference): void {
  _textReaderTheme = { ...value };
  persistTextReaderTheme(_textReaderTheme);
}

/** 局部更新文本阅读器主题 */
export function updateTextReaderTheme(patch: Partial<TextReaderThemePreference>): void {
  setTextReaderTheme({ ..._textReaderTheme, ...patch });
}

/** 读取材质透明度 */
export function getMaterialTransparency(): MaterialTransparency {
  return _materialTransparency;
}

/** 设置材质透明度 */
export function setMaterialTransparency(value: MaterialTransparency): void {
  _materialTransparency = value;
  applyMaterialTransparency(value);
  persistMaterialTransparency(value);
}

/**
 * 按辅助偏好应用有效材质透明度，不改写用户存储偏好。
 * 系统 `prefers-reduced-transparency`（或壳层标志）强制实色；
 * 标志清除后 DOM 恢复已存储的用户偏好。
 */
export function syncMaterialTransparencyForA11y(reducedTransparency: boolean): void {
  const effective: MaterialTransparency =
    reducedTransparency || _materialTransparency === 'low' ? 'low' : 'standard';
  applyMaterialTransparency(effective);
}

/** 读取当前 L2 Appearance pack */
export function getAppearancePack(): AppearancePack {
  return {
    id: _appearancePack.id,
    ...(_appearancePack.tokens ? { tokens: { ..._appearancePack.tokens } } : {}),
  };
}

/**
 * 设置 L2 Appearance pack。
 * 接受内置 `inkstone-precision` / `cold-cinnabar`；历史纸灯 id 映射为墨砚；未知 id no-op。
 */
export function setAppearancePack(
  pack: AppearancePack | { id: string; tokens?: AppearancePack['tokens'] },
): void {
  // 未知 id / 历史纸灯 → 默认墨砚；内置冷银朱可切换
  const normalized = normalizeAppearancePackId(pack.id);

  _appearancePack = {
    id: normalized,
    ...(pack.tokens ? { tokens: { ...pack.tokens } } : {}),
  };
  persistAppearancePackId(normalized);
  applyAppearancePack(_appearancePack, _currentTheme);
}
