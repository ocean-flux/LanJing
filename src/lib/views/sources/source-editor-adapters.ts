import type { RuntimePlatform } from '$lib/app/platform-runtime';
import type {
  SourceLanguageMethod,
  SourceLanguageRequestWithoutText,
  SourceLanguageResultMap,
} from './source-language-service-protocol';

export type EditorKind = 'monaco' | 'codemirror' | 'fallback';
export type EditorCommand = 'undo' | 'redo' | 'format';

export interface EditorSelection {
  readonly anchor: number;
  readonly head: number;
}

export interface EditorAdapterMount {
  readonly container: HTMLElement;
  readonly documentId: string;
  readonly text: string;
  readonly modelVersion: number;
  readonly onTextChange: (text: string, modelVersion: number) => void;
  readonly onSelectionChange?: (selection: EditorSelection) => void;
  readonly onRuntimeError: (error: unknown) => void;
  readonly requestLanguage: <M extends SourceLanguageMethod>(
    request: SourceLanguageRequestWithoutText<M>,
    options?: { readonly signal?: AbortSignal },
  ) => Promise<SourceLanguageResultMap[M]>;
}

export interface EditorAdapter {
  readonly kind: EditorKind;
  mount(options: EditorAdapterMount): void;
  setDocument(documentId: string, text: string, modelVersion: number): void;
  focus(): void;
  runCommand(command: EditorCommand): boolean | Promise<boolean>;
  dispose(): void;
}

export function resolveEditorKind(platform: RuntimePlatform): EditorKind {
  switch (platform) {
    case 'windows':
    case 'macos':
    case 'linux':
      return 'monaco';
    case 'ios':
    case 'android':
      return 'codemirror';
    case 'unknown':
      return 'fallback';
  }
}
