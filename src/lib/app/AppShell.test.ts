import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getMaterialTransparency, setMaterialTransparency } from '$lib/stores/theme.svelte';
import AppShell from './AppShell.svelte';
import { COLD_LAUNCH_SESSION_KEY, COLD_LAUNCH_THRESHOLD_MS } from './cold-launch';
import { RAIL_COLLAPSED_STORAGE_KEY } from './shell-rail-preference';
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
      appearancePack: 'inkstone-precision',
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
  setMaterialTransparency('standard');
});

describe('AppShell', () => {
  it('renders quiet desktop spine rail and minimal titlebar', () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          platform: desktopPlatform({ windowControls: 'browser-preview' }),
        }),
      },
    });

    const root = screen.getByTestId('mode-shell');
    expect(root.getAttribute('data-theme-mode')).toBe('system');
    expect(root.getAttribute('data-appearance-pack')).toBe('inkstone-precision');
    expect(root.getAttribute('data-chrome-family')).toBe('rail');

    const nav = screen.getByRole('navigation', { name: '主导航' });
    expect(nav.getAttribute('data-shell-rail')).toBe('spine');
    expect(screen.queryByRole('navigation', { name: '底部主导航' })).toBeNull();

    // 四境 + 脊上设置；无文案副标题、无 titlebar 工具簇
    expect(screen.getByRole('link', { name: '境场' })).toBeTruthy();
    expect(screen.getByRole('link', { name: '打开设置' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '收起侧栏' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: '打开全局搜索' })).toBeNull();
    expect(screen.queryByRole('button', { name: /切换主题模式/ })).toBeNull();

    const titlebar = screen.getByRole('banner', { name: '窗口标题栏：境场' });
    expect(titlebar.getAttribute('data-native-window-controls')).toBe('browser-preview');
    expect(titlebar.hasAttribute('data-tauri-drag-region')).toBe(false);
    expect(titlebar.querySelectorAll('[data-tauri-drag-region]').length).toBeGreaterThan(0);
    expect(screen.getByRole('button', { name: '最小化窗口' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '最大化或还原窗口' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '关闭窗口' })).toBeTruthy();
  });

  it('keeps mobile bottom nav accessible and separate from mini-player reservation', async () => {
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
    expect(bottomNav.className).toContain('pb-(--shell-bottom-safe-padding)');
    expect(bottomNav.className).toContain(
      'min-h-[calc(var(--shell-bottom-nav-height)+var(--shell-bottom-safe-padding))]',
    );
    expect(bottomNav.className).not.toContain('md:hidden');
    expect(screen.getByRole('link', { name: '境场' })).toBeTruthy();
    expect(screen.getByRole('link', { name: '应用' })).toBeTruthy();
    expect(screen.getByRole('link', { name: '来源' })).toBeTruthy();
    expect(screen.getByRole('link', { name: '资料库' })).toBeTruthy();
    // 移动：顶栏设置；底栏仅四境，无搜索入口
    const settingsLink = screen.getByRole('link', { name: '打开设置' });
    expect(settingsLink.closest('[data-mobile-toolbar]')).toBeTruthy();
    expect(screen.queryByRole('button', { name: '打开全局搜索' })).toBeNull();
    expect(bottomNav.querySelectorAll('a')).toHaveLength(4);

    // 无 ambient：纯座位，非幽灵按钮。
    const miniPlayer = document.querySelector('[data-mini-player="seat"]');
    expect(miniPlayer).toBeInstanceOf(HTMLElement);
    expect(screen.queryByRole('button', { name: '暂无播放内容' })).toBeNull();
    // 纵向顺序：main → mini-player 槽 → 底栏（互不遮盖）。
    const main = screen.getByRole('main');
    if (!(miniPlayer instanceof HTMLElement)) {
      throw new Error('expected mini-player seat element');
    }
    expect(
      main.compareDocumentPosition(miniPlayer) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      miniPlayer.compareDocumentPosition(bottomNav) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    // 内容列用 flex，无 titlebar 时 main 仍可伸展。
    expect(main.className).toContain('flex-1');
    expect(main.parentElement?.className).toContain('flex-col');
  });

  it('shows interactive mini-player only when ambient audio session exists', () => {
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

    const miniPlayer = screen.getByRole('button', { name: '当前播放' });
    expect(miniPlayer.getAttribute('data-mini-player')).toBe('active');
    expect(screen.getByText('雨声')).toBeTruthy();
    expect(document.querySelector('[data-mini-player="seat"]')).toBeNull();
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

  it('passes shell.platform.windowControls through titlebar without local override', () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          platform: desktopPlatform({
            kind: 'windows',
            windowControls: 'windows-overlay',
          }),
        }),
      },
    });

    const titlebar = screen.getByRole('banner', { name: '窗口标题栏：境场' });
    expect(titlebar.getAttribute('data-native-window-controls')).toBe('windows-overlay');
    // 无边框官方标题：windows-overlay 下 HTML 控件始终存在。
    expect(screen.getByRole('button', { name: '最小化窗口' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '最大化或还原窗口' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '关闭窗口' })).toBeTruthy();
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
  });

  it('collapses desktop spine then restores via left-edge hit', async () => {
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

    await fireEvent.click(screen.getByRole('button', { name: '收起侧栏' }));

    expect(root.getAttribute('data-rail-collapsed')).toBe('true');
    expect(screen.queryByRole('navigation', { name: '主导航' })).toBeNull();
    expect(localStorage.getItem(RAIL_COLLAPSED_STORAGE_KEY)).toBe('1');

    const edge = document.querySelector('[data-shell-rail-edge]');
    expect(edge).toBeInstanceOf(HTMLElement);
    if (!(edge instanceof HTMLElement)) throw new Error('expected rail edge hit');
    await fireEvent.pointerEnter(edge);

    expect(root.getAttribute('data-rail-collapsed')).toBe('false');
    expect(screen.getByRole('navigation', { name: '主导航' })).toBeTruthy();
    expect(localStorage.getItem(RAIL_COLLAPSED_STORAGE_KEY)).toBe('0');
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
            appearancePack: 'inkstone-precision',
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
