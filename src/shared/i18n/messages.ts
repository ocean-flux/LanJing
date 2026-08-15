import { m as paraglideMessages } from '@/shared/paraglide/messages.js';
import { useLocale } from '@/shared/i18n/locale';

export { m } from '@/shared/paraglide/messages.js';
export { LocaleProvider, useLocale, type AppLocale } from '@/shared/i18n/locale';

export function useMessages() {
  useLocale();
  return paraglideMessages;
}
