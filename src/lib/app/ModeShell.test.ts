import { render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { BeforeNavigate } from '@sveltejs/kit';
import ModeShell from './ModeShell.svelte';
import ModeShellPlatformContextHarness from './ModeShellPlatformContextHarness.test.svelte';
import {
  registerLeaveGuard,
  resetLeaveCoordinatorForTests,
  resolvePendingLeave,
} from './leave-coordinator.svelte';
import {
  getActivityOverride,
  resetShellSession,
  setActivityOverride,
  setAmbientAudio,
} from './shell-session.svelte';
import type { ModeShellContract } from './shell-types';
import type { RuntimePlatform } from './platform-runtime';

const resolveRuntimePlatform = vi.hoisted(() =>
  vi.fn<() => Promise<RuntimePlatform>>(() => Promise.resolve('unknown')),
);

const tauriWindowMocks = vi.hoisted(() => {
  const unlisten = vi.fn();
  const onCloseRequested = vi.fn(async () => unlisten);
  const destroy = vi.fn(() => Promise.resolve());
  const getCurrentWindow = vi.fn(() => ({ onCloseRequested, destroy }));
  return { destroy, getCurrentWindow, onCloseRequested, unlisten };
});

const navigationMocks = vi.hoisted(() => ({
  beforeNavigate: vi.fn<(callback: (navigation: BeforeNavigate) => void) => void>(),
  goto: vi.fn<(url: string | URL, options?: { replaceState?: boolean }) => Promise<void>>(() =>
    Promise.resolve(),
  ),
}));

vi.mock('./platform-runtime', () => ({ resolveRuntimePlatform }));
vi.mock('$app/navigation', () => navigationMocks);
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: tauriWindowMocks.getCurrentWindow }));

const contract: ModeShellContract = {
  productContext: 'library',
  settingsActive: false,
  mediaSpace: 'novel',
  foregroundActivity: { kind: 'reader', id: 'chapter-7' },
  presentation: 'reader',
  platform: {
    kind: 'android',
    orientation: 'portrait',
    viewportWidth: 390,
    viewportHeight: 844,
    hover: 'none',
    pointer: 'coarse',
    keyboard: false,
    touch: true,
    windowControls: 'browser-preview',
  },
  theme: {
    mode: 'dark',
    appearancePack: 'obsidian-void',
    reducedMotion: false,
    reducedTransparency: false,
  },
  ambientAudio: {
    id: 'ambient-1',
    state: 'paused',
    focus: 'none',
    label: '夜航',
  },
};

function setViewport(width: number, height: number) {
  Object.defineProperty(window, 'innerWidth', {
    configurable: true,
    writable: true,
    value: width,
  });
  Object.defineProperty(window, 'innerHeight', {
    configurable: true,
    writable: true,
    value: height,
  });
  window.dispatchEvent(new Event('resize'));
}

type TauriRuntimeWindow = Window & { __TAURI_INTERNALS__?: object };

function enableTauriRuntime() {
  (window as TauriRuntimeWindow).__TAURI_INTERNALS__ = {};
}

afterEach(() => {
  resetShellSession();
  resetLeaveCoordinatorForTests();
  navigationMocks.beforeNavigate.mockReset();
  navigationMocks.goto.mockReset();
  navigationMocks.goto.mockResolvedValue(undefined);
  resolveRuntimePlatform.mockReset();
  resolveRuntimePlatform.mockResolvedValue('unknown');
  delete (window as TauriRuntimeWindow).__TAURI_INTERNALS__;
  tauriWindowMocks.destroy.mockClear();
  tauriWindowMocks.getCurrentWindow.mockClear();
  tauriWindowMocks.onCloseRequested.mockClear();
  tauriWindowMocks.unlisten.mockClear();
});

describe('ModeShell', () => {
  it('publishes the asynchronously resolved OS through the reactive platform context', async () => {
    let resolvePlatform: ((platform: RuntimePlatform) => void) | undefined;
    resolveRuntimePlatform.mockReset();
    resolveRuntimePlatform.mockImplementationOnce(
      () =>
        new Promise<RuntimePlatform>((resolve) => {
          resolvePlatform = resolve;
        }),
    );

    render(ModeShellPlatformContextHarness);
    const probe = screen.getByTestId('platform-context-probe');
    expect(probe.textContent).toBe('unknown');
    await waitFor(() => expect(resolveRuntimePlatform).toHaveBeenCalledTimes(1));

    if (!resolvePlatform) throw new Error('Runtime platform resolver was not started');
    resolvePlatform('windows');

    await waitFor(() => {
      expect(probe.textContent).toBe('windows');
      expect(screen.getByTestId('mode-shell').getAttribute('data-platform')).toBe('windows');
    });
  });

  it('registers and cleans the desktop close guard when the OS query resolves', async () => {
    enableTauriRuntime();
    resolveRuntimePlatform.mockResolvedValueOnce('windows');
    const view = render(ModeShell);

    try {
      await waitFor(() => expect(tauriWindowMocks.onCloseRequested).toHaveBeenCalledOnce());
    } finally {
      view.unmount();
    }
    await waitFor(() => expect(tauriWindowMocks.unlisten).toHaveBeenCalledOnce());
  });

  it('registers and cleans the desktop close guard when the OS query rejects', async () => {
    enableTauriRuntime();
    resolveRuntimePlatform.mockRejectedValueOnce(new Error('OS plugin unavailable'));
    const view = render(ModeShell);

    try {
      await waitFor(() => expect(tauriWindowMocks.onCloseRequested).toHaveBeenCalledOnce());
      expect(screen.getByTestId('mode-shell').getAttribute('data-platform')).toBe('browser');
    } finally {
      view.unmount();
    }
    await waitFor(() => expect(tauriWindowMocks.unlisten).toHaveBeenCalledOnce());
  });

  it('does not import or register the Tauri close guard in a browser runtime', async () => {
    resolveRuntimePlatform.mockResolvedValueOnce('unknown');
    const view = render(ModeShell);

    try {
      await waitFor(() => expect(resolveRuntimePlatform).toHaveBeenCalledOnce());
      expect(tauriWindowMocks.getCurrentWindow).not.toHaveBeenCalled();
      expect(tauriWindowMocks.onCloseRequested).not.toHaveBeenCalled();
    } finally {
      view.unmount();
    }
  });

  it('passes product, media, activity, presentation, platform, theme, and audio through one boundary', () => {
    render(ModeShell, { props: { shell: contract } });

    const shell = screen.getByTestId('mode-shell');
    expect(shell.getAttribute('data-product-context')).toBe('library');
    expect(shell.getAttribute('data-settings-active')).toBe('false');
    expect(shell.getAttribute('data-media-space')).toBe('novel');
    expect(shell.getAttribute('data-foreground-activity')).toBe('reader:chapter-7');
    expect(shell.getAttribute('data-presentation')).toBe('reader');
    expect(shell.getAttribute('data-platform')).toBe('android');
    expect(shell.getAttribute('data-orientation')).toBe('portrait');
    expect(shell.getAttribute('data-theme-mode')).toBe('dark');
    expect(shell.getAttribute('data-appearance-pack')).toBe('obsidian-void');
    expect(shell.getAttribute('data-ambient-audio')).toBe('paused');
    expect(screen.queryByRole('navigation', { name: '主导航' })).toBeNull();
  });

  it('keeps unrelated activity, theme, and audio identity when route and platform change', async () => {
    const view = render(ModeShell, { props: { shell: contract } });

    await view.rerender({
      shell: {
        ...contract,
        productContext: 'realm',
        mediaSpace: null,
        presentation: 'normal',
        platform: {
          ...contract.platform,
          kind: 'windows',
          orientation: 'landscape',
          viewportWidth: 1440,
          viewportHeight: 900,
          hover: 'hover',
          pointer: 'fine',
          keyboard: true,
          touch: false,
          windowControls: 'windows-overlay',
        },
      },
    });

    const shell = screen.getByTestId('mode-shell');
    expect(shell.getAttribute('data-product-context')).toBe('realm');
    expect(shell.getAttribute('data-platform')).toBe('windows');
    expect(shell.getAttribute('data-orientation')).toBe('landscape');
    expect(shell.getAttribute('data-foreground-activity')).toBe('reader:chapter-7');
    expect(shell.getAttribute('data-theme-mode')).toBe('dark');
    expect(shell.getAttribute('data-appearance-pack')).toBe('obsidian-void');
    expect(shell.getAttribute('data-ambient-audio')).toBe('paused');
  });

  it('drives browse media-space and reader chrome from injected contract classes', async () => {
    const browseMedia: ModeShellContract = {
      ...contract,
      productContext: 'apps',
      mediaSpace: 'music',
      foregroundActivity: { kind: 'browse', id: 'music' },
      presentation: 'normal',
      ambientAudio: null,
      platform: {
        ...contract.platform,
        kind: 'windows',
        orientation: 'landscape',
        viewportWidth: 1440,
        viewportHeight: 900,
        hover: 'hover',
        pointer: 'fine',
        keyboard: true,
        touch: false,
        windowControls: 'windows-overlay',
      },
      theme: { ...contract.theme, mode: 'light' },
    };

    const view = render(ModeShell, { props: { shell: browseMedia } });
    let shell = screen.getByTestId('mode-shell');
    expect(shell.getAttribute('data-product-context')).toBe('apps');
    expect(shell.getAttribute('data-media-space')).toBe('music');
    expect(shell.getAttribute('data-foreground-activity')).toBe('browse:music');
    expect(shell.getAttribute('data-presentation')).toBe('normal');
    expect(screen.getByRole('navigation', { name: '主导航' })).toBeTruthy();

    await view.rerender({ shell: contract });
    shell = screen.getByTestId('mode-shell');
    expect(shell.getAttribute('data-foreground-activity')).toBe('reader:chapter-7');
    expect(shell.getAttribute('data-presentation')).toBe('reader');
    expect(screen.queryByRole('navigation', { name: '主导航' })).toBeNull();
  });

  it('keeps ambient audio from session seam when viewport/platform changes (production path)', async () => {
    setViewport(1280, 800);
    setAmbientAudio({
      id: 'ambient-live',
      state: 'playing',
      focus: 'ambient',
      label: '夜航',
    });

    render(ModeShell);

    let shell = screen.getByTestId('mode-shell');
    expect(shell.getAttribute('data-ambient-audio')).toBe('playing');
    // 工作台壳不再挂 mini-player 文案；ambient 仅 data 属性暴露
    expect(screen.queryByText('夜航')).toBeNull();
    expect(document.querySelector('[data-mini-player]')).toBeNull();

    setViewport(390, 844);
    // 等 svelte:window 绑定处理 resize
    await Promise.resolve();

    shell = screen.getByTestId('mode-shell');
    expect(shell.getAttribute('data-ambient-audio')).toBe('playing');
    expect(shell.getAttribute('data-orientation')).toBe('portrait');
    expect(screen.queryByText('夜航')).toBeNull();
  });

  it('keeps explicit activity override across platform-only changes (production path)', async () => {
    setViewport(1280, 800);
    setActivityOverride({ kind: 'player', id: 'track-9' });

    render(ModeShell);

    let shell = screen.getByTestId('mode-shell');
    expect(shell.getAttribute('data-foreground-activity')).toBe('player:track-9');
    expect(getActivityOverride()).toEqual({ kind: 'player', id: 'track-9' });

    setViewport(390, 844);
    await Promise.resolve();

    shell = screen.getByTestId('mode-shell');
    expect(shell.getAttribute('data-foreground-activity')).toBe('player:track-9');
    expect(shell.getAttribute('data-orientation')).toBe('portrait');
    expect(getActivityOverride()).toEqual({ kind: 'player', id: 'track-9' });
  });

  it('restores chrome when injected contract leaves reader presentation', async () => {
    const view = render(ModeShell, { props: { shell: contract } });
    expect(screen.queryByRole('navigation', { name: '主导航' })).toBeNull();

    await view.rerender({
      shell: {
        ...contract,
        productContext: 'realm',
        mediaSpace: null,
        foregroundActivity: { kind: 'browse', id: 'realm' },
        presentation: 'normal',
        platform: {
          ...contract.platform,
          kind: 'windows',
          orientation: 'landscape',
          viewportWidth: 1440,
          viewportHeight: 900,
          hover: 'hover',
          pointer: 'fine',
          keyboard: true,
          touch: false,
          windowControls: 'windows-overlay',
        },
        ambientAudio: null,
      },
    });

    const shell = screen.getByTestId('mode-shell');
    expect(shell.getAttribute('data-foreground-activity')).toBe('browse:realm');
    expect(shell.getAttribute('data-presentation')).toBe('normal');
    expect(screen.getByRole('navigation', { name: '主导航' })).toBeTruthy();
  });

  it('updates reduced motion/transparency data attrs when system media queries change', async () => {
    type Listener = (event: MediaQueryListEvent) => void;
    const mediaLists = new Map<
      string,
      MediaQueryList & { matches: boolean; listeners: Set<Listener> }
    >();
    const originalMatchMedia = window.matchMedia;

    const matchMediaMock = vi.fn((query: string): MediaQueryList => {
      const existing = mediaLists.get(query);
      if (existing) return existing;

      const entry = {
        matches: false,
        media: query,
        onchange: null,
        listeners: new Set<Listener>(),
        addListener: () => undefined,
        removeListener: () => undefined,
        addEventListener: (_type: string, listener: EventListenerOrEventListenerObject) => {
          entry.listeners.add(listener as Listener);
        },
        removeEventListener: (_type: string, listener: EventListenerOrEventListenerObject) => {
          entry.listeners.delete(listener as Listener);
        },
        dispatchEvent: () => false,
      };
      mediaLists.set(
        query,
        entry as MediaQueryList & { matches: boolean; listeners: Set<Listener> },
      );
      return entry as MediaQueryList;
    });

    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      writable: true,
      value: matchMediaMock,
    });

    try {
      setViewport(1280, 800);
      render(ModeShell);

      // 等 a11y media 监听挂上。
      await Promise.resolve();

      let shell = screen.getByTestId('mode-shell');
      expect(shell.getAttribute('data-reduced-motion')).toBe('false');
      expect(shell.getAttribute('data-reduced-transparency')).toBe('false');

      const fire = (query: string, matches: boolean) => {
        const list = mediaLists.get(query);
        if (!list) return;
        list.matches = matches;
        for (const listener of list.listeners) {
          listener({ matches, media: query } as MediaQueryListEvent);
        }
      };

      fire('(prefers-reduced-motion: reduce)', true);
      fire('(prefers-reduced-transparency: reduce)', true);
      await Promise.resolve();

      shell = screen.getByTestId('mode-shell');
      expect(shell.getAttribute('data-reduced-motion')).toBe('true');
      expect(shell.getAttribute('data-reduced-transparency')).toBe('true');
      // 降级后信息/焦点路径仍在。
      expect(screen.getByRole('navigation', { name: '主导航' })).toBeTruthy();
      expect(screen.getByRole('main')).toBeTruthy();
    } finally {
      Object.defineProperty(window, 'matchMedia', {
        configurable: true,
        writable: true,
        value: originalMatchMedia,
      });
    }
  });
  it('registers one route interceptor and replays an approved internal navigation once', async () => {
    const resolveLeave = vi.fn(() => Promise.resolve('resolved' as const));
    registerLeaveGuard({ canLeave: () => false, resolveLeave });
    render(ModeShell, { props: { shell: contract } });
    const callback = navigationMocks.beforeNavigate.mock.calls.at(-1)?.[0];
    if (!callback) throw new Error('beforeNavigate was not registered');

    const cancel = vi.fn();
    const target = new URL('https://lanjing.test/sources');
    callback({
      cancel,
      willUnload: false,
      type: 'link',
      from: { url: new URL('https://lanjing.test/sources/rules') },
      to: { url: target },
    } as unknown as BeforeNavigate);

    expect(cancel).toHaveBeenCalledOnce();
    await expect(resolvePendingLeave('discard')).resolves.toBe('resolved');
    expect(resolveLeave).toHaveBeenCalledWith('discard');
    expect(navigationMocks.goto).toHaveBeenCalledOnce();
    expect(navigationMocks.goto).toHaveBeenCalledWith('/sources');
  });

  it('replays an approved popstate target through goto without another history event', async () => {
    registerLeaveGuard({
      canLeave: () => false,
      resolveLeave: () => Promise.resolve('resolved'),
    });
    render(ModeShell, { props: { shell: contract } });
    const callback = navigationMocks.beforeNavigate.mock.calls.at(-1)?.[0];
    if (!callback) throw new Error('beforeNavigate was not registered');

    callback({
      cancel: vi.fn(),
      willUnload: false,
      type: 'popstate',
      delta: -1,
      from: { url: new URL('https://lanjing.test/sources/rules') },
      to: { url: new URL('https://lanjing.test/library') },
    } as unknown as BeforeNavigate);

    await expect(resolvePendingLeave('discard')).resolves.toBe('resolved');
    expect(navigationMocks.goto).toHaveBeenCalledOnce();
    expect(navigationMocks.goto).toHaveBeenCalledWith('/library', { replaceState: true });
  });
});
