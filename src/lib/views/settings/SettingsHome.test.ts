import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  getDarkThemeId,
  getLightThemeId,
  getMode,
  setDarkThemeId,
  setLightThemeId,
  setMode,
  WEB_PREFERENCES_STORAGE_KEY,
} from '$lib/stores/theme.svelte';
import SettingsHome from './SettingsHome.svelte';

beforeEach(() => {
  localStorage.removeItem(WEB_PREFERENCES_STORAGE_KEY);
  setMode('system');
  setLightThemeId('porcelain-day');
  setDarkThemeId('obsidian-void');
});

afterEach(() => {
  localStorage.removeItem(WEB_PREFERENCES_STORAGE_KEY);
  setMode('system');
  setLightThemeId('porcelain-day');
  setDarkThemeId('obsidian-void');
});

describe('SettingsHome', () => {
  it('shows a visible page heading and grouped production preferences', () => {
    render(SettingsHome);

    expect(screen.getByRole('heading', { level: 1, name: '设置' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '外观' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '语言' })).toBeTruthy();
    expect(screen.getByText('明暗模式')).toBeTruthy();
    expect(screen.getByText('亮色主题')).toBeTruthy();
    expect(screen.getByText('暗色主题')).toBeTruthy();
    expect(screen.getByText('界面语言')).toBeTruthy();
    expect(screen.queryByRole('link', { name: /境场|应用|来源|资料库/ })).toBeNull();
  });

  it('binds labeled mode choices and dual-track themes', async () => {
    render(SettingsHome);

    const dark = screen.getByRole('radio', { name: '深色' });
    expect(dark.textContent).toContain('深色');
    expect(dark.querySelector('[aria-hidden="true"]')).toBeTruthy();
    await fireEvent.click(dark);
    expect(getMode()).toBe('dark');

    await fireEvent.click(screen.getByRole('radio', { name: '浅色' }));
    expect(getMode()).toBe('light');

    const lightGroup = screen.getByRole('radiogroup', { name: '亮色主题' });
    const darkGroup = screen.getByRole('radiogroup', { name: '暗色主题' });
    const lightRadios = lightGroup.querySelectorAll('[role="radio"]');
    const darkRadios = darkGroup.querySelectorAll('[role="radio"]');
    expect(lightRadios).toHaveLength(2);
    expect(darkRadios).toHaveLength(2);

    await fireEvent.click(lightRadios[0]!);
    await fireEvent.click(darkRadios[1]!);
    expect(getLightThemeId()).toBe('porcelain-day');
    expect(getDarkThemeId()).toBe('graphite-atelier');
  });

  it('keeps one tab stop per group and supports arrows, Home, and End', async () => {
    render(SettingsHome);

    for (const name of ['明暗模式', '亮色主题', '暗色主题', '界面语言']) {
      const radios = screen.getByRole('radiogroup', { name }).querySelectorAll('[role="radio"]');
      expect(
        Array.from(radios).filter((radio) => radio.getAttribute('tabindex') === '0'),
      ).toHaveLength(1);
    }

    const system = screen.getByRole('radio', { name: '跟随系统' });
    await fireEvent.keyDown(system, { key: 'ArrowLeft' });
    expect(getMode()).toBe('dark');
    expect(document.activeElement).toBe(screen.getByRole('radio', { name: '深色' }));

    const lightGroup = screen.getByRole('radiogroup', { name: '亮色主题' });
    const lightRadios = lightGroup.querySelectorAll('[role="radio"]');
    await fireEvent.keyDown(lightRadios[0]!, { key: 'End' });
    expect(getLightThemeId()).toBe('mist-studio');
    expect(document.activeElement).toBe(lightRadios[1]);

    await fireEvent.keyDown(lightRadios[1]!, { key: 'Home' });
    expect(getLightThemeId()).toBe('porcelain-day');
    expect(document.activeElement).toBe(lightRadios[0]);
  });
});
