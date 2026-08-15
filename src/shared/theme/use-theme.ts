import { useCallback, useEffect, useState } from 'react';
import {
  applyTheme,
  getAppearancePackIds,
  persistTheme,
  readTheme,
  setAppearancePack,
  subscribeToTheme,
  type AppearancePackId,
  type Theme,
} from '@/shared/theme/theme';

export function useTheme() {
  const [currentTheme, setCurrentTheme] = useState<Theme>(() => readTheme());
  const [resolvedTheme, setResolvedTheme] = useState<'light' | 'dark'>(() =>
    document.documentElement.classList.contains('dark') ? 'dark' : 'light',
  );
  const [appearancePacks, setAppearancePacks] = useState(getAppearancePackIds);

  useEffect(() => {
    setResolvedTheme(applyTheme(currentTheme));
    const unsubscribe = subscribeToTheme((nextTheme) => {
      setCurrentTheme(nextTheme);
      setAppearancePacks(getAppearancePackIds());
    });
    if (typeof window.matchMedia !== 'function') return unsubscribe;

    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const update = () => {
      if (currentTheme === 'system') setResolvedTheme(applyTheme(currentTheme));
    };
    media.addEventListener('change', update);
    return () => {
      unsubscribe();
      media.removeEventListener('change', update);
    };
  }, [currentTheme]);

  const setTheme = useCallback((next: Theme) => {
    setCurrentTheme(next);
    persistTheme(next);
    setResolvedTheme(applyTheme(next));
  }, []);

  const chooseAppearancePack = useCallback((next: AppearancePackId) => {
    setAppearancePack(next);
    setAppearancePacks(getAppearancePackIds());
  }, []);

  const toggleTheme = useCallback(() => {
    const resolved = document.documentElement.classList.contains('dark') ? 'light' : 'dark';
    setTheme(resolved);
  }, [setTheme]);

  return {
    theme: currentTheme,
    resolvedTheme,
    ...appearancePacks,
    setTheme,
    chooseAppearancePack,
    toggleTheme,
  };
}
