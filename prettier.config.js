/** @type {import('prettier').Config & import('prettier-plugin-tailwindcss').PluginOptions} */
export default {
  // tailwind 必须放最后，才能与 prettier-plugin-svelte 共存
  plugins: ['prettier-plugin-svelte', 'prettier-plugin-tailwindcss'],
  printWidth: 100,
  tabWidth: 2,
  useTabs: false,
  semi: true,
  singleQuote: true,
  trailingComma: 'all',
  // Tailwind CSS v4：从 CSS 入口读 theme / @utility
  tailwindStylesheet: './src/index.css',
  // 项目 class 组合入口（与 $lib/utils#cn 对齐）
  tailwindFunctions: ['cn', 'clsx', 'tv', 'cva'],
  overrides: [
    {
      files: '*.svelte',
      options: {
        parser: 'svelte',
      },
    },
  ],
};
