import js from '@eslint/js';
import { defineConfig } from 'eslint/config';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import tseslint from 'typescript-eslint';

import svelteConfig from './svelte.config.js';

export default defineConfig([
  {
    ignores: [
      '.svelte-kit/**',
      '.tmp/**',
      '.agents/**',
      '.pi/**',
      '.trellis/**',
      'build/**',
      '.opencode/**',
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
  ...svelte.configs.prettier,
  {
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
    rules: {
      'no-console': 'off',
      'no-restricted-imports': [
        'error',
        {
          paths: [
            {
              name: '@lucide/svelte',
              message:
                '请从 @lucide/svelte/icons/<icon> 单图标导入；根入口会显著拖慢 Svelte/Vitest 转换。',
            },
            {
              name: '@lucide/svelte/icons',
              message:
                '请从 @lucide/svelte/icons/<icon> 单图标导入；icons 入口仍会加载完整图标索引。',
            },
          ],
        },
      ],
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
