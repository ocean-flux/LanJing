import { describe, expect, it } from 'vitest';
import { shouldRenderHtmlWindowControls } from './window-controls';

describe('shouldRenderHtmlWindowControls', () => {
  it.each([
    ['windows-overlay', true],
    ['browser-preview', false],
    ['macos-overlay', false],
    ['system-decorated', false],
  ] as const)('maps %s visibility to %s', (mode, expected) => {
    expect(shouldRenderHtmlWindowControls(mode)).toBe(expected);
  });
});
