import * as monaco from 'monaco-editor/editor';
import EditorWorker from 'monaco-editor/editor/editor.worker?worker';
import type {
  CompletionItem as LanguageCompletionItem,
  DocumentSymbol as LanguageDocumentSymbol,
  FoldingRange as LanguageFoldingRange,
  Hover as LanguageHover,
  MarkedString as LanguageMarkedString,
  MarkupContent as LanguageMarkupContent,
  Position as LanguagePosition,
  Range as LanguageRange,
  SelectionRange as LanguageSelectionRange,
  TextEdit as LanguageTextEdit,
} from 'vscode-json-languageservice';
import type {
  SourceLanguageDiagnostic,
  SourceLanguageMethod,
  SourceLanguageRequestWithoutText,
  SourceLanguageResultMap,
} from './source-language-service-protocol';
import type {
  EditorAdapter,
  EditorAdapterMount,
  EditorCommand,
  EditorSelection,
} from './source-editor-adapters';

type MonacoWorkerEnvironment = {
  getWorker(moduleId: string, label: string): Worker;
};

const workerScope = globalThis as typeof globalThis & {
  MonacoEnvironment?: MonacoWorkerEnvironment;
};

workerScope.MonacoEnvironment ??= {
  getWorker() {
    return new EditorWorker();
  },
};

// Tokenization stays local while the shared source-language-service remains the sole semantic owner.
const SOURCE_JSON_LANGUAGE_ID = 'lanjing-source-json';
monaco.languages.register({ id: SOURCE_JSON_LANGUAGE_ID, aliases: ['JSON'] });
let languageSupportUsers = 0;
let languageSupportDisposables: monaco.IDisposable[] = [];

function acquireLanguageSupport(): () => void {
  if (languageSupportUsers === 0) {
    const disposables: monaco.IDisposable[] = [];
    try {
      disposables.push(
        monaco.languages.setMonarchTokensProvider(SOURCE_JSON_LANGUAGE_ID, {
          tokenizer: {
            root: [
              [/\s+/, 'white'],
              [/[{}[\]]/, 'delimiter.bracket'],
              [/[:,]/, 'delimiter'],
              [/-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?/, 'number'],
              [/\b(?:true|false|null)\b/, 'keyword'],
              [/"(?:[^"\\]|\\.)*"(?=\s*:)/, 'string.key'],
              [/"/, { token: 'string.quote', bracket: '@open', next: '@string' }],
            ],
            string: [
              [/[^\\"]+/, 'string'],
              [/\\(?:["\\/bfnrt]|u[0-9a-fA-F]{4})/, 'string.escape'],
              [/"/, { token: 'string.quote', bracket: '@close', next: '@pop' }],
              [/\\./, 'string.escape.invalid'],
            ],
          },
        }),
      );
      disposables.push(
        monaco.languages.setLanguageConfiguration(SOURCE_JSON_LANGUAGE_ID, {
          brackets: [
            ['{', '}'],
            ['[', ']'],
          ],
          autoClosingPairs: [
            { open: '{', close: '}' },
            { open: '[', close: ']' },
            { open: '"', close: '"' },
          ],
        }),
      );
    } catch (error) {
      for (const disposable of disposables.reverse()) {
        try {
          disposable.dispose();
        } catch {
          // Preserve the acquisition error while releasing earlier providers.
        }
      }
      throw error;
    }
    languageSupportDisposables = disposables;
  }

  languageSupportUsers += 1;
  let released = false;
  return () => {
    if (released) return;
    released = true;
    languageSupportUsers -= 1;
    if (languageSupportUsers !== 0) return;

    for (const disposable of languageSupportDisposables.splice(0).reverse()) {
      try {
        disposable.dispose();
      } catch {
        // Continue releasing the remaining Monaco language providers.
      }
    }
  };
}

const LANGUAGE_MARKER_OWNER = 'lanjing-source-language-service';

function toLanguagePosition(position: monaco.IPosition): LanguagePosition {
  return {
    line: position.lineNumber - 1,
    character: position.column - 1,
  };
}

function toMonacoPosition(position: LanguagePosition): monaco.IPosition {
  return {
    lineNumber: position.line + 1,
    column: position.character + 1,
  };
}

function toMonacoRange(range: LanguageRange): monaco.IRange {
  const start = toMonacoPosition(range.start);
  const end = toMonacoPosition(range.end);
  return {
    startLineNumber: start.lineNumber,
    startColumn: start.column,
    endLineNumber: end.lineNumber,
    endColumn: end.column,
  };
}

function toMonacoOffsetRange(
  model: monaco.editor.ITextModel,
  offset: number,
  length: number,
): monaco.IRange {
  const start = model.getPositionAt(Math.max(0, offset));
  const end = model.getPositionAt(Math.max(0, offset + Math.max(0, length)));
  return {
    startLineNumber: start.lineNumber,
    startColumn: start.column,
    endLineNumber: end.lineNumber,
    endColumn: end.column,
  };
}

function pointRange(position: monaco.IPosition): monaco.IRange {
  return {
    startLineNumber: position.lineNumber,
    startColumn: position.column,
    endLineNumber: position.lineNumber,
    endColumn: position.column,
  };
}

function rangeContainsPosition(range: LanguageRange, position: LanguagePosition): boolean {
  return (
    range.start.line === range.end.line &&
    range.start.line === position.line &&
    range.start.character <= position.character &&
    position.character <= range.end.character
  );
}

function toMonacoCompletionKind(
  kind: LanguageCompletionItem['kind'],
): monaco.languages.CompletionItemKind {
  switch (kind) {
    case 2:
      return monaco.languages.CompletionItemKind.Method;
    case 3:
      return monaco.languages.CompletionItemKind.Function;
    case 4:
      return monaco.languages.CompletionItemKind.Constructor;
    case 5:
      return monaco.languages.CompletionItemKind.Field;
    case 6:
      return monaco.languages.CompletionItemKind.Variable;
    case 7:
      return monaco.languages.CompletionItemKind.Class;
    case 8:
      return monaco.languages.CompletionItemKind.Interface;
    case 9:
      return monaco.languages.CompletionItemKind.Module;
    case 10:
      return monaco.languages.CompletionItemKind.Property;
    case 11:
      return monaco.languages.CompletionItemKind.Unit;
    case 12:
      return monaco.languages.CompletionItemKind.Value;
    case 13:
      return monaco.languages.CompletionItemKind.Enum;
    case 14:
      return monaco.languages.CompletionItemKind.Keyword;
    case 15:
      return monaco.languages.CompletionItemKind.Snippet;
    case 16:
      return monaco.languages.CompletionItemKind.Color;
    case 17:
      return monaco.languages.CompletionItemKind.File;
    case 18:
      return monaco.languages.CompletionItemKind.Reference;
    case 19:
      return monaco.languages.CompletionItemKind.Folder;
    case 20:
      return monaco.languages.CompletionItemKind.EnumMember;
    case 21:
      return monaco.languages.CompletionItemKind.Constant;
    case 22:
      return monaco.languages.CompletionItemKind.Struct;
    case 23:
      return monaco.languages.CompletionItemKind.Event;
    case 24:
      return monaco.languages.CompletionItemKind.Operator;
    case 25:
      return monaco.languages.CompletionItemKind.TypeParameter;
    case 1:
    default:
      return monaco.languages.CompletionItemKind.Text;
  }
}

function toMonacoSymbolKind(kind: LanguageDocumentSymbol['kind']): monaco.languages.SymbolKind {
  switch (kind) {
    case 1:
      return monaco.languages.SymbolKind.File;
    case 2:
      return monaco.languages.SymbolKind.Module;
    case 3:
      return monaco.languages.SymbolKind.Namespace;
    case 4:
      return monaco.languages.SymbolKind.Package;
    case 5:
      return monaco.languages.SymbolKind.Class;
    case 6:
      return monaco.languages.SymbolKind.Method;
    case 7:
      return monaco.languages.SymbolKind.Property;
    case 8:
      return monaco.languages.SymbolKind.Field;
    case 9:
      return monaco.languages.SymbolKind.Constructor;
    case 10:
      return monaco.languages.SymbolKind.Enum;
    case 11:
      return monaco.languages.SymbolKind.Interface;
    case 12:
      return monaco.languages.SymbolKind.Function;
    case 13:
      return monaco.languages.SymbolKind.Variable;
    case 14:
      return monaco.languages.SymbolKind.Constant;
    case 15:
      return monaco.languages.SymbolKind.String;
    case 16:
      return monaco.languages.SymbolKind.Number;
    case 17:
      return monaco.languages.SymbolKind.Boolean;
    case 18:
      return monaco.languages.SymbolKind.Array;
    case 19:
      return monaco.languages.SymbolKind.Object;
    case 20:
      return monaco.languages.SymbolKind.Key;
    case 21:
      return monaco.languages.SymbolKind.Null;
    case 22:
      return monaco.languages.SymbolKind.EnumMember;
    case 23:
      return monaco.languages.SymbolKind.Struct;
    case 24:
      return monaco.languages.SymbolKind.Event;
    case 25:
      return monaco.languages.SymbolKind.Operator;
    case 26:
      return monaco.languages.SymbolKind.TypeParameter;
    default:
      return monaco.languages.SymbolKind.Property;
  }
}

function toMonacoMarkerSeverity(
  severity: SourceLanguageDiagnostic['severity'],
): monaco.MarkerSeverity {
  switch (severity) {
    case 'error':
      return monaco.MarkerSeverity.Error;
    case 'information':
      return monaco.MarkerSeverity.Info;
    case 'hint':
      return monaco.MarkerSeverity.Hint;
    case 'warning':
    default:
      return monaco.MarkerSeverity.Warning;
  }
}

function toMonacoMarker(
  model: monaco.editor.ITextModel,
  diagnostic: SourceLanguageDiagnostic,
): monaco.editor.IMarkerData {
  const range = toMonacoOffsetRange(model, diagnostic.offset, diagnostic.length);
  return {
    code: String(diagnostic.code),
    severity: toMonacoMarkerSeverity(diagnostic.severity),
    message: diagnostic.message,
    source: diagnostic.source,
    modelVersionId: model.getVersionId(),
    ...range,
  };
}

function toDocumentOffset(
  model: monaco.editor.ITextModel,
  position: LanguagePosition,
): number | null {
  if (
    !position ||
    !Number.isSafeInteger(position.line) ||
    position.line < 0 ||
    !Number.isSafeInteger(position.character) ||
    position.character < 0
  ) {
    return null;
  }

  const target = toMonacoPosition(position);
  try {
    const offset = model.getOffsetAt(target);
    const roundTrip = model.getPositionAt(offset);
    return roundTrip.lineNumber === target.lineNumber && roundTrip.column === target.column
      ? offset
      : null;
  } catch {
    return null;
  }
}

function mapMonacoTextEdits(
  model: monaco.editor.ITextModel,
  edits: readonly LanguageTextEdit[],
): monaco.languages.TextEdit[] | null {
  const mapped: Array<{
    readonly startOffset: number;
    readonly endOffset: number;
    readonly edit: monaco.languages.TextEdit;
  }> = [];

  for (const edit of edits) {
    const startOffset = toDocumentOffset(model, edit.range.start);
    const endOffset = toDocumentOffset(model, edit.range.end);
    if (startOffset === null || endOffset === null || startOffset > endOffset) return null;
    mapped.push({
      startOffset,
      endOffset,
      edit: { range: toMonacoRange(edit.range), text: edit.newText },
    });
  }

  const ordered = [...mapped].sort(
    (left, right) => left.startOffset - right.startOffset || left.endOffset - right.endOffset,
  );
  for (let index = 1; index < ordered.length; index += 1) {
    const previous = ordered[index - 1];
    const current = ordered[index];
    if (current.startOffset < previous.endOffset || current.startOffset === previous.startOffset) {
      return null;
    }
  }

  return mapped.map(({ edit }) => edit);
}

const SAFE_MARKDOWN_OPTIONS = {
  isTrusted: false,
  supportHtml: false,
  supportThemeIcons: false,
} as const;

function toPlaintextMarkdown(value: string): monaco.IMarkdownString {
  return {
    ...SAFE_MARKDOWN_OPTIONS,
    value: value.replace(/[!-/:-@[-`{-~]/g, '\\$&'),
  };
}

function toCompletionDocumentation(
  documentation: LanguageCompletionItem['documentation'],
): monaco.IMarkdownString | undefined {
  if (documentation === undefined) return undefined;
  return toPlaintextMarkdown(
    typeof documentation === 'string' ? documentation : documentation.value,
  );
}

function toCodeBlockMarkdown(
  marked: Exclude<LanguageMarkedString, string>,
): monaco.IMarkdownString {
  let fenceLength = 3;
  for (const run of marked.value.match(/`+/g) ?? []) {
    fenceLength = Math.max(fenceLength, run.length + 1);
  }
  const fence = '`'.repeat(fenceLength);
  const language = /^[A-Za-z0-9_+#.-]+$/.test(marked.language) ? marked.language : '';
  return {
    ...SAFE_MARKDOWN_OPTIONS,
    value: `${fence}${language}\n${marked.value}\n${fence}`,
  };
}

function isMarkupContent(value: LanguageHover['contents']): value is LanguageMarkupContent {
  return (
    !Array.isArray(value) &&
    typeof value === 'object' &&
    value !== null &&
    'kind' in value &&
    (value.kind === 'markdown' || value.kind === 'plaintext')
  );
}

function toMonacoHoverContents(contents: LanguageHover['contents']): monaco.IMarkdownString[] {
  if (isMarkupContent(contents)) return [toPlaintextMarkdown(contents.value)];

  const markedStrings = Array.isArray(contents) ? contents : [contents];
  return markedStrings.map((marked) =>
    typeof marked === 'string' ? toPlaintextMarkdown(marked) : toCodeBlockMarkdown(marked),
  );
}

function toMonacoDocumentSymbol(symbol: LanguageDocumentSymbol): monaco.languages.DocumentSymbol {
  const children = symbol.children?.map(toMonacoDocumentSymbol);
  const deprecated = symbol.deprecated === true || symbol.tags?.includes(1) === true;
  return {
    name: symbol.name,
    detail: symbol.detail ?? '',
    kind: toMonacoSymbolKind(symbol.kind),
    tags: deprecated ? [monaco.languages.SymbolTag.Deprecated] : [],
    range: toMonacoRange(symbol.range),
    selectionRange: toMonacoRange(symbol.selectionRange),
    ...(children && children.length > 0 ? { children } : {}),
  };
}

function toMonacoFoldingRange(range: LanguageFoldingRange): monaco.languages.FoldingRange | null {
  if (
    !Number.isSafeInteger(range.startLine) ||
    !Number.isSafeInteger(range.endLine) ||
    range.startLine < 0 ||
    range.endLine < range.startLine
  ) {
    return null;
  }

  let kind: monaco.languages.FoldingRangeKind | undefined;
  switch (range.kind) {
    case 'comment':
      kind = monaco.languages.FoldingRangeKind.Comment;
      break;
    case 'imports':
      kind = monaco.languages.FoldingRangeKind.Imports;
      break;
    case 'region':
      kind = monaco.languages.FoldingRangeKind.Region;
      break;
  }

  return {
    start: range.startLine + 1,
    end: range.endLine + 1,
    ...(kind ? { kind } : {}),
  };
}

function toMonacoSelectionRangeChain(
  selectionRange: LanguageSelectionRange,
): monaco.languages.SelectionRange[] {
  const ranges: monaco.languages.SelectionRange[] = [];
  const visited = new Set<LanguageSelectionRange>();
  let current: LanguageSelectionRange | undefined = selectionRange;
  while (current && !visited.has(current)) {
    visited.add(current);
    ranges.push({ range: toMonacoRange(current.range) });
    current = current.parent;
  }
  return ranges;
}

function isSilentLanguageError(error: unknown): boolean {
  if (typeof error !== 'object' || error === null) return false;
  const candidate = error as { readonly code?: unknown; readonly name?: unknown };
  return (
    candidate.name === 'AbortError' ||
    candidate.code === 'cancelled' ||
    candidate.code === 'stale_document' ||
    candidate.code === 'disposed'
  );
}

interface CompletionItemMetadata {
  readonly source: LanguageCompletionItem;
  readonly position: monaco.IPosition;
}

function isSafeLspSnippet(template: string): boolean {
  let index = 0;
  while (index < template.length) {
    if (template[index] === '\\') {
      index += index + 1 < template.length ? 2 : 1;
      continue;
    }
    if (template[index] !== '$') {
      index += 1;
      continue;
    }

    const numberedStop = /^\$(\d+)/.exec(template.slice(index));
    if (numberedStop) {
      index += numberedStop[0].length;
      continue;
    }
    const placeholder = /^\$\{(\d+)(?::([^{}$]*))?\}/.exec(template.slice(index));
    if (!placeholder) return false;
    index += placeholder[0].length;
  }
  return true;
}

function isValidLanguageRange(model: monaco.editor.ITextModel, range: LanguageRange): boolean {
  const startOffset = toDocumentOffset(model, range.start);
  const endOffset = toDocumentOffset(model, range.end);
  return startOffset !== null && endOffset !== null && startOffset <= endOffset;
}

function safeCompletionEdit(
  model: monaco.editor.ITextModel,
  item: LanguageCompletionItem,
  position: monaco.IPosition,
): {
  readonly insertText: string;
  readonly range: monaco.languages.CompletionItem['range'];
} | null {
  const edit = item.textEdit;
  if (!edit) return null;
  const languagePosition = toLanguagePosition(position);

  if ('range' in edit) {
    if (
      !isValidLanguageRange(model, edit.range) ||
      !rangeContainsPosition(edit.range, languagePosition)
    ) {
      return null;
    }
    return { insertText: edit.newText, range: toMonacoRange(edit.range) };
  }

  if (
    !isValidLanguageRange(model, edit.insert) ||
    !isValidLanguageRange(model, edit.replace) ||
    !rangeContainsPosition(edit.insert, languagePosition) ||
    !rangeContainsPosition(edit.replace, languagePosition) ||
    edit.insert.start.line !== edit.replace.start.line ||
    edit.insert.start.character !== edit.replace.start.character ||
    edit.insert.end.character > edit.replace.end.character
  ) {
    return null;
  }
  return {
    insertText: edit.newText,
    range: {
      insert: toMonacoRange(edit.insert),
      replace: toMonacoRange(edit.replace),
    },
  };
}

function toMonacoCompletionItem(
  source: LanguageCompletionItem,
  model: monaco.editor.ITextModel,
  position: monaco.IPosition,
  previous?: monaco.languages.CompletionItem,
): monaco.languages.CompletionItem | null {
  if (source.command || (source.additionalTextEdits?.length ?? 0) > 0) return null;
  if (
    source.insertTextFormat !== undefined &&
    source.insertTextFormat !== 1 &&
    source.insertTextFormat !== 2
  ) {
    return null;
  }

  const edit = safeCompletionEdit(model, source, position);
  if (source.textEdit && !edit) return null;
  const documentation = toCompletionDocumentation(source.documentation) ?? previous?.documentation;
  const deprecated = source.deprecated === true || source.tags?.includes(1) === true;
  const insertText = edit?.insertText ?? source.insertText ?? previous?.insertText ?? source.label;
  const insertTextRules =
    source.insertTextFormat === 2
      ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
      : source.insertTextFormat === 1
        ? undefined
        : previous?.insertTextRules;
  if (
    insertTextRules !== undefined &&
    (insertTextRules & monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet) !== 0 &&
    !isSafeLspSnippet(insertText)
  ) {
    return null;
  }

  return {
    label: source.label,
    kind:
      source.kind === undefined
        ? (previous?.kind ?? monaco.languages.CompletionItemKind.Text)
        : toMonacoCompletionKind(source.kind),
    insertText,
    range: edit?.range ?? previous?.range ?? pointRange(position),
    ...(insertTextRules === undefined ? {} : { insertTextRules }),
    ...(source.detail === undefined && previous?.detail === undefined
      ? {}
      : { detail: source.detail ?? previous?.detail }),
    ...(documentation === undefined ? {} : { documentation }),
    ...(source.sortText === undefined && previous?.sortText === undefined
      ? {}
      : { sortText: source.sortText ?? previous?.sortText }),
    ...(source.filterText === undefined && previous?.filterText === undefined
      ? {}
      : { filterText: source.filterText ?? previous?.filterText }),
    ...(source.preselect === undefined && previous?.preselect === undefined
      ? {}
      : { preselect: source.preselect ?? previous?.preselect }),
    ...(deprecated ? { tags: [monaco.languages.CompletionItemTag.Deprecated] } : {}),
  };
}

let nextModelInstance = 0;

class MonacoSourceEditorAdapter implements EditorAdapter {
  readonly kind = 'monaco' as const;

  private editor: monaco.editor.IStandaloneCodeEditor | null = null;
  private model: monaco.editor.ITextModel | null = null;
  private modelListener: monaco.IDisposable | null = null;
  private editorListeners: monaco.IDisposable[] = [];
  private semanticProviderDisposables: monaco.IDisposable[] = [];
  private pendingLanguageRequests = new Set<AbortController>();
  private completionItems = new WeakMap<monaco.languages.CompletionItem, CompletionItemMetadata>();
  private mountOptions: EditorAdapterMount | null = null;
  private releaseLanguageSupport: (() => void) | null = null;
  private documentId = '';
  private modelVersion = 0;
  private languageGeneration = 0;
  private suppressTextChanges = false;
  private composing = false;
  private compositionChanged = false;
  private runtimeFailed = false;
  private disposed = false;

  mount(options: EditorAdapterMount): void {
    if (this.disposed) throw new Error('Cannot mount a disposed Monaco source editor');
    if (this.editor) throw new Error('Monaco source editor is already mounted');

    this.mountOptions = options;
    this.documentId = options.documentId;
    this.modelVersion = options.modelVersion;

    try {
      this.releaseLanguageSupport = acquireLanguageSupport();
      this.model = this.createModel(options.documentId, options.text);
      this.editor = monaco.editor.create(options.container, {
        model: this.model,
        automaticLayout: true,
        ariaLabel: options.container.getAttribute('aria-label') ?? 'Source JSON editor',
        minimap: { enabled: false },
        scrollBeyondLastLine: false,
        wordWrap: 'on',
      });
      this.registerSemanticProviders();
      this.modelListener = this.listenToModel(this.model);
      this.editorListeners.push(
        this.editor.onDidChangeCursorSelection((event) => {
          this.emitSelection({
            anchor:
              this.model?.getOffsetAt({
                lineNumber: event.selection.selectionStartLineNumber,
                column: event.selection.selectionStartColumn,
              }) ?? 0,
            head:
              this.model?.getOffsetAt({
                lineNumber: event.selection.positionLineNumber,
                column: event.selection.positionColumn,
              }) ?? 0,
          });
        }),
      );
      this.editorListeners.push(
        this.editor.onDidCompositionStart(() => {
          this.composing = true;
          this.compositionChanged = false;
        }),
      );
      this.editorListeners.push(
        this.editor.onDidCompositionEnd(() => {
          this.composing = false;
          if (!this.compositionChanged || this.suppressTextChanges) return;
          this.compositionChanged = false;
          this.emitCurrentText();
          this.scheduleValidation(this.model, this.languageGeneration);
        }),
      );
      this.scheduleValidation(this.model, this.languageGeneration);
    } catch (error) {
      this.releaseResources();
      this.disposed = true;
      throw error;
    }
  }

  setDocument(documentId: string, text: string, modelVersion: number): void {
    const editor = this.requireEditor();
    const currentModel = this.requireModel();
    const generation = this.invalidateLanguageState(currentModel);

    try {
      this.suppressTextChanges = true;
      this.composing = false;
      this.compositionChanged = false;

      if (documentId === this.documentId) {
        this.modelVersion = modelVersion;
        if (currentModel.getValue() !== text) currentModel.setValue(text);
        this.scheduleValidation(currentModel, generation);
        return;
      }

      const nextModel = this.createModel(documentId, text);
      const previousListener = this.modelListener;
      try {
        editor.setModel(nextModel);
        this.model = nextModel;
        this.modelListener = this.listenToModel(nextModel);
        this.documentId = documentId;
        this.modelVersion = modelVersion;
      } catch (error) {
        nextModel.dispose();
        throw error;
      }

      try {
        previousListener?.dispose();
      } finally {
        currentModel.dispose();
      }
      this.scheduleValidation(nextModel, generation);
    } catch (error) {
      this.reportRuntimeError(error);
      throw error;
    } finally {
      this.suppressTextChanges = false;
    }
  }

  focus(): void {
    try {
      this.requireEditor().focus();
    } catch (error) {
      this.reportRuntimeError(error);
      throw error;
    }
  }

  async runCommand(command: EditorCommand): Promise<boolean> {
    const editor = this.requireEditor();

    try {
      if (command === 'undo' || command === 'redo') {
        editor.trigger('lanjing-source-editor', command, null);
        return true;
      }

      const model = this.requireModel();
      const modelOptions = model.getOptions();
      const edits = await this.requestLanguage(model, {
        method: 'format',
        payload: {
          options: {
            tabSize: modelOptions.tabSize,
            insertSpaces: modelOptions.insertSpaces,
          },
        },
      });
      if (edits === undefined || edits.length === 0) return false;
      const mappedEdits = mapMonacoTextEdits(model, edits);
      if (!mappedEdits) throw new Error('Source language format returned invalid text edits');

      editor.pushUndoStop();
      const applied = editor.executeEdits(
        'lanjing-source-editor.format',
        mappedEdits.map((edit) => ({ ...edit, forceMoveMarkers: true })),
      );
      editor.pushUndoStop();
      return applied;
    } catch (error) {
      this.reportRuntimeError(error);
      throw error;
    }
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.releaseResources();
  }

  private createModel(documentId: string, text: string): monaco.editor.ITextModel {
    nextModelInstance += 1;
    const uri = monaco.Uri.from({
      scheme: 'inmemory',
      authority: 'lanjing',
      path: `/sources/${encodeURIComponent(documentId)}/${nextModelInstance}.json`,
    });
    return monaco.editor.createModel(text, SOURCE_JSON_LANGUAGE_ID, uri);
  }

  private registerSemanticProviders(): void {
    this.semanticProviderDisposables.push(
      monaco.languages.registerCompletionItemProvider(SOURCE_JSON_LANGUAGE_ID, {
        triggerCharacters: ['"', ':'],
        provideCompletionItems: (model, position, _context, token) =>
          this.provideCompletionItems(model, position, token),
        resolveCompletionItem: (item, token) => this.resolveCompletionItem(item, token),
      }),
    );
    this.semanticProviderDisposables.push(
      monaco.languages.registerHoverProvider(SOURCE_JSON_LANGUAGE_ID, {
        provideHover: (model, position, token) => this.provideHover(model, position, token),
      }),
    );
    this.semanticProviderDisposables.push(
      monaco.languages.registerDocumentSymbolProvider(SOURCE_JSON_LANGUAGE_ID, {
        displayName: 'LanJing source JSON',
        provideDocumentSymbols: (model, token) => this.provideDocumentSymbols(model, token),
      }),
    );
    this.semanticProviderDisposables.push(
      monaco.languages.registerDocumentFormattingEditProvider(SOURCE_JSON_LANGUAGE_ID, {
        displayName: 'LanJing source JSON',
        provideDocumentFormattingEdits: (model, options, token) =>
          this.provideDocumentFormattingEdits(model, options, token),
      }),
    );
    this.semanticProviderDisposables.push(
      monaco.languages.registerFoldingRangeProvider(SOURCE_JSON_LANGUAGE_ID, {
        provideFoldingRanges: (model, _context, token) => this.provideFoldingRanges(model, token),
      }),
    );
    this.semanticProviderDisposables.push(
      monaco.languages.registerSelectionRangeProvider(SOURCE_JSON_LANGUAGE_ID, {
        provideSelectionRanges: (model, positions, token) =>
          this.provideSelectionRanges(model, positions, token),
      }),
    );
  }

  private listenToModel(model: monaco.editor.ITextModel): monaco.IDisposable {
    return model.onDidChangeContent(() => {
      if (this.suppressTextChanges) return;
      const generation = this.invalidateLanguageState(model);
      if (this.composing) {
        this.compositionChanged = true;
        return;
      }
      this.emitCurrentText();
      this.scheduleValidation(model, generation);
    });
  }

  private scheduleValidation(model: monaco.editor.ITextModel | null, generation: number): void {
    if (!model) return;
    queueMicrotask(() => {
      if (!this.isCurrentLanguageState(model, generation)) return;
      void this.refreshValidation(model);
    });
  }

  private async refreshValidation(model: monaco.editor.ITextModel): Promise<void> {
    try {
      const diagnostics = await this.requestLanguage(model, {
        method: 'validation',
        payload: {},
      });
      if (diagnostics === undefined || model !== this.model || this.disposed) return;
      monaco.editor.setModelMarkers(
        model,
        LANGUAGE_MARKER_OWNER,
        diagnostics.map((diagnostic) => toMonacoMarker(model, diagnostic)),
      );
    } catch (error) {
      if (!this.disposed && !isSilentLanguageError(error)) this.reportRuntimeError(error);
    }
  }

  private async provideCompletionItems(
    model: monaco.editor.ITextModel,
    position: monaco.Position,
    token: monaco.CancellationToken,
  ): Promise<monaco.languages.CompletionList> {
    return this.provideSafely({ suggestions: [] }, async () => {
      if (model !== this.model || token.isCancellationRequested) return { suggestions: [] };
      const result = await this.requestLanguage(
        model,
        {
          method: 'completion',
          payload: { position: toLanguagePosition(position) },
        },
        token,
      );
      if (!result) return { suggestions: [] };

      const suggestions: monaco.languages.CompletionItem[] = [];
      for (const source of result.items) {
        const item = toMonacoCompletionItem(source, model, position);
        if (!item) continue;
        this.completionItems.set(item, { source, position });
        suggestions.push(item);
      }
      return { suggestions, incomplete: result.isIncomplete };
    });
  }

  private async resolveCompletionItem(
    item: monaco.languages.CompletionItem,
    token: monaco.CancellationToken,
  ): Promise<monaco.languages.CompletionItem> {
    return this.provideSafely(item, async () => {
      const metadata = this.completionItems.get(item);
      const model = this.model;
      if (!metadata || !model || token.isCancellationRequested) return item;
      const resolved = await this.requestLanguage(
        model,
        {
          method: 'completionResolve',
          payload: { item: metadata.source },
        },
        token,
      );
      if (!resolved) return item;
      const mapped = toMonacoCompletionItem(resolved, model, metadata.position, item);
      if (!mapped) return item;
      this.completionItems.set(mapped, { source: resolved, position: metadata.position });
      return mapped;
    });
  }

  private async provideHover(
    model: monaco.editor.ITextModel,
    position: monaco.Position,
    token: monaco.CancellationToken,
  ): Promise<monaco.languages.Hover | null> {
    return this.provideSafely(null, async () => {
      if (model !== this.model || token.isCancellationRequested) return null;
      const hover = await this.requestLanguage(
        model,
        {
          method: 'hover',
          payload: { position: toLanguagePosition(position) },
        },
        token,
      );
      if (!hover) return null;
      return {
        contents: toMonacoHoverContents(hover.contents),
        ...(hover.range ? { range: toMonacoRange(hover.range) } : {}),
      };
    });
  }

  private async provideDocumentSymbols(
    model: monaco.editor.ITextModel,
    token: monaco.CancellationToken,
  ): Promise<monaco.languages.DocumentSymbol[]> {
    return this.provideSafely([], async () => {
      if (model !== this.model || token.isCancellationRequested) return [];
      const symbols = await this.requestLanguage(model, { method: 'symbols', payload: {} }, token);
      return symbols?.map(toMonacoDocumentSymbol) ?? [];
    });
  }

  private async provideDocumentFormattingEdits(
    model: monaco.editor.ITextModel,
    options: monaco.languages.FormattingOptions,
    token: monaco.CancellationToken,
  ): Promise<monaco.languages.TextEdit[]> {
    return this.provideSafely([], async () => {
      if (model !== this.model || token.isCancellationRequested) return [];
      const edits = await this.requestLanguage(
        model,
        {
          method: 'format',
          payload: {
            options: {
              tabSize: options.tabSize,
              insertSpaces: options.insertSpaces,
            },
          },
        },
        token,
      );
      if (!edits) return [];
      const mapped = mapMonacoTextEdits(model, edits);
      if (!mapped) throw new Error('Source language format returned invalid text edits');
      return mapped;
    });
  }

  private async provideFoldingRanges(
    model: monaco.editor.ITextModel,
    token: monaco.CancellationToken,
  ): Promise<monaco.languages.FoldingRange[]> {
    return this.provideSafely([], async () => {
      if (model !== this.model || token.isCancellationRequested) return [];
      const ranges = await this.requestLanguage(model, { method: 'folding', payload: {} }, token);
      if (!ranges) return [];
      const mapped: monaco.languages.FoldingRange[] = [];
      for (const range of ranges) {
        const value = toMonacoFoldingRange(range);
        if (value) mapped.push(value);
      }
      return mapped;
    });
  }

  private async provideSelectionRanges(
    model: monaco.editor.ITextModel,
    positions: monaco.Position[],
    token: monaco.CancellationToken,
  ): Promise<monaco.languages.SelectionRange[][]> {
    const empty = positions.map(() => [] as monaco.languages.SelectionRange[]);
    return this.provideSafely(empty, async () => {
      if (model !== this.model || token.isCancellationRequested) return empty;
      const ranges = await this.requestLanguage(
        model,
        {
          method: 'selectionRanges',
          payload: { positions: positions.map(toLanguagePosition) },
        },
        token,
      );
      if (!ranges) return empty;
      return positions.map((_, index) => {
        const range = ranges[index];
        return range ? toMonacoSelectionRangeChain(range) : [];
      });
    });
  }

  private async provideSafely<T>(fallback: T, operation: () => Promise<T>): Promise<T> {
    try {
      return await operation();
    } catch (error) {
      if (!this.disposed && !this.runtimeFailed && !isSilentLanguageError(error)) {
        this.reportRuntimeError(error);
      }
      return fallback;
    }
  }

  private async requestLanguage<M extends SourceLanguageMethod>(
    model: monaco.editor.ITextModel,
    request: SourceLanguageRequestWithoutText<M>,
    token?: monaco.CancellationToken,
  ): Promise<SourceLanguageResultMap[M] | undefined> {
    const options = this.mountOptions;
    const generation = this.languageGeneration;
    if (!options || !this.isCurrentLanguageState(model, generation)) return undefined;

    const controller = new AbortController();
    this.pendingLanguageRequests.add(controller);
    let cancellationDisposable: monaco.IDisposable | undefined;
    try {
      if (token?.isCancellationRequested) controller.abort();
      if (token && !controller.signal.aborted) {
        cancellationDisposable = token.onCancellationRequested(() => controller.abort());
      }
      if (controller.signal.aborted) return undefined;

      const result = await options.requestLanguage(request, { signal: controller.signal });
      if (
        controller.signal.aborted ||
        token?.isCancellationRequested ||
        !this.isCurrentLanguageState(model, generation)
      ) {
        return undefined;
      }
      return result;
    } catch (error) {
      if (
        controller.signal.aborted ||
        token?.isCancellationRequested ||
        !this.isCurrentLanguageState(model, generation) ||
        isSilentLanguageError(error)
      ) {
        return undefined;
      }
      this.reportRuntimeError(error);
      return undefined;
    } finally {
      cancellationDisposable?.dispose();
      this.pendingLanguageRequests.delete(controller);
    }
  }

  private isCurrentLanguageState(model: monaco.editor.ITextModel, generation: number): boolean {
    return (
      !this.disposed &&
      !this.runtimeFailed &&
      model === this.model &&
      generation === this.languageGeneration
    );
  }

  private invalidateLanguageState(model: monaco.editor.ITextModel | null): number {
    this.languageGeneration += 1;
    for (const controller of this.pendingLanguageRequests) controller.abort();
    this.pendingLanguageRequests.clear();
    this.completionItems = new WeakMap();
    if (model) this.clearMarkers(model);
    return this.languageGeneration;
  }

  private clearMarkers(model: monaco.editor.ITextModel): void {
    try {
      monaco.editor.setModelMarkers(model, LANGUAGE_MARKER_OWNER, []);
    } catch {
      // Marker cleanup must not prevent model, provider, or editor teardown.
    }
  }

  private emitCurrentText(): void {
    const model = this.model;
    const options = this.mountOptions;
    if (!model || !options || this.disposed) return;

    try {
      this.modelVersion += 1;
      options.onTextChange(model.getValue(), this.modelVersion);
    } catch (error) {
      this.reportRuntimeError(error);
    }
  }

  private emitSelection(selection: EditorSelection): void {
    const callback = this.mountOptions?.onSelectionChange;
    if (!callback || this.disposed) return;

    try {
      callback(selection);
    } catch (error) {
      this.reportRuntimeError(error);
    }
  }

  private reportRuntimeError(error: unknown): void {
    if (this.runtimeFailed || this.disposed) return;
    this.runtimeFailed = true;
    this.invalidateLanguageState(this.model);
    try {
      this.mountOptions?.onRuntimeError(error);
    } catch {
      // A consumer error must not prevent adapter cleanup by the host.
    }
  }

  private requireEditor(): monaco.editor.IStandaloneCodeEditor {
    if (!this.editor || this.disposed) throw new Error('Monaco source editor is not mounted');
    return this.editor;
  }

  private requireModel(): monaco.editor.ITextModel {
    if (!this.model || this.disposed) throw new Error('Monaco source editor model is not mounted');
    return this.model;
  }

  private releaseResources(): void {
    this.invalidateLanguageState(this.model);

    try {
      this.modelListener?.dispose();
    } catch {
      // Continue releasing editor and model resources.
    } finally {
      this.modelListener = null;
    }

    for (const listener of this.editorListeners.splice(0).reverse()) {
      try {
        listener.dispose();
      } catch {
        // Continue releasing the remaining Monaco resources.
      }
    }

    for (const disposable of this.semanticProviderDisposables.splice(0).reverse()) {
      try {
        disposable.dispose();
      } catch {
        // Continue releasing the remaining semantic providers.
      }
    }

    try {
      this.editor?.dispose();
    } finally {
      this.editor = null;
      try {
        this.model?.dispose();
      } finally {
        this.model = null;
        this.mountOptions = null;
        const releaseLanguageSupport = this.releaseLanguageSupport;
        this.releaseLanguageSupport = null;
        releaseLanguageSupport?.();
      }
    }
  }
}

export function createMonacoSourceEditor(): EditorAdapter {
  return new MonacoSourceEditorAdapter();
}
