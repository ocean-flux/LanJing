import { describe, expect, it } from 'vitest';
import {
  DEFAULT_APPEARANCE_PACK_ID,
  DEFAULT_MATERIAL_TRANSPARENCY,
  DEFAULT_TEXT_READER_THEME,
  WEB_PREFERENCES_STORAGE_KEY,
  getAppearancePack,
  getCurrentTheme,
  getDarkThemeId,
  getLightThemeId,
  getMaterialTransparency,
  getMode,
  getTextReaderTheme,
  resolveThemeIdForFace,
  setAppearancePack,
  setDarkThemeId,
  setLightThemeId,
  setMaterialTransparency,
  setMode,
  setTextReaderTheme,
  syncMaterialTransparencyForA11y,
  toggle,
  updateTextReaderTheme,
  type AppearancePack,
} from './theme.svelte';
import { BUILTIN_APPEARANCE_PACK_IDS } from './appearance-packs';

function readWebPrefs(): {
  mode?: string;
  appearancePackId?: string;
  lightThemeId?: string;
  darkThemeId?: string;
} {
  const raw = localStorage.getItem(WEB_PREFERENCES_STORAGE_KEY);
  if (!raw) return {};
  try {
    return JSON.parse(raw) as {
      mode?: string;
      appearancePackId?: string;
      lightThemeId?: string;
      darkThemeId?: string;
    };
  } catch {
    return {};
  }
}

function relativeLuminance(hex: string): number {
  const channels = hex.match(/[a-f\d]{2}/gi);
  if (!channels || channels.length !== 3) {
    throw new Error(`Expected six-digit hex color, received ${hex}`);
  }

  const [red, green, blue] = channels.map((channel) => {
    const normalized = Number.parseInt(channel, 16) / 255;
    return normalized <= 0.04045 ? normalized / 12.92 : Math.pow((normalized + 0.055) / 1.055, 2.4);
  });

  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

function contrastRatio(first: string, second: string): number {
  const [lighter, darker] = [relativeLuminance(first), relativeLuminance(second)].sort(
    (left, right) => right - left,
  );
  return (lighter + 0.05) / (darker + 0.05);
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

  it('keeps primary action text at AA contrast across every theme face', () => {
    for (const themeId of BUILTIN_APPEARANCE_PACK_IDS) {
      for (const face of ['light', 'dark'] as const) {
        setMode(face);
        if (face === 'light') setLightThemeId(themeId);
        else setDarkThemeId(themeId);

        const root = document.documentElement;
        const primary = root.style.getPropertyValue('--lantern-strong').trim();
        const onPrimary = root.style.getPropertyValue('--on-lantern').trim();

        expect(contrastRatio(primary, onPrimary), `${themeId}/${face}`).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  it('allows independent light and dark theme tracks', () => {
    setLightThemeId('inkstone-precision');
    setDarkThemeId('cold-cinnabar');
    expect(getLightThemeId()).toBe('inkstone-precision');
    expect(getDarkThemeId()).toBe('cold-cinnabar');

    setMode('light');
    expect(getAppearancePack().id).toBe('inkstone-precision');
    expect(document.documentElement.dataset.appearancePack).toBe('inkstone-precision');
    expect(document.documentElement.style.getPropertyValue('--lantern').trim()).toBe('#2a6f7a');

    setMode('dark');
    expect(getAppearancePack().id).toBe('cold-cinnabar');
    expect(document.documentElement.dataset.appearancePack).toBe('cold-cinnabar');
    // 冷银朱暗面 lantern 为手搓值，非亮面反相
    expect(document.documentElement.style.getPropertyValue('--lantern').trim()).toBe('#d4785a');

    const stored = readWebPrefs();
    expect(stored.lightThemeId).toBe('inkstone-precision');
    expect(stored.darkThemeId).toBe('cold-cinnabar');
  });

  it('resolves theme id for face without mixing tracks', () => {
    expect(resolveThemeIdForFace('light', 'inkstone-precision', 'cold-cinnabar')).toBe(
      'inkstone-precision',
    );
    expect(resolveThemeIdForFace('dark', 'inkstone-precision', 'cold-cinnabar')).toBe(
      'cold-cinnabar',
    );
  });

  it('migrates legacy single appearancePackId to both tracks', () => {
    localStorage.setItem(
      WEB_PREFERENCES_STORAGE_KEY,
      JSON.stringify({ mode: 'light', appearancePackId: 'cold-cinnabar' }),
    );
    // 通过 setAppearancePack 兼容路径验证双轨同值
    setAppearancePack({ id: 'cold-cinnabar' });
    expect(getLightThemeId()).toBe('cold-cinnabar');
    expect(getDarkThemeId()).toBe('cold-cinnabar');
    expect(readWebPrefs().lightThemeId).toBe('cold-cinnabar');
    expect(readWebPrefs().darkThemeId).toBe('cold-cinnabar');
  });

  it('switches to cold-cinnabar builtin pack and maps legacy paper-lantern id', () => {
    setMode('light');
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
