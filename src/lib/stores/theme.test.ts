import { describe, expect, it } from 'vitest';
import {
  DEFAULT_APPEARANCE_PACK_ID,
  DEFAULT_MATERIAL_TRANSPARENCY,
  DEFAULT_TEXT_READER_THEME,
  WEB_PREFERENCES_STORAGE_KEY,
  getAppearancePack,
  getCurrentTheme,
  getMaterialTransparency,
  getMode,
  getTextReaderTheme,
  setAppearancePack,
  setMaterialTransparency,
  setMode,
  setTextReaderTheme,
  syncMaterialTransparencyForA11y,
  toggle,
  updateTextReaderTheme,
  type AppearancePack,
} from './theme.svelte';

function readWebPrefs(): { mode?: string; appearancePackId?: string } {
  const raw = localStorage.getItem(WEB_PREFERENCES_STORAGE_KEY);
  if (!raw) return {};
  try {
    return JSON.parse(raw) as { mode?: string; appearancePackId?: string };
  } catch {
    return {};
  }
}

describe('theme preferences', () => {
  it('keeps light, dark, and system mode reflected on the document element', () => {
    setMode('light');
    expect(getMode()).toBe('light');
    expect(getCurrentTheme()).toBe('light');
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(readWebPrefs().mode).toBe('light');

    setMode('dark');
    expect(getMode()).toBe('dark');
    expect(getCurrentTheme()).toBe('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(readWebPrefs().mode).toBe('dark');

    toggle();
    expect(getMode()).toBe('light');
    expect(getCurrentTheme()).toBe('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(readWebPrefs().mode).toBe('light');

    setMode('system');
    expect(getMode()).toBe('system');
    // setup 的 matchMedia 默认 light；显式 mode 在 storage 仍为 system
    expect(getCurrentTheme()).toBe('light');
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(readWebPrefs().mode).toBe('system');
  });

  it('keeps explicit L0 mode preferred over system preference in storage and resolve path', () => {
    setMode('dark');
    expect(getMode()).toBe('dark');
    expect(getCurrentTheme()).toBe('dark');
    expect(readWebPrefs().mode).toBe('dark');

    // 再断言：显式 dark 不塌成 system；resolve 不走 OS 路径。
    setMode('light');
    expect(getMode()).toBe('light');
    expect(getCurrentTheme()).toBe('light');
    expect(readWebPrefs().mode).toBe('light');
    expect(getMode()).not.toBe('system');
  });

  it('keeps text reader defaults readable and independent', () => {
    expect(DEFAULT_TEXT_READER_THEME).toMatchObject({
      colorScheme: 'paper',
      fontFamily: 'serif',
      fontSize: 18,
      lineHeight: 1.75,
      pageMode: 'scroll',
      indentFirstLine: false,
    });
  });

  it('replaces and patches text reader theme', () => {
    setTextReaderTheme(DEFAULT_TEXT_READER_THEME);
    updateTextReaderTheme({ colorScheme: 'dark', fontSize: 20 });

    expect(getTextReaderTheme()).toMatchObject({
      ...DEFAULT_TEXT_READER_THEME,
      colorScheme: 'dark',
      fontSize: 20,
    });
  });

  it('keeps text reader theme when L0 app mode changes', () => {
    setTextReaderTheme({
      ...DEFAULT_TEXT_READER_THEME,
      colorScheme: 'black',
      fontSize: 22,
      pageMode: 'paged',
    });

    setMode('light');
    setMode('dark');
    setMode('system');

    expect(getTextReaderTheme()).toMatchObject({
      colorScheme: 'black',
      fontSize: 22,
      pageMode: 'paged',
      fontFamily: 'serif',
    });
  });

  it('marks default L2 appearance pack on the document element', () => {
    expect(DEFAULT_APPEARANCE_PACK_ID).toBe('inkstone-precision');
    expect(getAppearancePack().id).toBe('inkstone-precision');
    expect(document.documentElement.dataset.appearancePack).toBe('inkstone-precision');

    setAppearancePack({ id: 'inkstone-precision' });
    expect(getAppearancePack().id).toBe('inkstone-precision');
    expect(document.documentElement.dataset.appearancePack).toBe('inkstone-precision');
  });

  it('switches to cold-cinnabar builtin pack and maps legacy paper-lantern id', () => {
    setAppearancePack({ id: 'cold-cinnabar' });
    expect(getAppearancePack().id).toBe('cold-cinnabar');
    expect(document.documentElement.dataset.appearancePack).toBe('cold-cinnabar');
    expect(document.documentElement.style.getPropertyValue('--lantern').trim()).toBe('#c45a3c');

    setAppearancePack({ id: 'paper-lantern-precision' as AppearancePack['id'] });
    expect(getAppearancePack().id).toBe('inkstone-precision');
    expect(document.documentElement.dataset.appearancePack).toBe('inkstone-precision');
  });

  it('maps unknown appearance packs to default inkstone', () => {
    setAppearancePack({ id: 'inkstone-precision' });
    setAppearancePack({ id: 'future-pack' as AppearancePack['id'] });

    expect(getAppearancePack().id).toBe('inkstone-precision');
    expect(document.documentElement.dataset.appearancePack).toBe(DEFAULT_APPEARANCE_PACK_ID);
  });

  it('stores material transparency as a narrow two-option preference', () => {
    expect(DEFAULT_MATERIAL_TRANSPARENCY).toBe('standard');

    setMaterialTransparency('low');
    expect(getMaterialTransparency()).toBe('low');
    expect(document.documentElement.dataset.materialTransparency).toBe('low');
    expect(document.documentElement.classList.contains('low-transparency')).toBe(true);

    setMaterialTransparency('standard');
    expect(getMaterialTransparency()).toBe('standard');
    expect(document.documentElement.dataset.materialTransparency).toBe('standard');
    expect(document.documentElement.classList.contains('low-transparency')).toBe(false);
  });

  it('forces solid material for reduced transparency without rewriting stored preference', () => {
    setMaterialTransparency('standard');
    expect(getMaterialTransparency()).toBe('standard');

    syncMaterialTransparencyForA11y(true);
    expect(getMaterialTransparency()).toBe('standard');
    expect(document.documentElement.dataset.materialTransparency).toBe('low');
    expect(document.documentElement.classList.contains('low-transparency')).toBe(true);

    syncMaterialTransparencyForA11y(false);
    expect(getMaterialTransparency()).toBe('standard');
    expect(document.documentElement.dataset.materialTransparency).toBe('standard');
    expect(document.documentElement.classList.contains('low-transparency')).toBe(false);
  });

  it('keeps low material effective when a11y flag clears but user chose low', () => {
    setMaterialTransparency('low');
    syncMaterialTransparencyForA11y(true);
    syncMaterialTransparencyForA11y(false);

    expect(getMaterialTransparency()).toBe('low');
    expect(document.documentElement.dataset.materialTransparency).toBe('low');
  });
});
