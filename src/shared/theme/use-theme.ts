import { useCallback } from 'react';
import {
  activePackId,
  resolveTheme,
  setAppearancePack,
  setTheme,
  usePreferencesStore,
  type AppearancePackId,
  type ResolvedTheme,
  type Theme,
} from './theme';

export { applyTheme } from './theme';

export type UseThemeResult = {
  theme: Theme;
  resolvedTheme: ResolvedTheme;
  packId: AppearancePackId;
  lightThemeId: AppearancePackId;
  darkThemeId: AppearancePackId;
  setTheme: (theme: Theme) => void;
  chooseAppearancePack: (id: AppearancePackId) => void;
};

/**
 * 逐字段订阅而不是返回整个 state：zustand 是选择器订阅的，
 * 返回新对象会让任何一次变更都触发重渲染。
 */
export function useTheme(): UseThemeResult {
  const theme = usePreferencesStore((state) => state.theme);
  const lightThemeId = usePreferencesStore((state) => state.lightThemeId);
  const darkThemeId = usePreferencesStore((state) => state.darkThemeId);

  const choose = useCallback((id: AppearancePackId) => {
    setAppearancePack(id);
  }, []);
  const change = useCallback((next: Theme) => {
    setTheme(next);
  }, []);

  return {
    theme,
    resolvedTheme: resolveTheme(theme),
    packId: activePackId({ theme, lightThemeId, darkThemeId }),
    lightThemeId,
    darkThemeId,
    setTheme: change,
    chooseAppearancePack: choose,
  };
}
