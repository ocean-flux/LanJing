import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { getLocale, locales, setLocale as persistLocale } from '@/shared/paraglide/runtime.js';

export type AppLocale = (typeof locales)[number];

type LocaleContextValue = {
  locale: AppLocale;
  setLocale: (locale: AppLocale) => Promise<void>;
};

const LocaleContext = createContext<LocaleContextValue | undefined>(undefined);

function isAppLocale(value: string): value is AppLocale {
  return locales.includes(value as AppLocale);
}

function readLocale(): AppLocale {
  const locale = getLocale();
  return isAppLocale(locale) ? locale : locales[0];
}

export function LocaleProvider({ children }: { children: ReactNode }) {
  const [locale, setLocale] = useState<AppLocale>(readLocale);

  useEffect(() => {
    document.documentElement.lang = locale;
    document.documentElement.dir = 'ltr';
  }, [locale]);

  const updateLocale = useCallback(async (nextLocale: AppLocale) => {
    await persistLocale(nextLocale, { reload: false });
    setLocale(nextLocale);
  }, []);

  const value = useMemo(() => ({ locale, setLocale: updateLocale }), [locale, updateLocale]);

  return <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>;
}

export function useLocale() {
  const context = useContext(LocaleContext);
  if (!context) {
    throw new Error('useLocale must be used inside LocaleProvider.');
  }
  return context;
}
