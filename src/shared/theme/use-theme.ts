import { useCallback, useSyncExternalStore } from 'react';
import {
  activePackId,
  getPreferences,
  resolveTheme,
  setAppearancePack,
  setTheme,
  subscribe,
  type AppearancePackId,
  type ResolvedTheme,
  type Theme,
  type ThemePreferences,
} from './theme';

export { applyTheme } from './theme';

const SERVER_SNAPSHOT: ThemePreferences = {
  theme: 'system',
  lightThemeId: 'porcelain-day',
  darkThemeId: 'obsidian-void',
};

export type UseThemeResult = {
  theme: Theme;
  resolvedTheme: ResolvedTheme;
  packId: AppearancePackId;
  lightThemeId: AppearancePackId;
  darkThemeId: AppearancePackId;
  setTheme: (theme: Theme) => void;
  chooseAppearancePack: (id: AppearancePackId) => void;
};

export function useTheme(): UseThemeResult {
  const preferences = useSyncExternalStore(subscribe, getPreferences, () => SERVER_SNAPSHOT);
  const choose = useCallback((id: AppearancePackId) => {
    setAppearancePack(id);
  }, []);
  const change = useCallback((next: Theme) => {
    setTheme(next);
  }, []);

  return {
    theme: preferences.theme,
    resolvedTheme: resolveTheme(preferences.theme),
    packId: activePackId(preferences.theme),
    lightThemeId: preferences.lightThemeId,
    darkThemeId: preferences.darkThemeId,
    setTheme: change,
    chooseAppearancePack: choose,
  };
}
