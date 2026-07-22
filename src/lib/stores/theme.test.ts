import { describe, expect, it, vi } from 'vitest';
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
import {
  BUILTIN_APPEARANCE_PACK_IDS,
  THEME_REGISTRY,
  normalizeAppearancePackId,
} from './appearance-packs';

const tauriBoundary = vi.hoisted(() => {
  type PersistedState = Record<string, unknown>;
  type BeforeFrontendSync = (state: PersistedState) => PersistedState;

  class MockRuneStore {
    state: PersistedState;
    private readonly beforeFrontendSync?: BeforeFrontendSync;

    constructor(
      _id: string,
      state: PersistedState,
      options?: { hooks?: { beforeFrontendSync?: BeforeFrontendSync } },
    ) {
      this.state = { ...state };
      this.beforeFrontendSync = options?.hooks?.beforeFrontendSync;
      tauriBoundary.instance = this;
    }

    async start(): Promise<void> {
      this.patchFrontend(tauriBoundary.persistedState);
    }

    patchFrontend(state: PersistedState): void {
      const next = this.beforeFrontendSync?.({ ...state }) ?? state;
      Object.assign(this.state, next);
    }
  }

  return {
    enabled: false,
    persistedState: {} as PersistedState,
    instance: null as MockRuneStore | null,
    MockRuneStore,
  };
});

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => tauriBoundary.enabled }));
vi.mock('@tauri-store/svelte', () => ({ RuneStore: tauriBoundary.MockRuneStore }));

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
    expect(DEFAULT_APPEARANCE_PACK_ID).toBe('obsidian-void');
    // system 默认随环境；强制 dark 断言暗轨默认
    setMode('dark');
    expect(getAppearancePack().id).toBe('obsidian-void');
    expect(document.documentElement.dataset.appearancePack).toBe('obsidian-void');

    setAppearancePack({ id: 'obsidian-void' });
    setMode('dark');
    expect(getAppearancePack().id).toBe('obsidian-void');
    expect(document.documentElement.dataset.appearancePack).toBe('obsidian-void');
  });

  it('keeps primary action text at AA contrast across every theme face', () => {
    for (const themeId of BUILTIN_APPEARANCE_PACK_IDS) {
      const face = THEME_REGISTRY[themeId].face;
      setMode(face);
      if (face === 'light') setLightThemeId(themeId);
      else setDarkThemeId(themeId);

      const root = document.documentElement;
      const primary = root.style.getPropertyValue('--lantern-strong').trim();
      const onPrimary = root.style.getPropertyValue('--on-lantern').trim();

      expect(contrastRatio(primary, onPrimary), `${themeId}/${face}`).toBeGreaterThanOrEqual(4.5);
    }
  });

  it('normalizes historical ids to a legal theme on each face', () => {
    expect(normalizeAppearancePackId('inkstone-precision', 'light')).toBe('porcelain-day');
    expect(normalizeAppearancePackId('inkstone-precision', 'dark')).toBe('obsidian-void');
    expect(normalizeAppearancePackId('cold-cinnabar', 'light')).toBe('mist-studio');
    expect(normalizeAppearancePackId('cold-cinnabar', 'dark')).toBe('graphite-atelier');
    expect(normalizeAppearancePackId('paper-lantern-classic', 'light')).toBe('porcelain-day');
    expect(normalizeAppearancePackId('paper-lantern-classic', 'dark')).toBe('obsidian-void');
  });

  it('allows independent light and dark theme tracks', () => {
    setLightThemeId('porcelain-day');
    setDarkThemeId('graphite-atelier');
    expect(getLightThemeId()).toBe('porcelain-day');
    expect(getDarkThemeId()).toBe('graphite-atelier');

    setMode('light');
    expect(getAppearancePack().id).toBe('porcelain-day');
    expect(document.documentElement.dataset.appearancePack).toBe('porcelain-day');
    expect(document.documentElement.style.getPropertyValue('--lantern').trim()).toBe('#0f6e7a');

    setMode('dark');
    expect(getAppearancePack().id).toBe('graphite-atelier');
    expect(document.documentElement.dataset.appearancePack).toBe('graphite-atelier');
    // 钢蓝暗面 lantern，非亮面反相
    expect(document.documentElement.style.getPropertyValue('--lantern').trim()).toBe('#5b9fd4');

    const stored = readWebPrefs();
    expect(stored.lightThemeId).toBe('porcelain-day');
    expect(stored.darkThemeId).toBe('graphite-atelier');
  });

  it('resolves theme id for face without mixing tracks', () => {
    expect(resolveThemeIdForFace('light', 'porcelain-day', 'graphite-atelier')).toBe(
      'porcelain-day',
    );
    expect(resolveThemeIdForFace('dark', 'porcelain-day', 'graphite-atelier')).toBe(
      'graphite-atelier',
    );
  });

  it('migrates a legacy localStorage pack to both face-specific tracks', async () => {
    localStorage.removeItem(WEB_PREFERENCES_STORAGE_KEY);
    localStorage.setItem('appearance-pack', 'cold-cinnabar');
    // 该断言覆盖模块初始化边界，必须清缓存后重新执行初始化读取。
    vi.resetModules();

    const migratedTheme = await import('./theme.svelte');
    const lightThemeId = migratedTheme.getLightThemeId();
    const darkThemeId = migratedTheme.getDarkThemeId();
    localStorage.removeItem('appearance-pack');

    expect(lightThemeId).toBe('mist-studio');
    expect(darkThemeId).toBe('graphite-atelier');
  });

  it('persists the active compatibility id when system color scheme changes', async () => {
    const originalMatchMedia = window.matchMedia;
    let matches = false;
    let listener: ((event: MediaQueryListEvent) => void) | undefined;
    const mediaQuery = {
      get matches() {
        return matches;
      },
      media: '(prefers-color-scheme: dark)',
      addEventListener: (_type: string, callback: (event: MediaQueryListEvent) => void) => {
        listener = callback;
      },
      removeEventListener: () => undefined,
    } as unknown as MediaQueryList;
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      writable: true,
      value: () => mediaQuery,
    });

    try {
      tauriBoundary.enabled = false;
      localStorage.removeItem(WEB_PREFERENCES_STORAGE_KEY);
      vi.resetModules();
      // 该断言需要重新注册到可触发的 MediaQueryList 测试边界。
      const freshTheme = await import('./theme.svelte');
      freshTheme.setLightThemeId('mist-studio');
      freshTheme.setDarkThemeId('graphite-atelier');
      freshTheme.setMode('system');

      matches = true;
      listener?.({ matches: true, media: mediaQuery.media } as MediaQueryListEvent);

      expect(freshTheme.getCurrentTheme()).toBe('dark');
      expect(readWebPrefs().appearancePackId).toBe('graphite-atelier');
    } finally {
      Object.defineProperty(window, 'matchMedia', {
        configurable: true,
        writable: true,
        value: originalMatchMedia,
      });
    }
  });

  it('migrates legacy RuneStore state and applies later backend patches', async () => {
    localStorage.removeItem(WEB_PREFERENCES_STORAGE_KEY);
    tauriBoundary.enabled = true;
    tauriBoundary.persistedState = {
      mode: 'light',
      appearancePackId: 'cold-cinnabar',
    };
    vi.resetModules();

    try {
      // 该断言需要重新执行 Tauri-only 动态加载和 RuneStore start 边界。
      const freshTheme = await import('./theme.svelte');
      await freshTheme.startThemePreferences();

      expect(freshTheme.getLightThemeId()).toBe('mist-studio');
      expect(freshTheme.getDarkThemeId()).toBe('graphite-atelier');
      expect(tauriBoundary.instance?.state.lightThemeId).toBe('mist-studio');
      expect(tauriBoundary.instance?.state.darkThemeId).toBe('graphite-atelier');
      expect(tauriBoundary.instance?.state.appearancePackId).toBe('mist-studio');

      tauriBoundary.instance?.patchFrontend({
        mode: 'dark',
        lightThemeId: 'porcelain-day',
        darkThemeId: 'graphite-atelier',
        appearancePackId: 'graphite-atelier',
      });

      expect(freshTheme.getCurrentTheme()).toBe('dark');
      expect(freshTheme.getAppearancePack().id).toBe('graphite-atelier');
      expect(document.documentElement.dataset.appearancePack).toBe('graphite-atelier');
    } finally {
      tauriBoundary.enabled = false;
      tauriBoundary.persistedState = {};
      tauriBoundary.instance = null;
    }
  });

  it('maps legacy cinnabar and paper-lantern ids without orange', () => {
    setMode('light');
    setAppearancePack({ id: 'cold-cinnabar' });
    // 亮面解析为 mist-studio
    expect(getAppearancePack().id).toBe('mist-studio');
    expect(document.documentElement.dataset.appearancePack).toBe('mist-studio');
    expect(document.documentElement.style.getPropertyValue('--lantern').trim()).toBe('#0e7490');

    setAppearancePack({ id: 'paper-lantern-precision' as AppearancePack['id'] });
    setMode('light');
    expect(getAppearancePack().id).toBe('porcelain-day');
    expect(document.documentElement.dataset.appearancePack).toBe('porcelain-day');
  });

  it('maps unknown appearance packs to face defaults', () => {
    setMode('dark');
    setAppearancePack({ id: 'future-pack' as AppearancePack['id'] });

    expect(getDarkThemeId()).toBe('obsidian-void');
    expect(getLightThemeId()).toBe('porcelain-day');
    expect(getAppearancePack().id).toBe('obsidian-void');
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
