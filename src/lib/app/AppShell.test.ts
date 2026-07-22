import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getMaterialTransparency, setMaterialTransparency } from '$lib/stores/theme.svelte';
import AppShell from './AppShell.svelte';
import { COLD_LAUNCH_SESSION_KEY, COLD_LAUNCH_THRESHOLD_MS } from './cold-launch';
import { RAIL_BEHAVIOR_STORAGE_KEY, RAIL_COLLAPSED_STORAGE_KEY } from './shell-rail-preference';
import type { ModeShellContract, PlatformCapabilities } from './shell-types';

function desktopPlatform(overrides: Partial<PlatformCapabilities> = {}): PlatformCapabilities {
  return {
    kind: 'browser',
    orientation: 'landscape',
    viewportWidth: 1280,
    viewportHeight: 800,
    hover: 'hover',
    pointer: 'fine',
    keyboard: true,
    touch: false,
    windowControls: 'browser-preview',
    ...overrides,
  };
}

function mobilePlatform(overrides: Partial<PlatformCapabilities> = {}): PlatformCapabilities {
  return {
    kind: 'android',
    orientation: 'portrait',
    viewportWidth: 390,
    viewportHeight: 844,
    hover: 'none',
    pointer: 'coarse',
    keyboard: false,
    touch: true,
    windowControls: 'browser-preview',
    ...overrides,
  };
}

function makeShell(overrides: Partial<ModeShellContract> = {}): ModeShellContract {
  return {
    productContext: 'realm',
    mediaSpace: null,
    foregroundActivity: { kind: 'browse', id: 'realm' },
    presentation: 'normal',
    platform: desktopPlatform(),
    theme: {
      mode: 'system',
      appearancePack: 'obsidian-void',
      reducedMotion: false,
      reducedTransparency: false,
    },
    ambientAudio: null,
    ...overrides,
  };
}

function primaryNavs() {
  return {
    rail: screen.queryByRole('navigation', { name: '主导航' }),
    bottom: screen.queryByRole('navigation', { name: '底部主导航' }),
  };
}

function stubPerformanceNow(ms: number) {
  vi.spyOn(performance, 'now').mockReturnValue(ms);
}

beforeEach(() => {
  // 默认：快速冷启动，纯 chrome 用例不闪启动层。
  stubPerformanceNow(0);
  setMaterialTransparency('standard');
});

afterEach(() => {
  vi.restoreAllMocks();
  sessionStorage.removeItem(COLD_LAUNCH_SESSION_KEY);
  localStorage.removeItem(RAIL_COLLAPSED_STORAGE_KEY);
  localStorage.removeItem(RAIL_BEHAVIOR_STORAGE_KEY);
  setMaterialTransparency('standard');
  window.history.replaceState({}, '', '/');
});

describe('AppShell', () => {
  it('renders glass island desktop rail and minimal titlebar', async () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          platform: desktopPlatform({ windowControls: 'browser-preview' }),
        }),
      },
    });

    const root = screen.getByTestId('mode-shell');
    expect(root.getAttribute('data-theme-mode')).toBe('system');
    expect(root.getAttribute('data-appearance-pack')).toBe('obsidian-void');
    expect(root.getAttribute('data-chrome-family')).toBe('rail');

    const nav = screen.getByRole('navigation', { name: '主导航' });
    expect(nav.getAttribute('data-shell-rail')).toBe('spine');
    expect(nav.getAttribute('data-shell-island')).toBe('rail');
    expect(screen.queryByRole('navigation', { name: '底部主导航' })).toBeNull();
    expect(screen.getByRole('link', { name: '境场' })).toBeTruthy();
    expect(screen.getByRole('link', { name: '打开设置' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '打开快捷操作' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: '打开全局搜索' })).toBeNull();
    expect(document.querySelector('[data-mini-player]')).toBeNull();
    expect(document.querySelector('[data-shell-mesh]')).toBeInstanceOf(HTMLElement);
    expect(root.getAttribute('data-rail-behavior')).toBe('fixed');

    expect(screen.getByRole('banner')).toBeTruthy();
    expect(document.querySelectorAll('[data-tauri-drag-region]').length).toBeGreaterThan(0);
    expect(screen.getByRole('button', { name: '最小化窗口' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '最大化或还原窗口' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '关闭窗口' })).toBeTruthy();

    const expandTrigger = screen.getByRole('button', { name: '打开快捷操作' });
    await fireEvent.click(expandTrigger);
    expect(screen.getByRole('dialog', { name: '快捷操作' })).toBeTruthy();
    expect(screen.getByRole('link', { name: '外观' }).getAttribute('href')).toContain(
      '/settings#appearance',
    );
    expect(document.activeElement).toBe(screen.getByRole('button', { name: '关闭快捷操作' }));

    await fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('dialog', { name: '快捷操作' })).toBeNull();
    expect(document.activeElement).toBe(expandTrigger);
  });

  it('keeps mobile bottom island accessible without mini-player chrome', () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          platform: mobilePlatform(),
        }),
      },
    });

    expect(screen.queryByRole('navigation', { name: '主导航' })).toBeNull();
    expect(screen.queryByRole('banner')).toBeNull();
    const bottomNav = screen.getByRole('navigation', { name: '底部主导航' });
    expect(bottomNav.getAttribute('data-bottom-nav')).toBe('visible');
    expect(bottomNav.getAttribute('data-shell-island')).toBe('bottom');
    expect(bottomNav.className).not.toContain('md:hidden');
    expect(bottomNav.className).not.toContain('border-t');
    expect(screen.getByRole('link', { name: '境场' })).toBeTruthy();
    expect(screen.getByRole('link', { name: '应用' })).toBeTruthy();
    expect(screen.getByRole('link', { name: '来源' })).toBeTruthy();
    expect(screen.getByRole('link', { name: '资料库' })).toBeTruthy();

    const settingsLink = screen.getByRole('link', { name: '打开设置' });
    expect(settingsLink.closest('[data-mobile-toolbar]')).toBeTruthy();
    expect(screen.getByRole('button', { name: '打开快捷操作' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: '打开全局搜索' })).toBeNull();
    expect(bottomNav.querySelectorAll('a')).toHaveLength(4);
    expect(document.querySelector('[data-mini-player]')).toBeNull();
    expect(document.querySelector('[data-mobile-toolbar][data-tauri-drag-region]')).toBeTruthy();
    const main = screen.getByRole('main');
    expect(main.className).toContain('flex-1');
    expect(main.parentElement?.className).toContain('flex-col');
  });

  it('opens mobile Island Expand and traps focus', async () => {
    render(AppShell, { props: { shell: makeShell({ platform: mobilePlatform() }) } });
    const trigger = screen.getByRole('button', { name: '打开快捷操作' });

    await fireEvent.click(trigger);
    const dialog = screen.getByRole('dialog', { name: '快捷操作' });
    expect(dialog).toBeTruthy();
    const closeButton = screen.getByRole('button', { name: '关闭快捷操作' });
    expect(document.activeElement).toBe(closeButton);

    const links = screen.getAllByRole('link');
    const lastLink = links[links.length - 1];
    lastLink.focus();
    await fireEvent.keyDown(document, { key: 'Tab' });
    expect(document.activeElement).toBe(closeButton);

    await fireEvent.click(closeButton);
    expect(screen.queryByRole('dialog', { name: '快捷操作' })).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it('does not mount mini-player even when ambient audio session exists', () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          ambientAudio: {
            id: 'ambient-1',
            state: 'paused',
            focus: 'ambient',
            label: '雨声',
          },
        }),
      },
    });

    expect(screen.queryByRole('button', { name: '当前播放' })).toBeNull();
    expect(screen.queryByText('雨声')).toBeNull();
    expect(document.querySelector('[data-mini-player]')).toBeNull();
    // ambient 仍暴露在壳 data 属性，供后续媒体任务接线
    expect(screen.getByTestId('mode-shell').getAttribute('data-ambient-audio')).toBe('paused');
  });

  it('toggles rail behavior with popup focus and Escape restoration', async () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          platform: desktopPlatform({ windowControls: 'browser-preview' }),
          theme: {
            mode: 'system',
            appearancePack: 'obsidian-void',
            reducedMotion: true,
            reducedTransparency: false,
          },
        }),
      },
    });

    const root = screen.getByTestId('mode-shell');
    const optionsTrigger = screen.getByRole('button', { name: '侧栏选项' });
    expect(root.getAttribute('data-rail-behavior')).toBe('fixed');

    await fireEvent.click(optionsTrigger);
    const fixedOption = screen.getByRole('button', { name: '固定显示' });
    expect(document.activeElement).toBe(fixedOption);
    expect(screen.getByRole('button', { name: '收起侧栏' })).toBeTruthy();

    await fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('dialog', { name: '侧栏选项' })).toBeNull();
    expect(document.activeElement).toBe(optionsTrigger);

    await fireEvent.click(optionsTrigger);
    await fireEvent.click(screen.getByRole('button', { name: '悬停展开' }));
    expect(root.getAttribute('data-rail-behavior')).toBe('hover');
    expect(localStorage.getItem(RAIL_BEHAVIOR_STORAGE_KEY)).toBe('hover');
    expect(root.getAttribute('data-rail-collapsed')).toBe('true');

    const zone = document.querySelector('[data-shell-rail-zone]');
    expect(zone).toBeInstanceOf(HTMLElement);
    await fireEvent.pointerEnter(zone!);
    expect(root.getAttribute('data-rail-collapsed')).toBe('false');

    await fireEvent.click(screen.getByRole('button', { name: '侧栏选项' }));
    await fireEvent.click(screen.getByRole('button', { name: '固定显示' }));
    expect(root.getAttribute('data-rail-behavior')).toBe('fixed');
    expect(localStorage.getItem(RAIL_BEHAVIOR_STORAGE_KEY)).toBe('fixed');
  });

  it('clears pending hover timers when rail hides and on unmount', async () => {
    vi.useFakeTimers();
    try {
      localStorage.setItem(RAIL_BEHAVIOR_STORAGE_KEY, 'hover');
      const desktopShell = makeShell({
        platform: desktopPlatform(),
        theme: {
          mode: 'system',
          appearancePack: 'obsidian-void',
          reducedMotion: false,
          reducedTransparency: false,
        },
      });
      const { rerender, unmount } = render(AppShell, { props: { shell: desktopShell } });

      await fireEvent.pointerEnter(document.querySelector('[data-shell-rail-zone]')!);
      const baseline = vi.getTimerCount();
      expect(baseline).toBeGreaterThan(0);

      rerender({ shell: { ...desktopShell, platform: mobilePlatform() } });
      await Promise.resolve();
      expect(vi.getTimerCount()).toBeLessThan(baseline);
      const harnessTimerCount = vi.getTimerCount();

      rerender({ shell: desktopShell });
      await fireEvent.pointerEnter(document.querySelector('[data-shell-rail-zone]')!);
      unmount();
      expect(vi.getTimerCount()).toBe(harnessTimerCount);
    } finally {
      vi.useRealTimers();
    }
  });

  it('never shows rail and bottom primary nav together', () => {
    const cases: PlatformCapabilities[] = [
      desktopPlatform(),
      mobilePlatform(),
      desktopPlatform({
        viewportWidth: 1100,
        hover: 'hover',
        pointer: 'fine',
      }),
      mobilePlatform({
        viewportWidth: 900,
        viewportHeight: 1200,
        orientation: 'portrait',
      }),
      mobilePlatform({
        viewportWidth: 1100,
        viewportHeight: 700,
        orientation: 'landscape',
      }),
    ];

    for (const platform of cases) {
      const { unmount } = render(AppShell, {
        props: { shell: makeShell({ platform }) },
      });
      const { rail, bottom } = primaryNavs();
      expect(Boolean(rail) && Boolean(bottom)).toBe(false);
      unmount();
    }
  });

  it('keeps window controls persistent and respects native modes', () => {
    const base = makeShell({
      platform: desktopPlatform({
        kind: 'windows',
        windowControls: 'windows-overlay',
      }),
    });
    const { rerender } = render(AppShell, { props: { shell: base } });

    const titlebar = screen.getByRole('banner');
    expect(titlebar.getAttribute('data-titlebar-controls')).toBe('html');
    expect(document.querySelector('[data-shell-rail-window-controls]')).toBeNull();
    expect(screen.getByRole('button', { name: '最小化窗口' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '最大化或还原窗口' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '关闭窗口' })).toBeTruthy();

    rerender({
      shell: {
        ...base,
        platform: desktopPlatform({ windowControls: 'system-decorated' }),
      },
    });
    expect(screen.getByRole('banner').getAttribute('data-titlebar-controls')).toBe('native');
    expect(screen.queryByRole('button', { name: '最小化窗口' })).toBeNull();

    rerender({
      shell: {
        ...base,
        platform: desktopPlatform({ kind: 'macos', windowControls: 'macos-overlay' }),
      },
    });
    expect(screen.getByRole('banner').getAttribute('data-native-window-controls')).toBe(
      'macos-overlay',
    );
    expect(document.querySelector('[data-macos-traffic-light-safe="true"]')).toBeTruthy();
    expect(screen.queryByRole('button', { name: '关闭窗口' })).toBeNull();
  });

  it('keeps productContext when platform orientation/width changes navigation family', () => {
    const base = makeShell({
      productContext: 'library',
      foregroundActivity: { kind: 'browse', id: 'library' },
      platform: mobilePlatform({
        orientation: 'portrait',
        viewportWidth: 390,
        viewportHeight: 844,
      }),
    });

    const { rerender } = render(AppShell, { props: { shell: base } });
    const root = screen.getByTestId('mode-shell');
    expect(root.getAttribute('data-product-context')).toBe('library');
    expect(root.getAttribute('data-shell-mode')).toBe('mobile');
    expect(primaryNavs().bottom).toBeTruthy();
    expect(primaryNavs().rail).toBeNull();

    rerender({
      shell: {
        ...base,
        platform: desktopPlatform({
          kind: 'windows',
          orientation: 'landscape',
          viewportWidth: 1280,
          viewportHeight: 800,
          windowControls: 'windows-overlay',
        }),
      },
    });

    expect(root.getAttribute('data-product-context')).toBe('library');
    expect(root.getAttribute('data-shell-mode')).toBe('desktop');
    expect(root.getAttribute('data-orientation')).toBe('landscape');
    expect(primaryNavs().rail).toBeTruthy();
    expect(primaryNavs().bottom).toBeNull();
    expect(screen.getByRole('main')).toBeTruthy();
  });

  it('hides shell chrome in reader presentation', () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          presentation: 'reader',
          foregroundActivity: { kind: 'reader', id: 'chapter-7' },
          productContext: 'apps',
          mediaSpace: 'novel',
        }),
      },
    });

    expect(screen.queryByRole('navigation', { name: '主导航' })).toBeNull();
    expect(screen.queryByRole('navigation', { name: '底部主导航' })).toBeNull();
    expect(screen.queryByRole('banner')).toBeNull();
    expect(document.querySelector('[data-shell-mesh]')).toBeNull();
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('collapses desktop spine then restores with keyboard focus', async () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          platform: desktopPlatform({ windowControls: 'browser-preview' }),
        }),
      },
    });

    const root = screen.getByTestId('mode-shell');
    expect(root.getAttribute('data-rail-collapsed')).toBe('false');
    expect(screen.getByRole('navigation', { name: '主导航' })).toBeTruthy();
    expect(document.querySelector('[data-shell-rail-slot="expanded"]')).toBeInstanceOf(HTMLElement);

    await fireEvent.click(screen.getByRole('button', { name: '侧栏选项' }));
    await fireEvent.click(screen.getByRole('button', { name: '收起侧栏' }));

    expect(root.getAttribute('data-rail-collapsed')).toBe('true');
    expect(screen.queryByRole('navigation', { name: '主导航' })).toBeNull();
    expect(document.querySelector('[data-shell-rail-slot="collapsed"]')).toBeInstanceOf(
      HTMLElement,
    );
    expect(localStorage.getItem(RAIL_COLLAPSED_STORAGE_KEY)).toBe('1');

    const restoreButton = screen.getByRole('button', { name: '展开侧栏' });
    expect(restoreButton.className).toContain('min-h-11');
    expect(document.activeElement).toBe(restoreButton);
    await fireEvent.click(restoreButton);

    expect(root.getAttribute('data-rail-collapsed')).toBe('false');
    expect(screen.getByRole('navigation', { name: '主导航' })).toBeTruthy();
    expect(document.querySelector('[data-shell-rail-slot="expanded"]')).toBeInstanceOf(HTMLElement);
    expect(localStorage.getItem(RAIL_COLLAPSED_STORAGE_KEY)).toBe('0');
    expect(document.activeElement).toBe(screen.getByRole('button', { name: '侧栏选项' }));
  });

  it('consumes shell contract only — chrome follows shell.platform not window size', () => {
    // 窗口可很宽，但 shell 声明 mobile → 仅底栏。
    Object.defineProperty(window, 'innerWidth', {
      configurable: true,
      writable: true,
      value: 1440,
    });

    render(AppShell, {
      props: {
        shell: makeShell({ platform: mobilePlatform() }),
      },
    });

    expect(screen.queryByRole('navigation', { name: '主导航' })).toBeNull();
    expect(screen.getByRole('navigation', { name: '底部主导航' })).toBeTruthy();
    expect(screen.getByTestId('mode-shell').getAttribute('data-shell-mode')).toBe('mobile');
  });

  it('exposes reduced-motion and reduced-transparency on shell root', async () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          theme: {
            mode: 'system',
            appearancePack: 'obsidian-void',
            reducedMotion: true,
            reducedTransparency: true,
          },
        }),
      },
    });

    // 等 a11y 材质同步 effect 跑完。
    await Promise.resolve();

    const root = screen.getByTestId('mode-shell');
    expect(root.getAttribute('data-reduced-motion')).toBe('true');
    expect(root.getAttribute('data-reduced-transparency')).toBe('true');
    // 实色材质，不改写已存用户偏好。
    expect(getMaterialTransparency()).toBe('standard');
    expect(document.documentElement.dataset.materialTransparency).toBe('low');
    // a11y 降级时导航与可访问名仍在。
    expect(screen.getByRole('navigation', { name: '主导航' })).toBeTruthy();
    expect(screen.getByRole('main')).toBeTruthy();
  });

  it('marks reduced flags false when shell a11y prefs are off', () => {
    render(AppShell, { props: { shell: makeShell() } });

    const root = screen.getByTestId('mode-shell');
    expect(root.getAttribute('data-reduced-motion')).toBe('false');
    expect(root.getAttribute('data-reduced-transparency')).toBe('false');
  });

  it('skips launch visual when cold start is under threshold', () => {
    stubPerformanceNow(COLD_LAUNCH_THRESHOLD_MS - 40);

    render(AppShell, { props: { shell: makeShell() } });

    expect(screen.queryByRole('region', { name: 'LanJing 启动动画' })).toBeNull();
    expect(sessionStorage.getItem(COLD_LAUNCH_SESSION_KEY)).toBeNull();
  });

  it('shows launch visual only on slow cold start and records session', () => {
    stubPerformanceNow(COLD_LAUNCH_THRESHOLD_MS + 50);

    render(AppShell, { props: { shell: makeShell() } });

    expect(screen.getByRole('region', { name: 'LanJing 启动动画' })).toBeTruthy();
    expect(sessionStorage.getItem(COLD_LAUNCH_SESSION_KEY)).toBe('1');
  });

  it('does not replay launch when session already recorded a show', () => {
    sessionStorage.setItem(COLD_LAUNCH_SESSION_KEY, '1');
    stubPerformanceNow(COLD_LAUNCH_THRESHOLD_MS + 500);

    render(AppShell, { props: { shell: makeShell() } });

    expect(screen.queryByRole('region', { name: 'LanJing 启动动画' })).toBeNull();
  });
});
