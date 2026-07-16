import { render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import ModeShell from './ModeShell.svelte';
import {
  getActivityOverride,
  resetShellSession,
  setActivityOverride,
  setAmbientAudio,
} from './shell-session.svelte';
import type { ModeShellContract } from './shell-types';

const contract: ModeShellContract = {
  productContext: 'library',
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
    appearancePack: 'paper-lantern-precision',
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

afterEach(() => {
  resetShellSession();
});

describe('ModeShell', () => {
  it('passes product, media, activity, presentation, platform, theme, and audio through one boundary', () => {
    render(ModeShell, { props: { shell: contract } });

    const shell = screen.getByTestId('mode-shell');
    expect(shell.getAttribute('data-product-context')).toBe('library');
    expect(shell.getAttribute('data-media-space')).toBe('novel');
    expect(shell.getAttribute('data-foreground-activity')).toBe('reader:chapter-7');
    expect(shell.getAttribute('data-presentation')).toBe('reader');
    expect(shell.getAttribute('data-platform')).toBe('android');
    expect(shell.getAttribute('data-orientation')).toBe('portrait');
    expect(shell.getAttribute('data-theme-mode')).toBe('dark');
    expect(shell.getAttribute('data-appearance-pack')).toBe('paper-lantern-precision');
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
    expect(shell.getAttribute('data-appearance-pack')).toBe('paper-lantern-precision');
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
    expect(screen.getByText('夜航')).toBeTruthy();

    setViewport(390, 844);
    // 等 svelte:window 绑定处理 resize
    await Promise.resolve();

    shell = screen.getByTestId('mode-shell');
    expect(shell.getAttribute('data-ambient-audio')).toBe('playing');
    expect(shell.getAttribute('data-orientation')).toBe('portrait');
    expect(screen.getByText('夜航')).toBeTruthy();
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
});
