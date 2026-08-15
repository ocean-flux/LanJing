import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getMaterialTransparency, setMaterialTransparency } from '$lib/stores/theme.svelte';
import AppShell from './AppShell.svelte';
import { COLD_LAUNCH_SESSION_KEY, COLD_LAUNCH_THRESHOLD_MS } from './cold-launch';
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
    settingsActive: false,
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

function stubPerformanceNow(ms: number) {
  vi.spyOn(performance, 'now').mockReturnValue(ms);
}

beforeEach(() => {
  stubPerformanceNow(0);
  setMaterialTransparency('standard');
});

afterEach(() => {
  vi.restoreAllMocks();
  sessionStorage.removeItem(COLD_LAUNCH_SESSION_KEY);
  setMaterialTransparency('standard');
});

describe('AppShell', () => {
  it('renders desktop primary navigation inside the continuous titlebar only', () => {
    render(AppShell, { props: { shell: makeShell() } });

    const root = screen.getByTestId('mode-shell');
    const titlebar = screen.getByRole('banner');
    const primaryNav = screen.getByRole('navigation', { name: '主导航' });

    expect(root.getAttribute('data-chrome-family')).toBe('titlebar');
    expect(titlebar.getAttribute('data-titlebar-chrome')).toBe('desktop');
    expect(titlebar.contains(primaryNav)).toBe(true);
    expect(screen.queryByRole('navigation', { name: '底部主导航' })).toBeNull();
    expect(within(primaryNav).getAllByRole('link')).toHaveLength(5);
    expect(
      within(primaryNav).getByRole('link', { name: '境场' }).getAttribute('aria-current'),
    ).toBe('page');
    expect(document.querySelector('[data-tauri-drag-region]')).toBeInstanceOf(HTMLElement);
  });

  it('renders mobile app bar and bottom navigation without desktop navigation or drag chrome', () => {
    render(AppShell, { props: { shell: makeShell({ platform: mobilePlatform() }) } });

    const titlebar = screen.getByRole('banner');
    const bottomNav = screen.getByRole('navigation', { name: '底部主导航' });

    expect(titlebar.getAttribute('data-titlebar-chrome')).toBe('app-bar');
    expect(screen.queryByRole('navigation', { name: '主导航' })).toBeNull();
    expect(within(bottomNav).getAllByRole('link')).toHaveLength(5);
    expect(within(bottomNav).getByRole('link', { name: '境场' }).getAttribute('aria-current')).toBe(
      'page',
    );
    expect(within(titlebar).queryByRole('link', { name: '设置' })).toBeNull();
    expect(document.querySelector('[data-tauri-drag-region]')).toBeNull();
    expect(screen.queryByRole('button', { name: '最小化' })).toBeNull();
  });

  it('never mounts titlebar and bottom primary navigation together', () => {
    const cases: PlatformCapabilities[] = [
      desktopPlatform(),
      desktopPlatform({ viewportWidth: 900 }),
      mobilePlatform(),
      mobilePlatform({ viewportWidth: 900, viewportHeight: 1200 }),
      mobilePlatform({ viewportWidth: 1100, viewportHeight: 700, orientation: 'landscape' }),
    ];

    for (const platform of cases) {
      const { unmount } = render(AppShell, { props: { shell: makeShell({ platform }) } });
      const titlebarNav = screen.queryByRole('navigation', { name: '主导航' });
      const bottomNav = screen.queryByRole('navigation', { name: '底部主导航' });
      expect(Boolean(titlebarNav) && Boolean(bottomNav)).toBe(false);
      unmount();
    }
  });

  it('does not render fake window controls in browser preview', () => {
    render(AppShell, {
      props: {
        shell: makeShell({ platform: desktopPlatform({ windowControls: 'browser-preview' }) }),
      },
    });

    expect(screen.getByRole('banner').getAttribute('data-titlebar-controls')).toBe('native');
    expect(screen.queryByRole('button', { name: '最小化' })).toBeNull();
    expect(screen.queryByRole('button', { name: '最大化' })).toBeNull();
    expect(screen.queryByRole('button', { name: '关闭' })).toBeNull();
  });

  it('mounts all three HTML controls for 768px Windows overlay mode', () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          platform: desktopPlatform({
            kind: 'windows',
            viewportWidth: 768,
            viewportHeight: 900,
            windowControls: 'windows-overlay',
          }),
        }),
      },
    });

    expect(screen.getByRole('navigation', { name: '主导航' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '最小化' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '最大化' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '关闭' })).toBeTruthy();
    expect(document.querySelectorAll('[data-window-action]')).toHaveLength(3);
  });

  it('reserves macOS traffic-light space without HTML controls', () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          platform: desktopPlatform({ kind: 'macos', windowControls: 'macos-overlay' }),
        }),
      },
    });

    expect(document.querySelector('[data-macos-traffic-light-safe="true"]')).toBeTruthy();
    expect(screen.queryByRole('button', { name: '关闭' })).toBeNull();
  });

  it('marks only settings current when the ModeShell contract says settings is active', () => {
    render(AppShell, {
      props: {
        shell: makeShell({ productContext: 'realm', settingsActive: true }),
      },
    });

    const primaryNav = screen.getByRole('navigation', { name: '主导航' });
    const primaryLinks = within(primaryNav).getAllByRole('link');
    const settingsLink = within(primaryNav).getByRole('link', { name: '设置' });
    const currentLinks = within(primaryNav).getAllByRole('link', { current: 'page' });
    expect(screen.getByTestId('mode-shell').getAttribute('data-settings-active')).toBe('true');
    expect(primaryLinks).toHaveLength(5);
    expect(primaryLinks[primaryLinks.length - 1]).toBe(settingsLink);
    expect(currentLinks).toEqual([settingsLink]);
  });

  it('uses settings as the fifth bottom destination without duplicating it in the app bar', () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          productContext: 'realm',
          settingsActive: true,
          platform: mobilePlatform(),
        }),
      },
    });

    const titlebar = screen.getByRole('banner');
    const bottomNav = screen.getByRole('navigation', { name: '底部主导航' });
    const bottomLinks = within(bottomNav).getAllByRole('link');
    const settingsLink = within(bottomNav).getByRole('link', { name: '设置' });
    const currentLinks = within(bottomNav).getAllByRole('link', { current: 'page' });

    expect(bottomLinks).toHaveLength(5);
    expect(bottomLinks[bottomLinks.length - 1]).toBe(settingsLink);
    expect(currentLinks).toEqual([settingsLink]);
    expect(within(titlebar).queryByRole('link', { name: '设置' })).toBeNull();
  });

  it('hides titlebar and bottom navigation in reader presentation', () => {
    render(AppShell, {
      props: {
        shell: makeShell({
          presentation: 'reader',
          foregroundActivity: { kind: 'reader', id: 'chapter-7' },
        }),
      },
    });

    expect(screen.queryByRole('banner')).toBeNull();
    expect(screen.queryByRole('navigation', { name: '主导航' })).toBeNull();
    expect(screen.queryByRole('navigation', { name: '底部主导航' })).toBeNull();
    expect(screen.getByRole('main')).toBeTruthy();
  });

  it('exposes one keyboard skip target on the single application scroll owner', async () => {
    render(AppShell, { props: { shell: makeShell() } });

    const owners = document.querySelectorAll('[data-app-scroll-region]');
    const main = screen.getByRole('main');
    const skipLink = screen.getByRole('link', { name: '跳到主要内容' });
    expect(owners).toHaveLength(1);
    expect(owners[0]).toBe(main);
    expect(main.id).toBe('main-content');
    expect(main.getAttribute('tabindex')).toBe('-1');
    expect(main.classList.contains('app-scroll-region')).toBe(true);
    expect(main.classList.contains('overflow-y-auto')).toBe(true);
    expect(skipLink.getAttribute('href')).toBe('#main-content');

    skipLink.focus();
    expect(document.activeElement).toBe(skipLink);
    await fireEvent.click(skipLink);
    expect(document.activeElement).toBe(main);
  });

  it('keeps ambient audio as contract data without mini-player chrome', () => {
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

    expect(screen.getByTestId('mode-shell').getAttribute('data-ambient-audio')).toBe('paused');
    expect(document.querySelector('[data-mini-player]')).toBeNull();
    expect(screen.queryByText('雨声')).toBeNull();
  });

  it('syncs reduced transparency without changing the stored preference', async () => {
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

    await Promise.resolve();

    expect(screen.getByTestId('mode-shell').getAttribute('data-reduced-motion')).toBe('true');
    expect(getMaterialTransparency()).toBe('standard');
    expect(document.documentElement.dataset.materialTransparency).toBe('low');
  });

  it('shows the cold-launch visual only for a slow first launch', () => {
    stubPerformanceNow(COLD_LAUNCH_THRESHOLD_MS + 50);
    const { unmount } = render(AppShell, { props: { shell: makeShell() } });

    expect(screen.getByRole('region', { name: 'LanJing 启动动画' })).toBeTruthy();
    expect(sessionStorage.getItem(COLD_LAUNCH_SESSION_KEY)).toBe('1');
    unmount();

    render(AppShell, { props: { shell: makeShell() } });
    expect(screen.queryByRole('region', { name: 'LanJing 启动动画' })).toBeNull();
  });
});
