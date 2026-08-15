declare module '@/shared/paraglide/runtime.js' {
  export const locales: readonly ['en', 'zh-CN'];
  export function getLocale(): string;
  export function setLocale(
    locale: (typeof locales)[number],
    options?: { reload?: boolean },
  ): void | Promise<void>;
}

declare module '@/shared/paraglide/messages.js' {
  type MessageInputs = Record<string, string | number>;
  type Message = (inputs?: MessageInputs, options?: { locale?: string }) => string;
  export const m: Record<string, Message>;
}
