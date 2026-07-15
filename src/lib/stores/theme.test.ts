import { describe, expect, it } from 'vitest';
import {
  DEFAULT_MATERIAL_TRANSPARENCY,
  DEFAULT_TEXT_READER_THEME,
  getCurrentTheme,
  getMaterialTransparency,
  getMode,
  getTextReaderTheme,
  setMaterialTransparency,
  setMode,
  setTextReaderTheme,
  toggle,
  updateTextReaderTheme,
} from './theme.svelte';

describe('theme preferences', () => {
  it('keeps light, dark, and system mode reflected on the document element', () => {
    setMode('light');
    expect(getMode()).toBe('light');
    expect(getCurrentTheme()).toBe('light');
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);

    setMode('dark');
    expect(getMode()).toBe('dark');
    expect(getCurrentTheme()).toBe('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);

    toggle();
    expect(getMode()).toBe('light');
    expect(getCurrentTheme()).toBe('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);

    setMode('system');
    expect(getMode()).toBe('system');
    expect(getCurrentTheme()).toBe('light');
    expect(document.documentElement.dataset.theme).toBe('light');
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
});
