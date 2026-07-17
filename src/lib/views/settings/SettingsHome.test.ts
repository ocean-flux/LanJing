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
  setLightThemeId('inkstone-precision');
  setDarkThemeId('inkstone-precision');
});

afterEach(() => {
  localStorage.removeItem(WEB_PREFERENCES_STORAGE_KEY);
  setMode('system');
  setLightThemeId('inkstone-precision');
  setDarkThemeId('inkstone-precision');
});

describe('SettingsHome', () => {
  it('renders denselist prefs without shortcut grid or page H1', () => {
    render(SettingsHome);

    const home = screen.getByTestId('settings-home');
    expect(home).toBeTruthy();
    expect(home.querySelector('h1')).toBeNull();
    expect(screen.queryByRole('link', { name: /境场|应用|来源|资料库/ })).toBeNull();
    expect(screen.queryByText(/快捷/)).toBeNull();
    expect(screen.queryByText(/外观包/)).toBeNull();

    expect(screen.getByText('明暗模式')).toBeTruthy();
    expect(screen.getByText('亮色主题')).toBeTruthy();
    expect(screen.getByText('暗色主题')).toBeTruthy();
    expect(screen.getByText('界面语言')).toBeTruthy();
  });

  it('binds mode and dual-track themes', async () => {
    render(SettingsHome);

    await fireEvent.click(screen.getByRole('radio', { name: '深色' }));
    expect(getMode()).toBe('dark');

    await fireEvent.click(screen.getByRole('radio', { name: '浅色' }));
    expect(getMode()).toBe('light');

    // 亮/暗主题各两个色石；按 radiogroup 顺序点第二个冷银朱
    const lightGroup = screen.getByRole('radiogroup', { name: '亮色主题' });
    const darkGroup = screen.getByRole('radiogroup', { name: '暗色主题' });
    const lightRadios = lightGroup.querySelectorAll('[role="radio"]');
    const darkRadios = darkGroup.querySelectorAll('[role="radio"]');
    expect(lightRadios).toHaveLength(2);
    expect(darkRadios).toHaveLength(2);

    await fireEvent.click(lightRadios[0]!);
    await fireEvent.click(darkRadios[1]!);
    expect(getLightThemeId()).toBe('inkstone-precision');
    expect(getDarkThemeId()).toBe('cold-cinnabar');
  });
});
