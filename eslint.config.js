import js from '@eslint/js';
import { defineConfig } from 'eslint/config';
import eslintConfigPrettier from 'eslint-config-prettier';
import svelte from 'eslint-plugin-svelte';
import tailwindcss from 'eslint-plugin-tailwindcss';
import globals from 'globals';
import tseslint from 'typescript-eslint';

import svelteConfig from './svelte.config.js';

const tailwindRecommended = tailwindcss.configs.recommended;

export default defineConfig([
  {
    ignores: [
      '.svelte-kit/**',
      '.tmp/**',
      '.pi/**',
      '.trellis/**',
      '.pi-subagents/**',
      '.agents/**',
      '.playwright-mcp/**',
      '.omp/**',
      'build/**',
      '.vscode/**',
      'coverage/**',
      'dist/**',
      'node_modules/**',
      'src-tauri/**',
      // paraglide 编译产物，由 @inlang/paraglide-js 自动生成
      'src/lib/paraglide/**',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...svelte.configs.recommended,
  // 关闭与 Prettier 冲突的 ESLint 格式规则
  ...svelte.configs.prettier,
  eslintConfigPrettier,
  // Tailwind CSS v4：合并 recommended，规则升为 error，并保证 plugins 同对象
  {
    ...tailwindRecommended,
    settings: {
      ...tailwindRecommended.settings,
      tailwindcss: {
        cssConfigPath: './src/index.css',
        attributes: ['class', 'className'],
        functions: ['cn', 'clsx', 'tv', 'cva', 'classnames', 'classNames', 'twMerge', 'twJoin'],
      },
    },
    rules: {
      ...tailwindRecommended.rules,
      // class 顺序交给 prettier-plugin-tailwindcss
      'tailwindcss/classnames-order': 'off',
      // 其余全部 error（覆盖 recommended 的 warn）
      'tailwindcss/enforces-negative-arbitrary-values': 'error',
      'tailwindcss/enforces-shorthand': 'error',
      'tailwindcss/important-modifier-suffix': 'error',
      'tailwindcss/no-contradicting-classname': 'error',
      'tailwindcss/no-custom-classname': 'error',
      'tailwindcss/no-unnecessary-arbitrary-value': 'error',
      // 默认关闭：项目允许合理 arbitrary value
      'tailwindcss/no-arbitrary-value': 'off',
    },
  },
  {
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
    rules: {
      'no-console': 'off',
    },
  },
  {
    files: ['**/*.svelte', '**/*.svelte.js', '**/*.svelte.ts'],
    languageOptions: {
      parserOptions: {
        extraFileExtensions: ['.svelte'],
        parser: tseslint.parser,
        projectService: true,
        svelteConfig,
      },
    },
  },
]);
