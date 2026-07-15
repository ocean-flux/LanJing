// i18n 统一入口：从 paraglide 编译产物重新导出消息函数与运行时 API
// 编译产物由 vite 插件 @inlang/paraglide-js 在构建/开发时生成到 ./src/lib/paraglide
//
// 用法：
//   import { m, getLocale, setLocale } from '$lib/i18n';
//   m.welcome()                       // 当前语言下的欢迎文案
//   setLocale('zh-CN')                // 切换语言（默认触发页面重载）
//   getLocale()                       // 读取当前语言
export { m } from '$lib/paraglide/messages.js';
export {
  getLocale,
  setLocale,
  getTextDirection,
  localizeHref,
  deLocalizeUrl,
} from '$lib/paraglide/runtime.js';

// 支持的语言列表，与 project.inlang/settings.json 的 locales 保持一致
export const locales = ['en', 'zh-CN'] as const;
export type Locale = (typeof locales)[number];
export const baseLocale: Locale = 'zh-CN';
