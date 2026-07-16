import { describe, expect, it } from 'vitest';
import {
  DEFAULT_APPEARANCE_PACK,
  DEFAULT_APPEARANCE_PACK_ID,
  DEFAULT_MATERIAL_TRANSPARENCY,
  DEFAULT_TEXT_READER_THEME,
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

describe('theme preferences', () => {
  it('keeps light, dark, and system mode reflected on the document element', () => {
    setMode('light');
    expect(getMode()).toBe('light');
    expect(getCurrentTheme()).toBe('light');
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(localStorage.getItem('theme')).toBe('light');

    setMode('dark');
    expect(getMode()).toBe('dark');
    expect(getCurrentTheme()).toBe('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(localStorage.getItem('theme')).toBe('dark');

    toggle();
    expect(getMode()).toBe('light');
    expect(getCurrentTheme()).toBe('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(localStorage.getItem('theme')).toBe('light');

    setMode('system');
    expect(getMode()).toBe('system');
    // setup matchMedia defaults to light; explicit mode stays in storage as system
    expect(getCurrentTheme()).toBe('light');
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(localStorage.getItem('theme')).toBe('system');
  });

  it('keeps explicit L0 mode preferred over system preference in storage and resolve path', () => {
    setMode('dark');
    expect(getMode()).toBe('dark');
    expect(getCurrentTheme()).toBe('dark');
    expect(localStorage.getItem('theme')).toBe('dark');

    // Re-assert: explicit dark does not collapse to system; resolve ignores OS path.
    setMode('light');
    expect(getMode()).toBe('light');
    expect(getCurrentTheme()).toBe('light');
    expect(localStorage.getItem('theme')).toBe('light');
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
    expect(DEFAULT_APPEARANCE_PACK_ID).toBe('paper-lantern-precision');
    expect(getAppearancePack()).toEqual(DEFAULT_APPEARANCE_PACK);
    expect(document.documentElement.dataset.appearancePack).toBe('paper-lantern-precision');

    setAppearancePack({ id: 'paper-lantern-precision' });
    expect(getAppearancePack().id).toBe('paper-lantern-precision');
    expect(document.documentElement.dataset.appearancePack).toBe('paper-lantern-precision');
  });

  it('no-ops non-default appearance packs in production', () => {
    setAppearancePack({ id: 'paper-lantern-precision' });
    const before = getAppearancePack();

    // Future multi-pack ids are reserved; production seam ignores them.
    setAppearancePack({ id: 'future-pack' as AppearancePack['id'] });

    expect(getAppearancePack()).toEqual(before);
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
