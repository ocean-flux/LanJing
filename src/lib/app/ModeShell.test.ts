import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import ModeShell from './ModeShell.svelte';
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
          windowControls: 'system-decorated',
        },
      },
    });

    const shell = screen.getByTestId('mode-shell');
    expect(shell.getAttribute('data-product-context')).toBe('realm');
    expect(shell.getAttribute('data-platform')).toBe('windows');
    expect(shell.getAttribute('data-orientation')).toBe('landscape');
    expect(shell.getAttribute('data-foreground-activity')).toBe('reader:chapter-7');
    expect(shell.getAttribute('data-theme-mode')).toBe('dark');
    expect(shell.getAttribute('data-ambient-audio')).toBe('paused');
  });
});
