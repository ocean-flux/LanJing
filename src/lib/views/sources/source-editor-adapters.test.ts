import { closeCompletion, currentCompletions, startCompletion } from '@codemirror/autocomplete';
import { diagnosticCount, forceLinting, forEachDiagnostic } from '@codemirror/lint';
import { activateHover, EditorView, keymap } from '@codemirror/view';
import { describe, expect, it, vi, type Mock } from 'vitest';
import { createCodeMirrorSourceEditor } from './codemirror-source-editor';
import {
  resolveEditorKind,
  type EditorAdapter,
  type EditorAdapterMount,
  type EditorKind,
} from './source-editor-adapters';
import { createSourceEditorAdapterLoader } from './source-editor-loaders';

interface MonacoDisposableHarness {
  readonly dispose: Mock<() => void>;
}

interface MonacoPositionHarness {
  readonly lineNumber: number;
  readonly column: number;
}

interface MonacoRangeHarness {
  readonly startLineNumber: number;
  readonly startColumn: number;
  readonly endLineNumber: number;
  readonly endColumn: number;
}

interface MonacoMarkerHarness extends MonacoRangeHarness {
  readonly code?: string;
  readonly severity: number;
  readonly message: string;
  readonly source?: string;
  readonly modelVersionId?: number;
}

interface MonacoModelHarness {
  readonly uri: object;
  value: string;
  getValue(): string;
  setValue(value: string): void;
  onDidChangeContent(listener: () => void): MonacoDisposableHarness;
  getOffsetAt(position: MonacoPositionHarness): number;
  getPositionAt(offset: number): MonacoPositionHarness;
  getFullModelRange(): MonacoRangeHarness;
  getOptions(): { readonly tabSize: number; readonly insertSpaces: boolean };
  getVersionId(): number;
  readonly dispose: Mock<() => void>;
  readonly listenerDisposables: MonacoDisposableHarness[];
}

interface MonacoEditorHarness {
  readonly dispose: Mock<() => void>;
  readonly trigger: Mock<(source: string, command: string, payload: unknown) => void>;
  readonly pushUndoStop: Mock<() => boolean>;
  readonly executeEdits: Mock<
    (
      source: string,
      edits: Array<{ readonly range: MonacoRangeHarness; readonly text: string }>,
    ) => boolean
  >;
  readonly listenerDisposables: MonacoDisposableHarness[];
  emitSelection(anchorColumn: number, headColumn: number): void;
  startComposition(): void;
  endComposition(): void;
}

interface MonacoCancellationTokenHarness {
  readonly isCancellationRequested: boolean;
  readonly listenerDisposables: MonacoDisposableHarness[];
  onCancellationRequested(listener: () => void): MonacoDisposableHarness;
}

interface MonacoMarkdownHarness {
  readonly value: string;
  readonly isTrusted?: boolean;
  readonly supportHtml?: boolean;
  readonly supportThemeIcons?: boolean;
}

interface MonacoCompletionItemHarness {
  readonly label: string;
  readonly kind: number;
  readonly insertText: string;
  readonly insertTextRules?: number;
  readonly range: MonacoRangeHarness | { insert: MonacoRangeHarness; replace: MonacoRangeHarness };
  readonly detail?: string;
  readonly documentation?: string | MonacoMarkdownHarness;
}

interface MonacoCompletionProviderHarness {
  provideCompletionItems(
    model: MonacoModelHarness,
    position: MonacoPositionHarness,
    context: object,
    token: MonacoCancellationTokenHarness,
  ): Promise<{
    readonly suggestions: MonacoCompletionItemHarness[];
    readonly incomplete?: boolean;
  }>;
  resolveCompletionItem(
    item: MonacoCompletionItemHarness,
    token: MonacoCancellationTokenHarness,
  ): Promise<MonacoCompletionItemHarness>;
}

interface MonacoHoverProviderHarness {
  provideHover(
    model: MonacoModelHarness,
    position: MonacoPositionHarness,
    token: MonacoCancellationTokenHarness,
  ): Promise<{
    readonly contents: MonacoMarkdownHarness[];
    readonly range?: MonacoRangeHarness;
  } | null>;
}

interface MonacoSymbolHarness {
  readonly name: string;
  readonly detail: string;
  readonly kind: number;
  readonly tags: readonly number[];
  readonly range: MonacoRangeHarness;
  readonly selectionRange: MonacoRangeHarness;
  readonly children?: MonacoSymbolHarness[];
}

interface MonacoDocumentSymbolProviderHarness {
  provideDocumentSymbols(
    model: MonacoModelHarness,
    token: MonacoCancellationTokenHarness,
  ): Promise<MonacoSymbolHarness[]>;
}

interface MonacoFormattingProviderHarness {
  provideDocumentFormattingEdits(
    model: MonacoModelHarness,
    options: { readonly tabSize: number; readonly insertSpaces: boolean },
    token: MonacoCancellationTokenHarness,
  ): Promise<Array<{ readonly range: MonacoRangeHarness; readonly text: string }>>;
}

interface MonacoFoldingProviderHarness {
  provideFoldingRanges(
    model: MonacoModelHarness,
    context: object,
    token: MonacoCancellationTokenHarness,
  ): Promise<Array<{ readonly start: number; readonly end: number; readonly kind?: object }>>;
}

interface MonacoSelectionProviderHarness {
  provideSelectionRanges(
    model: MonacoModelHarness,
    positions: MonacoPositionHarness[],
    token: MonacoCancellationTokenHarness,
  ): Promise<Array<Array<{ readonly range: MonacoRangeHarness }>>>;
}

const monacoHarness = vi.hoisted(() => {
  const models: MonacoModelHarness[] = [];
  const editors: MonacoEditorHarness[] = [];
  const languageDisposables: MonacoDisposableHarness[] = [];
  const semanticProviderDisposables: MonacoDisposableHarness[] = [];
  const markerUpdates: Array<{
    readonly model: MonacoModelHarness;
    readonly owner: string;
    readonly markers: readonly MonacoMarkerHarness[];
  }> = [];
  const semanticProviders = {
    completion: [] as MonacoCompletionProviderHarness[],
    hover: [] as MonacoHoverProviderHarness[],
    symbols: [] as MonacoDocumentSymbolProviderHarness[],
    formatting: [] as MonacoFormattingProviderHarness[],
    folding: [] as MonacoFoldingProviderHarness[],
    selection: [] as MonacoSelectionProviderHarness[],
  };

  const createLanguageDisposable = (): MonacoDisposableHarness => {
    const disposable = { dispose: vi.fn<() => void>() };
    languageDisposables.push(disposable);
    return disposable;
  };

  const registerSemanticProvider = <T>(providers: T[]) =>
    vi.fn((_languageId: string, provider: T) => {
      providers.push(provider);
      const disposable = { dispose: vi.fn<() => void>() };
      semanticProviderDisposables.push(disposable);
      return disposable;
    });

  const createCancellationToken = (): MonacoCancellationTokenHarness => {
    const listeners = new Set<() => void>();
    const listenerDisposables: MonacoDisposableHarness[] = [];
    return {
      isCancellationRequested: false,
      listenerDisposables,
      onCancellationRequested(listener: () => void) {
        listeners.add(listener);
        const disposable = {
          dispose: vi.fn(() => {
            listeners.delete(listener);
          }),
        };
        listenerDisposables.push(disposable);
        return disposable;
      },
    };
  };

  const createModel = (
    initialText: string,
    _languageId?: string,
    uri: object = {},
  ): MonacoModelHarness => {
    const contentListeners = new Set<() => void>();
    const listenerDisposables: MonacoDisposableHarness[] = [];
    let versionId = 1;
    const lineStarts = (): number[] => {
      const starts = [0];
      for (let index = 0; index < model.value.length; index += 1) {
        if (model.value.charCodeAt(index) === 10) starts.push(index + 1);
      }
      return starts;
    };
    const model: MonacoModelHarness = {
      uri,
      value: initialText,
      getValue: () => model.value,
      setValue(value: string) {
        model.value = value;
        versionId += 1;
        for (const listener of contentListeners) listener();
      },
      onDidChangeContent(listener: () => void) {
        contentListeners.add(listener);
        const disposable = {
          dispose: vi.fn(() => {
            contentListeners.delete(listener);
          }),
        };
        listenerDisposables.push(disposable);
        return disposable;
      },
      getOffsetAt(position: MonacoPositionHarness) {
        const starts = lineStarts();
        const lineIndex = Math.min(Math.max(0, position.lineNumber - 1), starts.length - 1);
        const start = starts[lineIndex] ?? 0;
        const nextStart = starts[lineIndex + 1] ?? model.value.length + 1;
        return Math.min(nextStart - 1, start + Math.max(0, position.column - 1));
      },
      getPositionAt(offset: number) {
        const normalized = Math.min(Math.max(0, offset), model.value.length);
        const starts = lineStarts();
        let lineIndex = starts.length - 1;
        while (lineIndex > 0 && (starts[lineIndex] ?? 0) > normalized) lineIndex -= 1;
        return {
          lineNumber: lineIndex + 1,
          column: normalized - (starts[lineIndex] ?? 0) + 1,
        };
      },
      getFullModelRange() {
        const end = model.getPositionAt(model.value.length);
        return {
          startLineNumber: 1,
          startColumn: 1,
          endLineNumber: end.lineNumber,
          endColumn: end.column,
        };
      },
      getOptions: () => ({ tabSize: 2, insertSpaces: true }),
      getVersionId: () => versionId,
      dispose: vi.fn<() => void>(),
      listenerDisposables,
    };
    models.push(model);
    return model;
  };

  const createEditor = (_container: HTMLElement, options: { model: MonacoModelHarness }) => {
    let model = options.model;
    let selectionListener:
      | ((event: {
          selection: {
            selectionStartLineNumber: number;
            selectionStartColumn: number;
            positionLineNumber: number;
            positionColumn: number;
          };
        }) => void)
      | null = null;
    let compositionStartListener: (() => void) | null = null;
    let compositionEndListener: (() => void) | null = null;
    const listenerDisposables: MonacoDisposableHarness[] = [];
    const subscribe = (cleanup: () => void) => {
      const disposable = { dispose: vi.fn(cleanup) };
      listenerDisposables.push(disposable);
      return disposable;
    };
    const editor: MonacoEditorHarness & {
      onDidChangeCursorSelection(
        listener: NonNullable<typeof selectionListener>,
      ): MonacoDisposableHarness;
      onDidCompositionStart(listener: () => void): MonacoDisposableHarness;
      onDidCompositionEnd(listener: () => void): MonacoDisposableHarness;
      setModel(nextModel: MonacoModelHarness): void;
      focus: Mock<() => void>;
    } = {
      onDidChangeCursorSelection(listener: NonNullable<typeof selectionListener>) {
        selectionListener = listener;
        return subscribe(() => {
          if (selectionListener === listener) selectionListener = null;
        });
      },
      onDidCompositionStart(listener: () => void) {
        compositionStartListener = listener;
        return subscribe(() => {
          if (compositionStartListener === listener) compositionStartListener = null;
        });
      },
      onDidCompositionEnd(listener: () => void) {
        compositionEndListener = listener;
        return subscribe(() => {
          if (compositionEndListener === listener) compositionEndListener = null;
        });
      },
      setModel(nextModel: MonacoModelHarness) {
        model = nextModel;
      },
      focus: vi.fn(),
      trigger: vi.fn<(source: string, command: string, payload: unknown) => void>(),
      pushUndoStop: vi.fn(() => true),
      executeEdits: vi.fn(
        (
          _source: string,
          edits: Array<{ readonly range: MonacoRangeHarness; readonly text: string }>,
        ) => {
          const replacements = edits
            .map((edit) => ({
              start: model.getOffsetAt({
                lineNumber: edit.range.startLineNumber,
                column: edit.range.startColumn,
              }),
              end: model.getOffsetAt({
                lineNumber: edit.range.endLineNumber,
                column: edit.range.endColumn,
              }),
              text: edit.text,
            }))
            .sort((left, right) => right.start - left.start);
          let nextValue = model.value;
          for (const replacement of replacements) {
            nextValue = `${nextValue.slice(0, replacement.start)}${replacement.text}${nextValue.slice(replacement.end)}`;
          }
          if (replacements.length > 0) model.setValue(nextValue);
          return replacements.length > 0;
        },
      ),
      dispose: vi.fn<() => void>(),
      listenerDisposables,
      emitSelection(anchorColumn: number, headColumn: number) {
        selectionListener?.({
          selection: {
            selectionStartLineNumber: 1,
            selectionStartColumn: anchorColumn,
            positionLineNumber: 1,
            positionColumn: headColumn,
          },
        });
      },
      startComposition() {
        compositionStartListener?.();
      },
      endComposition() {
        compositionEndListener?.();
      },
    };
    editors.push(editor);
    return editor;
  };

  return {
    models,
    editors,
    languageDisposables,
    semanticProviderDisposables,
    semanticProviders,
    markerUpdates,
    createCancellationToken,
    module: {
      Uri: { from: vi.fn((value) => value) },
      MarkerSeverity: { Hint: 1, Info: 2, Warning: 4, Error: 8 },
      languages: {
        register: vi.fn(),
        setMonarchTokensProvider: vi.fn(createLanguageDisposable),
        setLanguageConfiguration: vi.fn(createLanguageDisposable),
        CompletionItemKind: {
          Method: 0,
          Function: 1,
          Constructor: 2,
          Field: 3,
          Variable: 4,
          Class: 5,
          Struct: 6,
          Interface: 7,
          Module: 8,
          Property: 9,
          Event: 10,
          Operator: 11,
          Unit: 12,
          Value: 13,
          Constant: 14,
          Enum: 15,
          EnumMember: 16,
          Keyword: 17,
          Text: 18,
          Color: 19,
          File: 20,
          Reference: 21,
          Folder: 23,
          TypeParameter: 24,
          Snippet: 28,
        },
        CompletionItemInsertTextRule: { None: 0, KeepWhitespace: 1, InsertAsSnippet: 4 },
        CompletionItemTag: { Deprecated: 1 },
        SymbolKind: {
          File: 0,
          Module: 1,
          Namespace: 2,
          Package: 3,
          Class: 4,
          Method: 5,
          Property: 6,
          Field: 7,
          Constructor: 8,
          Enum: 9,
          Interface: 10,
          Function: 11,
          Variable: 12,
          Constant: 13,
          String: 14,
          Number: 15,
          Boolean: 16,
          Array: 17,
          Object: 18,
          Key: 19,
          Null: 20,
          EnumMember: 21,
          Struct: 22,
          Event: 23,
          Operator: 24,
          TypeParameter: 25,
        },
        SymbolTag: { Deprecated: 1 },
        FoldingRangeKind: {
          Comment: { value: 'comment' },
          Imports: { value: 'imports' },
          Region: { value: 'region' },
        },
        registerCompletionItemProvider: registerSemanticProvider(semanticProviders.completion),
        registerHoverProvider: registerSemanticProvider(semanticProviders.hover),
        registerDocumentSymbolProvider: registerSemanticProvider(semanticProviders.symbols),
        registerDocumentFormattingEditProvider: registerSemanticProvider(
          semanticProviders.formatting,
        ),
        registerFoldingRangeProvider: registerSemanticProvider(semanticProviders.folding),
        registerSelectionRangeProvider: registerSemanticProvider(semanticProviders.selection),
      },
      editor: {
        createModel: vi.fn(createModel),
        create: vi.fn(createEditor),
        setModelMarkers: vi.fn(
          (model: MonacoModelHarness, owner: string, markers: MonacoMarkerHarness[]) => {
            markerUpdates.push({ model, owner, markers });
          },
        ),
      },
    },
  };
});

vi.mock('monaco-editor/editor', () => monacoHarness.module);
vi.mock('monaco-editor/editor/editor.worker?worker', () => ({
  default: class MockEditorWorker {},
}));

class LoaderAdapter implements EditorAdapter {
  readonly kind: EditorKind;

  constructor(kind: EditorKind) {
    this.kind = kind;
  }

  mount(options: EditorAdapterMount): void {
    void options;
  }
  setDocument(documentId: string, text: string, modelVersion: number): void {
    void documentId;
    void text;
    void modelVersion;
  }
  focus(): void {
    return;
  }
  runCommand(): boolean {
    return true;
  }
  dispose(): void {
    return;
  }
}

describe('source editor platform resolution', () => {
  it.each([
    ['windows', 'monaco'],
    ['macos', 'monaco'],
    ['linux', 'monaco'],
    ['ios', 'codemirror'],
    ['android', 'codemirror'],
    ['unknown', 'fallback'],
  ] as const)('maps %s directly to %s', (platform, kind) => {
    expect(resolveEditorKind(platform)).toBe(kind);
  });

  it.each([
    ['windows', 'monaco'],
    ['macos', 'monaco'],
    ['linux', 'monaco'],
  ] as const)('loads only Monaco for desktop platform %s', async (platform, kind) => {
    const importMonaco = vi.fn(async () => ({
      createMonacoSourceEditor: () => new LoaderAdapter('monaco'),
    }));
    const importCodeMirror = vi.fn(async () => ({
      createCodeMirrorSourceEditor: () => new LoaderAdapter('codemirror'),
    }));
    const load = createSourceEditorAdapterLoader({
      monaco: importMonaco,
      codemirror: importCodeMirror,
    });

    await expect(load(platform)).resolves.toMatchObject({ kind });
    expect(importMonaco).toHaveBeenCalledOnce();
    expect(importCodeMirror).not.toHaveBeenCalled();
  });

  it.each([
    ['ios', 'codemirror'],
    ['android', 'codemirror'],
  ] as const)('loads only CodeMirror for mobile platform %s', async (platform, kind) => {
    const importMonaco = vi.fn(async () => ({
      createMonacoSourceEditor: () => new LoaderAdapter('monaco'),
    }));
    const importCodeMirror = vi.fn(async () => ({
      createCodeMirrorSourceEditor: () => new LoaderAdapter('codemirror'),
    }));
    const load = createSourceEditorAdapterLoader({
      monaco: importMonaco,
      codemirror: importCodeMirror,
    });

    await expect(load(platform)).resolves.toMatchObject({ kind });
    expect(importMonaco).not.toHaveBeenCalled();
    expect(importCodeMirror).toHaveBeenCalledOnce();
  });

  it('keeps unknown platforms on fallback without requesting either advanced module', async () => {
    const importMonaco = vi.fn(async () => ({
      createMonacoSourceEditor: () => new LoaderAdapter('monaco'),
    }));
    const importCodeMirror = vi.fn(async () => ({
      createCodeMirrorSourceEditor: () => new LoaderAdapter('codemirror'),
    }));
    const load = createSourceEditorAdapterLoader({
      monaco: importMonaco,
      codemirror: importCodeMirror,
    });

    await expect(load('unknown')).resolves.toBeNull();
    expect(importMonaco).not.toHaveBeenCalled();
    expect(importCodeMirror).not.toHaveBeenCalled();
  });
});

describe('Monaco source editor adapter', () => {
  it('disposes markers, providers, model, editor, and every listener it creates', async () => {
    // This test intentionally loads the platform chunk only after its Monaco boundary is mocked.
    const { createMonacoSourceEditor } = await import('./monaco-source-editor');
    const priorLanguageDisposableCount = monacoHarness.languageDisposables.length;
    const priorProviderDisposableCount = monacoHarness.semanticProviderDisposables.length;
    const textChanges: Array<{ text: string; modelVersion: number }> = [];
    const adapter = createMonacoSourceEditor();
    adapter.mount({
      container: document.createElement('div'),
      documentId: 'document:desktop',
      text: '{}',
      modelVersion: 8,
      requestLanguage: vi.fn(async () => []) as unknown as EditorAdapterMount['requestLanguage'],
      onTextChange: (text, modelVersion) => textChanges.push({ text, modelVersion }),
      onRuntimeError: (error) => {
        throw error;
      },
    });

    const model = monacoHarness.models.at(-1)!;
    const editor = monacoHarness.editors.at(-1)!;
    model.setValue('{"desktop":true}');
    expect(textChanges).toEqual([{ text: '{"desktop":true}', modelVersion: 9 }]);

    adapter.dispose();
    expect(model.dispose).toHaveBeenCalledOnce();
    expect(editor.dispose).toHaveBeenCalledOnce();
    expect(model.listenerDisposables).toHaveLength(1);
    expect(model.listenerDisposables[0]?.dispose).toHaveBeenCalledOnce();
    expect(editor.listenerDisposables).toHaveLength(3);
    for (const listener of editor.listenerDisposables) {
      expect(listener.dispose).toHaveBeenCalledOnce();
    }
    const providerDisposables = monacoHarness.semanticProviderDisposables.slice(
      priorProviderDisposableCount,
    );
    expect(providerDisposables).toHaveLength(6);
    for (const disposable of providerDisposables) {
      expect(disposable.dispose).toHaveBeenCalledOnce();
    }
    const languageDisposables = monacoHarness.languageDisposables.slice(
      priorLanguageDisposableCount,
    );
    expect(languageDisposables).toHaveLength(2);
    for (const disposable of languageDisposables) {
      expect(disposable.dispose).toHaveBeenCalledOnce();
    }
    expect(monacoHarness.markerUpdates.at(-1)).toEqual({
      model,
      owner: 'lanjing-source-language-service',
      markers: [],
    });

    model.setValue('{"stale":true}');
    expect(textChanges).toHaveLength(1);
  });

  it('coalesces desktop IME, reports selection, and keeps native undo groups for worker format edits', async () => {
    // Dynamic import intentionally exercises the desktop-only module boundary after mocking Monaco.
    const { createMonacoSourceEditor } = await import('./monaco-source-editor');
    const textChanges: Array<{ text: string; modelVersion: number }> = [];
    const selections: Array<{ anchor: number; head: number }> = [];
    const requestLanguage = vi.fn(async (request: { readonly method: string }) => {
      if (request.method === 'format') {
        return [
          {
            range: {
              start: { line: 0, character: 0 },
              end: { line: 0, character: 14 },
            },
            newText: '{\n  "value": "你好"\n}',
          },
        ];
      }
      return [];
    }) as unknown as EditorAdapterMount['requestLanguage'];
    const adapter = createMonacoSourceEditor();
    adapter.mount({
      container: document.createElement('div'),
      documentId: 'document:desktop-ime',
      text: '{"value":1}',
      modelVersion: 2,
      requestLanguage,
      onTextChange: (text, modelVersion) => textChanges.push({ text, modelVersion }),
      onSelectionChange: (selection) => selections.push(selection),
      onRuntimeError: (error) => {
        throw error;
      },
    });

    const model = monacoHarness.models.at(-1)!;
    const editor = monacoHarness.editors.at(-1)!;
    editor.startComposition();
    model.setValue('{"value":"你"}');
    model.setValue('{"value":"你好"}');
    expect(textChanges).toEqual([]);
    editor.endComposition();
    expect(textChanges).toEqual([{ text: '{"value":"你好"}', modelVersion: 3 }]);

    editor.emitSelection(2, 5);
    expect(selections).toEqual([{ anchor: 1, head: 4 }]);
    await expect(adapter.runCommand('undo')).resolves.toBe(true);
    await expect(adapter.runCommand('redo')).resolves.toBe(true);
    expect(editor.trigger).toHaveBeenNthCalledWith(1, 'lanjing-source-editor', 'undo', null);
    expect(editor.trigger).toHaveBeenNthCalledWith(2, 'lanjing-source-editor', 'redo', null);

    await expect(adapter.runCommand('format')).resolves.toBe(true);
    expect(editor.pushUndoStop).toHaveBeenCalledTimes(2);
    expect(editor.executeEdits).toHaveBeenCalledOnce();
    expect(textChanges.at(-1)).toEqual({
      modelVersion: 4,
      text: '{\n  "value": "你好"\n}',
    });
    expect(requestLanguage).toHaveBeenCalledWith(
      {
        method: 'format',
        payload: { options: { tabSize: 2, insertSpaces: true } },
      },
      expect.objectContaining({ signal: expect.anything() }),
    );
    adapter.dispose();
  });

  it('rejects overlapping format edits before they can corrupt the Monaco model', async () => {
    // Dynamic import intentionally exercises the desktop-only module after the Monaco mock.
    const { createMonacoSourceEditor } = await import('./monaco-source-editor');
    const runtimeErrors: unknown[] = [];
    const requestLanguage = vi.fn(async (request: { readonly method: string }) => {
      if (request.method !== 'format') return [];
      return [
        {
          range: {
            start: { line: 0, character: 0 },
            end: { line: 0, character: 4 },
          },
          newText: '{}',
        },
        {
          range: {
            start: { line: 0, character: 2 },
            end: { line: 0, character: 6 },
          },
          newText: '[]',
        },
      ];
    }) as unknown as EditorAdapterMount['requestLanguage'];
    const adapter = createMonacoSourceEditor();
    adapter.mount({
      container: document.createElement('div'),
      documentId: 'document:overlapping-format',
      text: '{"value":1}',
      modelVersion: 1,
      requestLanguage,
      onTextChange: vi.fn(),
      onRuntimeError: (error) => runtimeErrors.push(error),
    });

    const model = monacoHarness.models.at(-1)!;
    const editor = monacoHarness.editors.at(-1)!;
    await expect(adapter.runCommand('format')).rejects.toThrow(
      'Source language format returned invalid text edits',
    );
    expect(model.value).toBe('{"value":1}');
    expect(editor.executeEdits).not.toHaveBeenCalled();
    expect(runtimeErrors).toHaveLength(1);
    adapter.dispose();
  });

  it('maps every host semantic result through Monaco native providers', async () => {
    const { createMonacoSourceEditor } = await import('./monaco-source-editor');
    const text = '{\n  "😀": tru\n}';
    const unsafeMarkdown =
      '![remote](https://invalid.test/pixel.png) <img src="https://invalid.test/pixel.png"> [run](command:open)';
    const requestLanguage = vi.fn(
      async (request: { readonly method: string; readonly payload: Record<string, unknown> }) => {
        switch (request.method) {
          case 'validation':
            return [
              {
                source: 'json-language-service',
                severity: 'error',
                code: 519,
                message: 'Expected value',
                path: '/😀',
                offset: 5,
                length: 2,
                range: {
                  start: { line: 1, character: 3 },
                  end: { line: 1, character: 5 },
                },
              },
            ];
          case 'completion':
            return {
              isIncomplete: false,
              items: [
                {
                  label: 'true',
                  kind: 14,
                  insertTextFormat: 2,
                  textEdit: {
                    range: {
                      start: { line: 1, character: 8 },
                      end: { line: 1, character: 11 },
                    },
                    newText: '${1:true}',
                  },
                  data: { catalog: 'boolean' },
                },
                {
                  label: 'unsafe-clipboard',
                  kind: 15,
                  insertText: '${CLIPBOARD}',
                  insertTextFormat: 2,
                },
              ],
            };
          case 'completionResolve': {
            const payload = request.payload as { readonly item: Record<string, unknown> };
            return {
              ...payload.item,
              detail: 'boolean',
              documentation: { kind: 'markdown', value: `${unsafeMarkdown} **Boolean value**` },
            };
          }
          case 'hover':
            return {
              contents: [{ language: 'json', value: 'true' }, unsafeMarkdown],
              range: {
                start: { line: 1, character: 8 },
                end: { line: 1, character: 11 },
              },
            };
          case 'symbols':
            return [
              {
                name: 'source',
                kind: 19,
                range: {
                  start: { line: 0, character: 0 },
                  end: { line: 2, character: 1 },
                },
                selectionRange: {
                  start: { line: 0, character: 0 },
                  end: { line: 0, character: 1 },
                },
                children: [
                  {
                    name: '😀',
                    detail: 'boolean',
                    kind: 7,
                    range: {
                      start: { line: 1, character: 2 },
                      end: { line: 1, character: 11 },
                    },
                    selectionRange: {
                      start: { line: 1, character: 2 },
                      end: { line: 1, character: 6 },
                    },
                  },
                ],
              },
            ];
          case 'format':
            return [
              {
                range: {
                  start: { line: 1, character: 8 },
                  end: { line: 1, character: 11 },
                },
                newText: 'true',
              },
            ];
          case 'folding':
            return [{ startLine: 0, endLine: 2, kind: 'region', collapsedText: '{…}' }];
          case 'selectionRanges':
            return [
              {
                range: {
                  start: { line: 1, character: 8 },
                  end: { line: 1, character: 11 },
                },
                parent: {
                  range: {
                    start: { line: 1, character: 2 },
                    end: { line: 1, character: 11 },
                  },
                },
              },
            ];
          default:
            throw new Error(`Unexpected semantic method: ${request.method}`);
        }
      },
    ) as unknown as EditorAdapterMount['requestLanguage'];
    const adapter = createMonacoSourceEditor();
    adapter.mount({
      container: document.createElement('div'),
      documentId: 'document:semantic',
      text,
      modelVersion: 3,
      requestLanguage,
      onTextChange: vi.fn(),
      onRuntimeError: (error) => {
        throw error;
      },
    });

    const model = monacoHarness.models.at(-1)!;
    await vi.waitFor(() => {
      expect(
        monacoHarness.markerUpdates.some(
          (update) => update.model === model && update.markers.length === 1,
        ),
      ).toBe(true);
    });
    const markerUpdate = monacoHarness.markerUpdates.find(
      (update) => update.model === model && update.markers.length === 1,
    );
    expect(markerUpdate).toEqual({
      model,
      owner: 'lanjing-source-language-service',
      markers: [
        expect.objectContaining({
          code: '519',
          severity: 8,
          startLineNumber: 2,
          startColumn: 4,
          endLineNumber: 2,
          endColumn: 6,
        }),
      ],
    });

    const token = monacoHarness.createCancellationToken();
    const completionProvider = monacoHarness.semanticProviders.completion.at(-1)!;
    const completion = await completionProvider.provideCompletionItems(
      model,
      { lineNumber: 2, column: 12 },
      {},
      token,
    );
    expect(completion).toEqual({
      suggestions: [
        expect.objectContaining({
          label: 'true',
          kind: 17,
          insertText: '${1:true}',
          insertTextRules: 4,
          range: {
            startLineNumber: 2,
            startColumn: 9,
            endLineNumber: 2,
            endColumn: 12,
          },
        }),
      ],
      incomplete: false,
    });
    expect(requestLanguage).toHaveBeenCalledWith(
      { method: 'completion', payload: { position: { line: 1, character: 11 } } },
      expect.objectContaining({ signal: expect.anything() }),
    );

    const resolved = await completionProvider.resolveCompletionItem(
      completion.suggestions[0]!,
      token,
    );
    expect(resolved).toEqual(expect.objectContaining({ detail: 'boolean' }));
    const resolvedDocumentation = resolved.documentation;
    if (!resolvedDocumentation || typeof resolvedDocumentation === 'string') {
      throw new Error('expected safe Monaco completion documentation');
    }
    expect(resolvedDocumentation).toMatchObject({
      isTrusted: false,
      supportHtml: false,
      supportThemeIcons: false,
    });
    expect(resolvedDocumentation.value).toContain('Boolean value');
    expect(resolvedDocumentation.value).not.toContain('![');
    expect(resolvedDocumentation.value).not.toContain('https://');
    expect(resolvedDocumentation.value).not.toContain('command:');

    const hover = await monacoHarness.semanticProviders.hover
      .at(-1)!
      .provideHover(model, { lineNumber: 2, column: 10 }, token);
    expect(hover?.range).toEqual({
      startLineNumber: 2,
      startColumn: 9,
      endLineNumber: 2,
      endColumn: 12,
    });
    expect(hover?.contents[0]).toEqual({
      value: '```json\ntrue\n```',
      isTrusted: false,
      supportHtml: false,
      supportThemeIcons: false,
    });
    const hoverDocumentation = hover?.contents[1];
    expect(hoverDocumentation).toMatchObject({
      isTrusted: false,
      supportHtml: false,
      supportThemeIcons: false,
    });
    expect(hoverDocumentation?.value).not.toContain('![');
    expect(hoverDocumentation?.value).not.toContain('https://');
    expect(hoverDocumentation?.value).not.toContain('command:');

    const symbols = await monacoHarness.semanticProviders.symbols
      .at(-1)!
      .provideDocumentSymbols(model, token);
    expect(symbols).toEqual([
      expect.objectContaining({
        name: 'source',
        detail: '',
        kind: 18,
        children: [expect.objectContaining({ name: '😀', detail: 'boolean', kind: 6 })],
      }),
    ]);

    const formatEdits = await monacoHarness.semanticProviders.formatting
      .at(-1)!
      .provideDocumentFormattingEdits(model, { tabSize: 4, insertSpaces: false }, token);
    expect(formatEdits).toEqual([
      {
        range: {
          startLineNumber: 2,
          startColumn: 9,
          endLineNumber: 2,
          endColumn: 12,
        },
        text: 'true',
      },
    ]);

    const folding = await monacoHarness.semanticProviders.folding
      .at(-1)!
      .provideFoldingRanges(model, {}, token);
    expect(folding).toEqual([{ start: 1, end: 3, kind: { value: 'region' } }]);

    const selections = await monacoHarness.semanticProviders.selection
      .at(-1)!
      .provideSelectionRanges(model, [{ lineNumber: 2, column: 10 }], token);
    expect(selections).toEqual([
      [
        {
          range: {
            startLineNumber: 2,
            startColumn: 9,
            endLineNumber: 2,
            endColumn: 12,
          },
        },
        {
          range: {
            startLineNumber: 2,
            startColumn: 3,
            endLineNumber: 2,
            endColumn: 12,
          },
        },
      ],
    ]);
    expect(token.listenerDisposables.length).toBeGreaterThanOrEqual(6);
    for (const disposable of token.listenerDisposables) {
      expect(disposable.dispose).toHaveBeenCalledOnce();
    }
    adapter.dispose();
  });

  it('aborts old validation and never applies its stale markers to a replaced model', async () => {
    const { createMonacoSourceEditor } = await import('./monaco-source-editor');
    const pending: Array<{
      readonly signal: AbortSignal | undefined;
      readonly resolve: (value: unknown) => void;
    }> = [];
    const requestLanguage = vi.fn(
      (request: { readonly method: string }, options?: { readonly signal?: AbortSignal }) => {
        if (request.method !== 'validation') return Promise.resolve([]);
        return new Promise<unknown>((resolve) => {
          pending.push({ signal: options?.signal, resolve });
        });
      },
    ) as unknown as EditorAdapterMount['requestLanguage'];
    const adapter = createMonacoSourceEditor();
    adapter.mount({
      container: document.createElement('div'),
      documentId: 'document:old',
      text: '{"old":true}',
      modelVersion: 1,
      requestLanguage,
      onTextChange: vi.fn(),
      onRuntimeError: (error) => {
        throw error;
      },
    });

    await vi.waitFor(() => expect(pending).toHaveLength(1));
    const oldModel = monacoHarness.models.at(-1)!;
    adapter.setDocument('document:new', '{"new":true}', 2);
    expect(pending[0]?.signal?.aborted).toBe(true);
    await vi.waitFor(() => expect(pending).toHaveLength(2));
    const newModel = monacoHarness.models.at(-1)!;

    pending[0]?.resolve([
      {
        source: 'json-language-service',
        severity: 'error',
        code: 'stale',
        message: 'stale marker',
        path: '/old',
        offset: 0,
        length: 1,
        range: {
          start: { line: 0, character: 0 },
          end: { line: 0, character: 1 },
        },
      },
    ]);
    pending[1]?.resolve([
      {
        source: 'json-language-service',
        severity: 'warning',
        code: 'current',
        message: 'current marker',
        path: '/new',
        offset: 2,
        length: 3,
        range: {
          start: { line: 0, character: 2 },
          end: { line: 0, character: 5 },
        },
      },
    ]);

    await vi.waitFor(() => {
      expect(
        monacoHarness.markerUpdates.some(
          (update) => update.model === newModel && update.markers[0]?.code === 'current',
        ),
      ).toBe(true);
    });
    expect(
      monacoHarness.markerUpdates.some(
        (update) => update.model === oldModel && update.markers[0]?.code === 'stale',
      ),
    ).toBe(false);
    expect(oldModel.dispose).toHaveBeenCalledOnce();
    expect(oldModel.listenerDisposables[0]?.dispose).toHaveBeenCalledOnce();
    adapter.dispose();
  });

  it('silences typed cancellation but reports an untyped provider failure without parsing its message', async () => {
    const { createMonacoSourceEditor } = await import('./monaco-source-editor');
    const runtimeErrors: unknown[] = [];
    const requestLanguage = vi.fn(async (request: { readonly method: string }) => {
      if (request.method === 'completion') {
        throw Object.assign(new Error('cancelled'), { code: 'cancelled' });
      }
      if (request.method === 'hover') throw new Error('cancelled');
      return [];
    }) as unknown as EditorAdapterMount['requestLanguage'];
    const adapter = createMonacoSourceEditor();
    adapter.mount({
      container: document.createElement('div'),
      documentId: 'document:failures',
      text: '{}',
      modelVersion: 1,
      requestLanguage,
      onTextChange: vi.fn(),
      onRuntimeError: (error) => runtimeErrors.push(error),
    });

    const model = monacoHarness.models.at(-1)!;
    const token = monacoHarness.createCancellationToken();
    await expect(
      monacoHarness.semanticProviders.completion
        .at(-1)!
        .provideCompletionItems(model, { lineNumber: 1, column: 1 }, {}, token),
    ).resolves.toEqual({ suggestions: [] });
    expect(runtimeErrors).toEqual([]);

    await expect(
      monacoHarness.semanticProviders.hover
        .at(-1)!
        .provideHover(model, { lineNumber: 1, column: 1 }, token),
    ).resolves.toBeNull();
    expect(runtimeErrors).toHaveLength(1);
    expect(runtimeErrors[0]).toMatchObject({ name: 'Error', message: 'cancelled' });
    adapter.dispose();
  });
});

describe('CodeMirror source editor adapter', () => {
  it('coalesces IME composition, reports native selection, and does not cancel touch scroll', async () => {
    const container = document.createElement('div');
    container.setAttribute('aria-label', 'Masked source JSON');
    document.body.append(container);
    const textChanges: Array<{ text: string; modelVersion: number }> = [];
    const selections: Array<{ anchor: number; head: number }> = [];
    const runtimeErrors: unknown[] = [];
    const adapter = createCodeMirrorSourceEditor();
    adapter.mount({
      container,
      documentId: 'document:mobile',
      text: '{}',
      modelVersion: 4,
      requestLanguage: (async () => []) as unknown as EditorAdapterMount['requestLanguage'],
      onTextChange: (text, modelVersion) => textChanges.push({ text, modelVersion }),
      onSelectionChange: (selection) => selections.push(selection),
      onRuntimeError: (error) => runtimeErrors.push(error),
    });

    const editorElement = container.querySelector<HTMLElement>('.cm-editor');
    expect(editorElement).not.toBeNull();
    const view = EditorView.findFromDOM(editorElement!);
    expect(view).not.toBeNull();
    expect(view!.contentDOM.getAttribute('aria-label')).toBe('Masked source JSON');

    view!.contentDOM.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
    view!.dispatch({
      changes: { from: 1, insert: '你' },
      userEvent: 'input.type.compose',
    });
    view!.dispatch({
      changes: { from: 2, insert: '好' },
      userEvent: 'input.type.compose',
    });
    expect(textChanges).toEqual([]);

    view!.contentDOM.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true }));
    await Promise.resolve();
    expect(textChanges).toEqual([{ text: '{你好}', modelVersion: 5 }]);

    view!.dispatch({ selection: { anchor: 1, head: 3 } });
    expect(selections.at(-1)).toEqual({ anchor: 1, head: 3 });

    const touchMove = new Event('touchmove', { bubbles: true, cancelable: true });
    view!.scrollDOM.dispatchEvent(touchMove);
    expect(touchMove.defaultPrevented).toBe(false);
    expect(runtimeErrors).toEqual([]);

    adapter.dispose();
    expect(container.querySelector('.cm-editor')).toBeNull();
    container.remove();
  });

  it('exposes the native JSON fold gutter and registered folding keymap', async () => {
    const container = document.createElement('div');
    document.body.append(container);
    const adapter = createCodeMirrorSourceEditor();
    adapter.mount({
      container,
      documentId: 'document:folding',
      text: '{\n  "value": 1\n}',
      modelVersion: 0,
      requestLanguage: (async () => []) as unknown as EditorAdapterMount['requestLanguage'],
      onTextChange: () => undefined,
      onRuntimeError: (error) => {
        throw error;
      },
    });
    await Promise.resolve();

    const editorElement = container.querySelector<HTMLElement>('.cm-editor');
    const view = EditorView.findFromDOM(editorElement!);
    expect(container.querySelector('.cm-foldGutter')).not.toBeNull();
    const bindings = view!.state.facet(keymap).flat();
    const fold = bindings.find((binding) => binding.key === 'Ctrl-Shift-[');
    const unfold = bindings.find((binding) => binding.key === 'Ctrl-Shift-]');
    expect(fold).toBeDefined();
    expect(unfold).toBeDefined();
    if (!fold?.run || !unfold?.run) throw new Error('folding keymap is unavailable');

    view!.dispatch({ selection: { anchor: 0 } });
    expect(fold.run(view!)).toBe(true);
    expect(container.querySelector('.cm-foldPlaceholder')).not.toBeNull();
    expect(unfold.run(view!)).toBe(true);
    expect(container.querySelector('.cm-foldPlaceholder')).toBeNull();

    adapter.dispose();
    container.remove();
  });

  it('shows worker diagnostics, completion resolve, and hover through native entry points', async () => {
    const source = '{\n  "": 0\n}';
    const completionPosition = source.indexOf('""') + 1;
    const propertyRange = {
      start: { line: 1, character: 2 },
      end: { line: 1, character: 4 },
    };
    const calls: Array<{
      readonly method: string;
      readonly payload: unknown;
      readonly signal: AbortSignal | undefined;
    }> = [];
    const requestLanguage = (async (
      request: { readonly method: string; readonly payload: unknown },
      options?: { readonly signal?: AbortSignal },
    ): Promise<unknown> => {
      calls.push({ method: request.method, payload: request.payload, signal: options?.signal });
      switch (request.method) {
        case 'validation':
          return [
            {
              source: 'json-language-service',
              severity: 'error',
              code: 513,
              message: 'Property required',
              path: '$',
              offset: source.indexOf('""'),
              length: 2,
              range: propertyRange,
            },
          ];
        case 'completion':
          return {
            isIncomplete: false,
            items: [
              {
                label: 'bookSourceName',
                kind: 10,
                detail: 'Legado field',
                documentation: 'Initial documentation',
                insertTextFormat: 2,
                textEdit: { range: propertyRange, newText: '"bookSourceName"' },
              },
            ],
          };
        case 'completionResolve':
          return {
            label: 'bookSourceName',
            detail: 'Legado field',
            documentation: {
              kind: 'markdown',
              value: '<strong>Resolved documentation</strong>',
            },
          };
        case 'hover':
          return {
            contents: {
              kind: 'markdown',
              value: '<img src="https://invalid.test/tracker" onerror="alert(1)"> Hover text',
            },
            range: propertyRange,
          };
        case 'format':
        case 'symbols':
        case 'folding':
        case 'selectionRanges':
          return [];
        default:
          return null;
      }
    }) as unknown as EditorAdapterMount['requestLanguage'];
    const container = document.createElement('div');
    document.body.append(container);
    const adapter = createCodeMirrorSourceEditor();
    adapter.mount({
      container,
      documentId: 'document:semantics',
      text: source,
      modelVersion: 2,
      requestLanguage,
      onTextChange: () => undefined,
      onRuntimeError: (error) => {
        throw error;
      },
    });

    const editorElement = container.querySelector<HTMLElement>('.cm-editor');
    const view = EditorView.findFromDOM(editorElement!);
    await Promise.resolve();
    forceLinting(view!);
    await vi.waitFor(() => expect(diagnosticCount(view!.state)).toBe(1));
    const diagnostics: Array<{ message: string; from: number; to: number }> = [];
    forEachDiagnostic(view!.state, (diagnostic, from, to) => {
      diagnostics.push({ message: diagnostic.message, from, to });
    });
    expect(diagnostics).toEqual([
      { message: 'Property required', from: source.indexOf('""'), to: source.indexOf('""') + 2 },
    ]);
    expect(container.querySelector('.cm-lintRange-error')).not.toBeNull();

    view!.dispatch({ selection: { anchor: completionPosition } });
    expect(startCompletion(view!)).toBe(true);
    await vi.waitFor(() =>
      expect(currentCompletions(view!.state).some((item) => item.label === 'bookSourceName')).toBe(
        true,
      ),
    );
    const completion = currentCompletions(view!.state).find(
      (item) => item.label === 'bookSourceName',
    )!;
    expect(container.querySelector('.cm-tooltip-autocomplete')?.textContent).toContain(
      'bookSourceName',
    );
    if (typeof completion.info !== 'function') throw new Error('completion resolve entry missing');
    const completionInfo = await completion.info(completion);
    const completionInfoNode =
      completionInfo instanceof Node ? completionInfo : completionInfo?.dom;
    expect(completionInfoNode?.textContent).toBe(
      'Legado field\n\n<strong>Resolved documentation</strong>',
    );
    expect((completionInfoNode as HTMLElement).querySelector('strong')).toBeNull();
    closeCompletion(view!);

    activateHover(view!, completionPosition, 1);
    await vi.waitFor(() =>
      expect(container.querySelector('.cm-source-language-hover')?.textContent).toContain(
        '<img src="https://invalid.test/tracker" onerror="alert(1)"> Hover text',
      ),
    );
    expect(container.querySelector('.cm-source-language-hover img')).toBeNull();
    expect(calls).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ method: 'validation', payload: {} }),
        expect.objectContaining({
          method: 'completion',
          payload: { position: { line: 1, character: 3 } },
        }),
        expect.objectContaining({ method: 'completionResolve' }),
        expect.objectContaining({
          method: 'hover',
          payload: { position: { line: 1, character: 3 } },
        }),
      ]),
    );

    adapter.dispose();
    container.remove();
  });

  it('keeps native history and applies worker format edits as one undo group', async () => {
    const container = document.createElement('div');
    const textChanges: Array<{ text: string; modelVersion: number }> = [];
    const languageRequests: Array<{ readonly method: string; readonly payload: unknown }> = [];
    const requestLanguage = (async (request: {
      readonly method: string;
      readonly payload: unknown;
    }): Promise<unknown> => {
      languageRequests.push(request);
      if (request.method === 'format') {
        return [
          {
            range: {
              start: { line: 0, character: 0 },
              end: { line: 0, character: 11 },
            },
            newText: '{\n  "value": 2\n}',
          },
        ];
      }
      return [];
    }) as unknown as EditorAdapterMount['requestLanguage'];
    const adapter = createCodeMirrorSourceEditor();
    adapter.mount({
      container,
      documentId: 'document:history',
      text: '{"value":1}',
      modelVersion: 0,
      requestLanguage,
      onTextChange: (text, modelVersion) => textChanges.push({ text, modelVersion }),
      onRuntimeError: (error) => {
        throw error;
      },
    });
    await Promise.resolve();

    const editorElement = container.querySelector<HTMLElement>('.cm-editor');
    const view = EditorView.findFromDOM(editorElement!);
    view!.dispatch({
      changes: { from: 0, to: view!.state.doc.length, insert: '{"value":2}' },
      userEvent: 'input.type',
    });
    expect(textChanges.at(-1)).toEqual({ text: '{"value":2}', modelVersion: 1 });

    await expect(adapter.runCommand('undo')).resolves.toBe(true);
    expect(textChanges.at(-1)).toEqual({ text: '{"value":1}', modelVersion: 2 });
    await expect(adapter.runCommand('redo')).resolves.toBe(true);
    expect(textChanges.at(-1)).toEqual({ text: '{"value":2}', modelVersion: 3 });

    await expect(adapter.runCommand('format')).resolves.toBe(true);
    expect(textChanges.at(-1)).toEqual({ text: '{\n  "value": 2\n}', modelVersion: 4 });
    expect(languageRequests.at(-1)).toEqual({
      method: 'format',
      payload: { options: { tabSize: 2, insertSpaces: true } },
    });
    await expect(adapter.runCommand('undo')).resolves.toBe(true);
    expect(textChanges.at(-1)).toEqual({ text: '{"value":2}', modelVersion: 5 });

    adapter.setDocument('document:next', '{"next":true}', 11);
    expect(textChanges.at(-1)?.modelVersion).toBe(5);
    expect(EditorView.findFromDOM(editorElement!)?.state.doc.toString()).toBe('{"next":true}');

    adapter.dispose();
  });

  it('drops stale diagnostics and aborts semantic work on document change and dispose', async () => {
    const container = document.createElement('div');
    document.body.append(container);
    const runtimeErrors: unknown[] = [];
    const pendingValidations: Array<{
      readonly signal: AbortSignal;
      readonly resolve: (value: unknown) => void;
    }> = [];
    const requestLanguage = ((
      request: { readonly method: string },
      options?: { readonly signal?: AbortSignal },
    ): Promise<unknown> => {
      if (request.method !== 'validation') return Promise.resolve([]);
      const { promise, resolve } = Promise.withResolvers<unknown>();
      pendingValidations.push({ signal: options!.signal!, resolve });
      return promise;
    }) as unknown as EditorAdapterMount['requestLanguage'];
    const adapter = createCodeMirrorSourceEditor();
    adapter.mount({
      container,
      documentId: 'document:stale',
      text: '{}',
      modelVersion: 0,
      requestLanguage,
      onTextChange: () => undefined,
      onRuntimeError: (error) => runtimeErrors.push(error),
    });

    await vi.waitFor(() => expect(pendingValidations).toHaveLength(1));
    adapter.setDocument('document:stale-next', '{"next":true}', 7);
    await vi.waitFor(() => expect(pendingValidations).toHaveLength(2));
    expect(pendingValidations[0].signal.aborted).toBe(true);
    pendingValidations[0].resolve([
      {
        source: 'json-language-service',
        severity: 'error',
        code: 1,
        message: 'Old diagnostic',
        path: '$',
        offset: 0,
        length: 1,
        range: {
          start: { line: 0, character: 0 },
          end: { line: 0, character: 1 },
        },
      },
    ]);
    await Promise.resolve();
    await Promise.resolve();
    expect(
      diagnosticCount(EditorView.findFromDOM(container.querySelector('.cm-editor')!)!.state),
    ).toBe(0);

    adapter.dispose();
    expect(pendingValidations[1].signal.aborted).toBe(true);
    pendingValidations[1].resolve([]);
    await Promise.resolve();
    expect(runtimeErrors).toEqual([]);
    expect(container.querySelector('.cm-editor')).toBeNull();
    container.remove();
  });
});
