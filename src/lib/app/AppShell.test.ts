import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import AppShell from './AppShell.svelte';
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
      reducedMotion: false,
      reducedTransparency: false,
    },
    ambientAudio: null,
    ...overrides,
  };
}

describe('AppShell', () => {
  it('renders quiet desktop shell navigation and command search', async () => {
    render(AppShell, { props: { shell: makeShell() } });

    const nav = screen.getByRole('navigation', { name: '主导航' });
    expect(nav.getAttribute('data-shell-rail')).toBe('expanded');
    expect(screen.getByText('LanJing')).toBeTruthy();

    const titlebar = screen.getByRole('banner', { name: '窗口标题栏：境场' });
    expect(['browser-preview', 'system-decorated', 'macos-overlay', 'windows-overlay']).toContain(
      titlebar.getAttribute('data-native-window-controls'),
    );
    expect(titlebar.hasAttribute('data-tauri-drag-region')).toBe(true);
    expect(titlebar.querySelectorAll('[data-tauri-drag-region]').length).toBeGreaterThan(1);
    expect(screen.getByRole('button', { name: '最小化窗口' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '最大化或还原窗口' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '关闭窗口' })).toBeTruthy();

    await fireEvent.click(screen.getByRole('button', { name: '打开全局搜索' }));

    expect(screen.getByRole('dialog', { name: '全局搜索' })).toBeTruthy();
    expect(screen.getAllByText('暂无来源。先添加来源或导入文件。').length).toBeGreaterThan(0);
    expect(screen.getByRole('link', { name: '添加来源' })).toBeTruthy();
    expect(screen.getByRole('link', { name: '导入本地文件' })).toBeTruthy();
  });

  it('keeps mobile bottom nav accessible and separate from mini-player reservation', () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          platform: mobilePlatform(),
        }),
      },
    });

    expect(screen.queryByRole('navigation', { name: '主导航' })).toBeNull();
    const bottomNav = screen.getByRole('navigation', { name: '底部主导航' });
    expect(bottomNav.getAttribute('data-bottom-nav')).toBe('visible');
    expect(screen.getByRole('link', { name: '境场' })).toBeTruthy();
    expect(screen.getByRole('link', { name: '应用' })).toBeTruthy();
    expect(screen.getByRole('link', { name: '来源' })).toBeTruthy();
    expect(screen.getByRole('link', { name: '资料库' })).toBeTruthy();
    expect(
      screen.getByRole('button', { name: '暂无播放内容' }).getAttribute('data-mini-player'),
    ).toBe('reserved');
  });

  it('exposes icon-only theme control without a second material-standard switch', async () => {
    render(AppShell, { props: { shell: makeShell() } });

    const themeButton = screen.getByRole('button', { name: /切换主题模式/ });
    expect(themeButton.querySelector('svg')).toBeTruthy();
    expect(screen.getByRole('button', { name: '最小化窗口' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: /切换界面透明度/ })).toBeNull();

    await fireEvent.click(themeButton);

    expect(screen.getByRole('button', { name: /切换主题模式/ })).toBeTruthy();
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

  it('consumes shell contract only — chrome follows shell.platform not window size', () => {
    // Window may be wide, but shell says mobile → bottom nav only.
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
});
