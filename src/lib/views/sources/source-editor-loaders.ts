import type { RuntimePlatform } from '$lib/app/platform-runtime';
import { resolveEditorKind, type EditorAdapter } from './source-editor-adapters';

export interface MonacoSourceEditorModule {
  createMonacoSourceEditor(): EditorAdapter;
}

export interface CodeMirrorSourceEditorModule {
  createCodeMirrorSourceEditor(): EditorAdapter;
}

export interface SourceEditorModuleImports {
  readonly monaco?: () => Promise<MonacoSourceEditorModule>;
  readonly codemirror?: () => Promise<CodeMirrorSourceEditorModule>;
}

export type SourceEditorAdapterLoader = (
  platform: RuntimePlatform,
) => Promise<EditorAdapter | null>;

export function createSourceEditorAdapterLoader(
  moduleImports: SourceEditorModuleImports = {},
): SourceEditorAdapterLoader {
  return async (platform) => {
    switch (resolveEditorKind(platform)) {
      case 'monaco': {
        // This platform-only chunk must stay lazy so mobile never requests Monaco.
        const module = moduleImports.monaco
          ? await moduleImports.monaco()
          : await import('./monaco-source-editor');
        return module.createMonacoSourceEditor();
      }
      case 'codemirror': {
        // This platform-only chunk must stay lazy so desktop never requests CodeMirror.
        const module = moduleImports.codemirror
          ? await moduleImports.codemirror()
          : await import('./codemirror-source-editor');
        return module.createCodeMirrorSourceEditor();
      }
      case 'fallback':
        return null;
    }
  };
}

export const loadSourceEditorAdapter = createSourceEditorAdapterLoader();
