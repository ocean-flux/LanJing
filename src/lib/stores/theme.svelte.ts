// 主题 store：支持 light / dark / system 三种模式
// 使用 Svelte 5 runes（.svelte.ts 中 $state 可用于模块顶层）
import { browser } from '$app/environment';

export type ThemeMode = 'light' | 'dark' | 'system';
export type ResolvedTheme = 'light' | 'dark';

const STORAGE_KEY = 'theme';

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
  document.documentElement.classList.toggle('dark', theme === 'dark');
}

// 模块级 $state：单例，全应用共享
const initialMode = readStoredMode();
const initialResolved = resolveTheme(initialMode);

let _mode = $state<ThemeMode>(initialMode);
let _currentTheme = $state<ResolvedTheme>(initialResolved);

// 初始应用到 DOM
applyTheme(initialResolved);

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
