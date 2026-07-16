// 主题 store：支持 light / dark / system 三种模式
// 使用 Svelte 5 runes（.svelte.ts 中 $state 可用于模块顶层）
import { browser } from '$app/environment';

export type ThemeMode = 'light' | 'dark' | 'system';
export type ResolvedTheme = 'light' | 'dark';
export type MaterialTransparency = 'standard' | 'low';

/** L2 appearance pack id. Production ships only the default pack. */
export type AppearancePackId = 'paper-lantern-precision';

/**
 * L2 Appearance pack seam: rebinds L1 role token values only.
 * Multi-pack UI is out of scope; production set accepts the default pack only.
 */
export type AppearancePack = {
  id: AppearancePackId;
  /** Optional role → value map reserved for future packs. Default pack uses CSS. */
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
const TEXT_READER_THEME_STORAGE_KEY = 'text-reader-theme';
const MATERIAL_TRANSPARENCY_STORAGE_KEY = 'material-transparency';

export const DEFAULT_APPEARANCE_PACK_ID: AppearancePackId = 'paper-lantern-precision';

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

function applyAppearancePack(pack: AppearancePack): void {
  if (!browser) return;
  // Mark pack id only — L1 CSS variable names stay stable; packs rebind values later.
  document.documentElement.dataset.appearancePack = pack.id;
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

// 模块级 $state：单例，全应用共享
const initialMode = readStoredMode();
const initialResolved = resolveTheme(initialMode);
const initialTextReaderTheme = readStoredTextReaderTheme();
const initialMaterialTransparency = readStoredMaterialTransparency();
const initialAppearancePack = DEFAULT_APPEARANCE_PACK;

let _mode = $state<ThemeMode>(initialMode);
let _currentTheme = $state<ResolvedTheme>(initialResolved);
let _textReaderTheme = $state<TextReaderThemePreference>(initialTextReaderTheme);
let _materialTransparency = $state<MaterialTransparency>(initialMaterialTransparency);
let _appearancePack = $state<AppearancePack>(initialAppearancePack);

// 初始应用到 DOM
applyTheme(initialResolved);
applyAppearancePack(initialAppearancePack);
applyMaterialTransparency(initialMaterialTransparency);

// 统一的同步逻辑：持久化、重新解析、应用到 DOM
function syncMode(value: ThemeMode): void {
  _mode = value;
  if (browser) localStorage.setItem(STORAGE_KEY, value);
  const resolved = resolveTheme(value);
  _currentTheme = resolved;
  applyTheme(resolved);
}

// 监听系统主题变化，仅在 system 模式下同步 currentTheme 与 DOM
if (browser) {
  const media = window.matchMedia('(prefers-color-scheme: dark)');
  media.addEventListener('change', (event) => {
    if (_mode !== 'system') return;
    const resolved: ResolvedTheme = event.matches ? 'dark' : 'light';
    _currentTheme = resolved;
    applyTheme(resolved);
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
 * Apply effective material transparency for a11y without rewriting user preference.
 * System `prefers-reduced-transparency` (or explicit shell flag) forces solid surfaces;
 * when the flag clears, DOM restores the stored user preference.
 */
export function syncMaterialTransparencyForA11y(reducedTransparency: boolean): void {
  const effective: MaterialTransparency =
    reducedTransparency || _materialTransparency === 'low' ? 'low' : 'standard';
  applyMaterialTransparency(effective);
}

/** 读取当前 L2 Appearance pack（生产仅默认包） */
export function getAppearancePack(): AppearancePack {
  return {
    id: _appearancePack.id,
    ...(_appearancePack.tokens ? { tokens: { ..._appearancePack.tokens } } : {}),
  };
}

/**
 * 设置 L2 Appearance pack。
 * 生产仅接受默认包 `paper-lantern-precision`；其它 id 为 no-op，预留给后续多包任务。
 */
export function setAppearancePack(pack: AppearancePack): void {
  if (pack.id !== DEFAULT_APPEARANCE_PACK_ID) return;
  _appearancePack = {
    id: pack.id,
    ...(pack.tokens ? { tokens: { ...pack.tokens } } : {}),
  };
  applyAppearancePack(_appearancePack);
}
