import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
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

const setLocale = vi.hoisted(() => vi.fn<(locale: 'en' | 'zh-CN') => void>());

vi.mock('$lib/i18n', async () => {
  const actual = await vi.importActual<Record<string, unknown>>('$lib/i18n');
  return { ...actual, setLocale };
});

beforeEach(() => {
  setLocale.mockReset();
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
  it('uses the standard page frame and keeps the production preference regions', () => {
    const { container } = render(SettingsHome);

    expect(container.querySelector('[data-slot="page-frame"]')?.getAttribute('data-width')).toBe(
      'standard',
    );
    expect(screen.getByRole('heading', { level: 1, name: '设置' })).toBeTruthy();

    const appearance = screen.getByRole('region', { name: '外观' });
    const language = screen.getByRole('region', { name: '语言' });
    expect(within(appearance).getAllByRole('radiogroup')).toHaveLength(3);
    expect(within(language).getAllByRole('radiogroup')).toHaveLength(1);

    expect(screen.getByText('明暗模式')).toBeTruthy();
    expect(screen.getByText('亮色主题')).toBeTruthy();
    expect(screen.getByText('暗色主题')).toBeTruthy();
    expect(screen.getByText('界面语言')).toBeTruthy();
    expect(screen.queryByRole('link', { name: /境场|应用|来源|资料库/ })).toBeNull();
  });

  it('binds labeled choices to theme and locale owners', async () => {
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
    const english = screen.getByRole('radio', { name: 'EN' });
    await fireEvent.click(english);
    expect(english.getAttribute('aria-checked')).toBe('true');
    expect(setLocale).toHaveBeenCalledTimes(1);
    expect(setLocale).toHaveBeenCalledWith('en');
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
