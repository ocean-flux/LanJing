import {
  autocompletion,
  snippet,
  type Completion,
  type CompletionContext,
  type CompletionInfo,
  type CompletionResult,
} from '@codemirror/autocomplete';
import { history, historyKeymap, isolateHistory, redo, undo } from '@codemirror/commands';
import { json } from '@codemirror/lang-json';
import { foldGutter, foldKeymap } from '@codemirror/language';
import { forceLinting, linter, type Diagnostic } from '@codemirror/lint';
import { EditorState, Transaction, type ChangeSpec, type Text } from '@codemirror/state';
import { EditorView, hoverTooltip, keymap, type Tooltip, type ViewUpdate } from '@codemirror/view';
import type {
  CompletionItem,
  CompletionList,
  Hover,
  Position,
  Range,
  TextEdit,
} from 'vscode-json-languageservice';
import type {
  EditorAdapter,
  EditorAdapterMount,
  EditorCommand,
  EditorSelection,
} from './source-editor-adapters';

const MOBILE_EDITOR_THEME = EditorView.theme({
  '&': {
    height: '100%',
    minHeight: '0',
  },
  '.cm-scroller': {
    overflow: 'auto',
    touchAction: 'pan-x pan-y',
    WebkitOverflowScrolling: 'touch',
  },
  '.cm-content': {
    minHeight: '100%',
  },
});

type LanguageRequestKind = 'validation' | 'completion' | 'completionResolve' | 'hover' | 'format';

const LANGUAGE_REQUEST_KINDS: readonly LanguageRequestKind[] = [
  'validation',
  'completion',
  'completionResolve',
  'hover',
  'format',
];

interface LanguageRequestSlot {
  controller: AbortController | null;
  token: number;
}

interface LanguageRequestContext {
  readonly kind: LanguageRequestKind;
  readonly controller: AbortController;
  readonly token: number;
  readonly view: EditorView;
  readonly document: Text;
  readonly documentId: string;
  readonly modelVersion: number;
}

interface CodeMirrorRange {
  readonly from: number;
  readonly to: number;
}

interface CompletionCandidate {
  readonly item: CompletionItem;
  readonly range: CodeMirrorRange;
  readonly insertText: string;
  readonly snippetTemplate: string | null;
}

function codeMirrorOffsetToLspPosition(document: Text, offset: number): Position | null {
  if (!Number.isSafeInteger(offset) || offset < 0 || offset > document.length) return null;
  const line = document.lineAt(offset);
  return { line: line.number - 1, character: offset - line.from };
}

function lspPositionToCodeMirrorOffset(document: Text, position: Position): number | null {
  if (
    !Number.isSafeInteger(position.line) ||
    position.line < 0 ||
    !Number.isSafeInteger(position.character) ||
    position.character < 0 ||
    position.line >= document.lines
  ) {
    return null;
  }

  const line = document.line(position.line + 1);
  if (position.character > line.length) return null;
  return line.from + position.character;
}

function lspRangeToCodeMirrorRange(document: Text, range: Range): CodeMirrorRange | null {
  const from = lspPositionToCodeMirrorOffset(document, range.start);
  const to = lspPositionToCodeMirrorOffset(document, range.end);
  return from === null || to === null || from > to ? null : { from, to };
}

function completionType(kind: CompletionItem['kind']): string | undefined {
  switch (kind) {
    case 2:
      return 'method';
    case 3:
      return 'function';
    case 4:
      return 'class';
    case 5:
    case 10:
      return 'property';
    case 6:
      return 'variable';
    case 7:
      return 'class';
    case 8:
      return 'interface';
    case 9:
      return 'namespace';
    case 13:
      return 'enum';
    case 14:
      return 'keyword';
    case 15:
      return 'text';
    case 20:
      return 'enum';
    case 21:
      return 'constant';
    case 22:
      return 'type';
    case 25:
      return 'type';
    default:
      return kind === undefined ? undefined : 'text';
  }
}

function translateLspSnippet(template: string): string | null {
  let translated = '';
  let index = 0;

  while (index < template.length) {
    if (template[index] !== '$') {
      translated += template[index];
      index += 1;
      continue;
    }

    const numberedStop = /^\$(\d+)/.exec(template.slice(index));
    if (numberedStop) {
      translated += `\${${numberedStop[1]}}`;
      index += numberedStop[0].length;
      continue;
    }

    const placeholder = /^\$\{(\d+)(?::([^{}$]*))?\}/.exec(template.slice(index));
    if (!placeholder) return null;
    translated +=
      placeholder[2] === undefined
        ? `\${${placeholder[1]}}`
        : `\${${placeholder[1]}:${placeholder[2]}}`;
    index += placeholder[0].length;
  }

  return translated;
}

function markupText(value: unknown): string {
  if (typeof value === 'string') return value;
  if (!value || typeof value !== 'object') return '';
  const candidate = value as { readonly value?: unknown };
  return typeof candidate.value === 'string' ? candidate.value : '';
}

function hoverText(hover: Hover): string {
  const entries = Array.isArray(hover.contents) ? hover.contents : [hover.contents];
  return entries
    .map((entry) => markupText(entry))
    .filter((entry) => entry.length > 0)
    .join('\n\n');
}

function completionInfoText(item: CompletionItem): string {
  const parts = [item.detail ?? '', markupText(item.documentation)].filter(
    (part, index, all) => part.length > 0 && all.indexOf(part) === index,
  );
  return parts.join('\n\n');
}

function plainTextNode(className: string, text: string): HTMLElement {
  const element = document.createElement('div');
  element.className = className;
  element.textContent = text;
  return element;
}

function isCancelledLanguageError(error: unknown): boolean {
  if (error instanceof DOMException && error.name === 'AbortError') return true;
  if (!error || typeof error !== 'object') return false;
  const candidate = error as { readonly code?: unknown; readonly name?: unknown };
  return (
    candidate.name === 'AbortError' ||
    candidate.code === 'cancelled' ||
    candidate.code === 'disposed' ||
    candidate.code === 'stale_document'
  );
}

class CodeMirrorSourceEditorAdapter implements EditorAdapter {
  readonly kind = 'codemirror' as const;

  private view: EditorView | null = null;
  private mountOptions: EditorAdapterMount | null = null;
  private documentId = '';
  private modelVersion = 0;
  private suppressUpdates = false;
  private composing = false;
  private pendingCompositionText: string | null = null;
  private compositionFlushId = 0;
  private languageStartupId = 0;
  private languageRequestsEnabled = false;
  private runtimeFailed = false;
  private disposed = false;
  private readonly languageRequests: Record<LanguageRequestKind, LanguageRequestSlot> = {
    validation: { controller: null, token: 0 },
    completion: { controller: null, token: 0 },
    completionResolve: { controller: null, token: 0 },
    hover: { controller: null, token: 0 },
    format: { controller: null, token: 0 },
  };

  mount(options: EditorAdapterMount): void {
    if (this.disposed) throw new Error('Cannot mount a disposed CodeMirror source editor');
    if (this.view) throw new Error('CodeMirror source editor is already mounted');

    this.mountOptions = options;
    this.documentId = options.documentId;
    this.modelVersion = options.modelVersion;

    try {
      this.view = new EditorView({
        state: this.createState(options.text),
        parent: options.container,
      });
      const view = this.view;
      const startupId = ++this.languageStartupId;
      queueMicrotask(() => {
        if (this.disposed || this.view !== view || startupId !== this.languageStartupId) return;
        this.languageRequestsEnabled = true;
        try {
          forceLinting(view);
        } catch (error) {
          this.reportRuntimeError(error);
        }
      });
    } catch (error) {
      this.view?.destroy();
      this.view = null;
      this.mountOptions = null;
      this.disposed = true;
      throw error;
    }
  }

  setDocument(documentId: string, text: string, modelVersion: number): void {
    const view = this.requireView();
    this.abortLanguageRequests();

    try {
      this.suppressUpdates = true;
      this.composing = false;
      this.pendingCompositionText = null;
      this.compositionFlushId += 1;
      this.modelVersion = modelVersion;

      if (documentId === this.documentId) {
        const currentText = view.state.doc.toString();
        if (currentText !== text) {
          view.dispatch({
            changes: { from: 0, to: view.state.doc.length, insert: text },
            annotations: Transaction.addToHistory.of(false),
          });
        }
      } else {
        this.documentId = documentId;
        view.setState(this.createState(text));
      }
    } catch (error) {
      this.reportRuntimeError(error);
      throw error;
    } finally {
      this.suppressUpdates = false;
    }

    if (this.languageRequestsEnabled) {
      try {
        forceLinting(view);
      } catch (error) {
        this.reportRuntimeError(error);
        throw error;
      }
    }
  }

  focus(): void {
    try {
      this.requireView().focus();
    } catch (error) {
      this.reportRuntimeError(error);
      throw error;
    }
  }

  async runCommand(command: EditorCommand): Promise<boolean> {
    const view = this.requireView();

    try {
      if (command === 'undo') return undo(view);
      if (command === 'redo') return redo(view);
      if (this.composing || view.composing) return false;
      return await this.formatDocument(view);
    } catch (error) {
      if (this.disposed || isCancelledLanguageError(error)) return false;
      this.reportRuntimeError(error);
      throw error;
    }
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.languageRequestsEnabled = false;
    this.languageStartupId += 1;
    this.compositionFlushId += 1;
    this.pendingCompositionText = null;
    this.abortLanguageRequests();
    try {
      this.view?.destroy();
    } finally {
      this.view = null;
      this.mountOptions = null;
    }
  }

  private createState(text: string): EditorState {
    const ariaLabel =
      this.mountOptions?.container.getAttribute('aria-label') ?? 'Source JSON editor';
    return EditorState.create({
      doc: text,
      extensions: [
        history(),
        keymap.of([...historyKeymap, ...foldKeymap]),
        json(),
        foldGutter(),
        linter((view) => this.requestDiagnostics(view), { delay: 200 }),
        autocompletion({
          override: [(context) => this.requestCompletions(context)],
        }),
        hoverTooltip((view, position) => this.requestHover(view, position), {
          hideOnChange: 'touch',
        }),
        EditorView.lineWrapping,
        MOBILE_EDITOR_THEME,
        EditorView.contentAttributes.of({
          'aria-label': ariaLabel,
          autocapitalize: 'off',
          autocorrect: 'off',
          spellcheck: 'false',
        }),
        EditorView.updateListener.of((update) => {
          this.handleUpdate(update);
        }),
        EditorView.domEventHandlers({
          compositionstart: () => {
            if (this.suppressUpdates) return false;
            this.composing = true;
            this.pendingCompositionText = null;
            this.compositionFlushId += 1;
            this.abortLanguageRequests();
            return false;
          },
          compositionend: (_event, view) => {
            if (this.suppressUpdates) return false;
            this.composing = false;
            const flushId = ++this.compositionFlushId;
            queueMicrotask(() => {
              if (
                this.disposed ||
                this.suppressUpdates ||
                flushId !== this.compositionFlushId ||
                this.pendingCompositionText === null
              ) {
                return;
              }
              const text = view.state.doc.toString();
              this.pendingCompositionText = null;
              this.emitText(text);
              if (this.languageRequestsEnabled) {
                try {
                  forceLinting(view);
                } catch (error) {
                  this.reportRuntimeError(error);
                }
              }
            });
            return false;
          },
        }),
        EditorView.exceptionSink.of((error) => {
          this.reportRuntimeError(error);
        }),
      ],
    });
  }

  private handleUpdate(update: ViewUpdate): void {
    if (this.suppressUpdates || this.disposed) return;

    if (update.docChanged) {
      this.abortLanguageRequests();
      const text = update.state.doc.toString();
      if (this.composing || update.view.composing) {
        this.pendingCompositionText = text;
      } else {
        this.pendingCompositionText = null;
        this.compositionFlushId += 1;
        this.emitText(text);
      }
    }

    if (update.selectionSet) {
      const selection = update.state.selection.main;
      this.emitSelection({ anchor: selection.anchor, head: selection.head });
    }
  }

  private async requestDiagnostics(view: EditorView): Promise<readonly Diagnostic[]> {
    const request = this.beginLanguageRequest('validation', view);
    const options = this.mountOptions;
    if (!request || !options) return [];

    try {
      const diagnostics = await options.requestLanguage(
        { method: 'validation', payload: {} },
        { signal: request.controller.signal },
      );
      if (!this.isLanguageRequestCurrent(request)) return [];

      const mapped: Diagnostic[] = [];
      for (const diagnostic of diagnostics) {
        const range = lspRangeToCodeMirrorRange(request.document, diagnostic.range);
        if (!range) continue;
        mapped.push({
          ...range,
          message: diagnostic.message,
          severity: diagnostic.severity === 'information' ? 'info' : diagnostic.severity,
          source: diagnostic.source,
        });
      }
      return mapped;
    } catch (error) {
      if (!this.isLanguageRequestCurrent(request) || isCancelledLanguageError(error)) return [];
      throw error;
    } finally {
      this.finishLanguageRequest(request);
    }
  }

  private async requestCompletions(context: CompletionContext): Promise<CompletionResult | null> {
    const view = context.view;
    if (!view || context.aborted) return null;
    const position = codeMirrorOffsetToLspPosition(context.state.doc, context.pos);
    if (!position) return null;

    const request = this.beginLanguageRequest('completion', view, context.state.doc);
    const options = this.mountOptions;
    if (!request || !options) return null;
    context.addEventListener('abort', () => request.controller.abort(), { onDocChange: true });

    try {
      const result = await options.requestLanguage(
        { method: 'completion', payload: { position } },
        { signal: request.controller.signal },
      );
      if (!result || !this.isLanguageRequestCurrent(request) || context.aborted) return null;
      return this.mapCompletionResult(result, context, request);
    } catch (error) {
      if (!this.isLanguageRequestCurrent(request) || isCancelledLanguageError(error)) return null;
      throw error;
    } finally {
      this.finishLanguageRequest(request);
    }
  }

  private mapCompletionResult(
    result: CompletionList,
    context: CompletionContext,
    request: LanguageRequestContext,
  ): CompletionResult | null {
    const word = context.matchBefore(/[\w$-]*/);
    const fallbackRange = { from: word?.from ?? context.pos, to: context.pos };
    const candidates: CompletionCandidate[] = [];

    for (const item of result.items) {
      if (item.command || (item.additionalTextEdits?.length ?? 0) > 0) continue;

      let range = fallbackRange;
      let insertText = item.textEditText ?? item.insertText ?? item.label;
      if (item.textEdit) {
        if (!('range' in item.textEdit)) continue;
        const mapped = lspRangeToCodeMirrorRange(request.document, item.textEdit.range);
        if (!mapped) continue;
        range = mapped;
        insertText = item.textEdit.newText;
      }
      if (range.from > context.pos || range.to < context.pos) continue;

      const insertTextFormat = item.insertTextFormat ?? result.itemDefaults?.insertTextFormat ?? 1;
      if (insertTextFormat !== 1 && insertTextFormat !== 2) continue;
      const snippetTemplate = insertTextFormat === 2 ? translateLspSnippet(insertText) : null;
      if (insertTextFormat === 2 && snippetTemplate === null) continue;
      candidates.push({ item, range, insertText, snippetTemplate });
    }

    const completionRange =
      candidates.find((candidate) => candidate.range !== fallbackRange)?.range ?? fallbackRange;
    const options = candidates
      .filter(
        (candidate) =>
          candidate.range.from === completionRange.from &&
          candidate.range.to === completionRange.to,
      )
      .map((candidate): Completion => {
        const item = candidate.item;
        const type = completionType(item.kind);
        const commitCharacters = (
          item.commitCharacters ?? result.itemDefaults?.commitCharacters
        )?.filter((character) => character.length === 1);
        return {
          label: item.label,
          ...(item.sortText ? { sortText: item.sortText } : {}),
          ...(item.detail ? { detail: item.detail } : {}),
          ...(type ? { type } : {}),
          ...(item.preselect ? { boost: 1 } : {}),
          ...(commitCharacters?.length ? { commitCharacters } : {}),
          apply:
            candidate.snippetTemplate === null
              ? candidate.insertText
              : snippet(candidate.snippetTemplate),
          info: () => this.resolveCompletionInfo(item, request),
        };
      });

    if (options.length === 0) return null;
    return {
      from: completionRange.from,
      to: completionRange.to,
      options,
      filter: false,
    };
  }

  private async resolveCompletionInfo(
    item: CompletionItem,
    completionRequest: LanguageRequestContext,
  ): Promise<CompletionInfo> {
    if (!this.isLanguageDocumentCurrent(completionRequest)) return null;
    const view = completionRequest.view;
    const request = this.beginLanguageRequest(
      'completionResolve',
      view,
      completionRequest.document,
    );
    const options = this.mountOptions;
    if (!request || !options) return null;

    try {
      const resolved = await options.requestLanguage(
        { method: 'completionResolve', payload: { item } },
        { signal: request.controller.signal },
      );
      if (!this.isLanguageRequestCurrent(request)) return null;
      const text = completionInfoText(resolved) || completionInfoText(item);
      return text.length > 0 ? plainTextNode('cm-source-language-completion-info', text) : null;
    } catch (error) {
      if (!this.isLanguageRequestCurrent(request) || isCancelledLanguageError(error)) return null;
      throw error;
    } finally {
      this.finishLanguageRequest(request);
    }
  }

  private async requestHover(view: EditorView, position: number): Promise<Tooltip | null> {
    const lspPosition = codeMirrorOffsetToLspPosition(view.state.doc, position);
    if (!lspPosition) return null;
    const request = this.beginLanguageRequest('hover', view);
    const options = this.mountOptions;
    if (!request || !options) return null;

    try {
      const hover = await options.requestLanguage(
        { method: 'hover', payload: { position: lspPosition } },
        { signal: request.controller.signal },
      );
      if (!hover || !this.isLanguageRequestCurrent(request)) return null;
      const text = hoverText(hover);
      if (text.length === 0) return null;
      const range = hover.range
        ? lspRangeToCodeMirrorRange(request.document, hover.range)
        : { from: position, to: position };
      if (!range) return null;

      return {
        pos: range.from,
        ...(range.to !== range.from ? { end: range.to } : {}),
        create: () => ({
          dom: plainTextNode('cm-source-language-hover', text),
        }),
      };
    } catch (error) {
      if (!this.isLanguageRequestCurrent(request) || isCancelledLanguageError(error)) return null;
      throw error;
    } finally {
      this.finishLanguageRequest(request);
    }
  }

  private async formatDocument(view: EditorView): Promise<boolean> {
    const request = this.beginLanguageRequest('format', view);
    const options = this.mountOptions;
    if (!request || !options) return false;

    try {
      const edits = await options.requestLanguage(
        {
          method: 'format',
          payload: { options: { tabSize: 2, insertSpaces: true } },
        },
        { signal: request.controller.signal },
      );
      if (!this.isLanguageRequestCurrent(request)) return false;
      const changes = this.mapFormatEdits(request.document, edits);
      if (!changes) throw new Error('Source language format returned invalid text edits');
      if (changes.length === 0) return false;

      view.dispatch({
        changes,
        annotations: isolateHistory.of('full'),
        userEvent: 'input.format',
      });
      return true;
    } catch (error) {
      if (!this.isLanguageRequestCurrent(request) || isCancelledLanguageError(error)) return false;
      throw error;
    } finally {
      this.finishLanguageRequest(request);
    }
  }

  private mapFormatEdits(document: Text, edits: readonly TextEdit[]): readonly ChangeSpec[] | null {
    const changes: Array<CodeMirrorRange & { readonly insert: string }> = [];
    for (const edit of edits) {
      const range = lspRangeToCodeMirrorRange(document, edit.range);
      if (!range || typeof edit.newText !== 'string') return null;
      changes.push({ ...range, insert: edit.newText });
    }
    changes.sort((left, right) => left.from - right.from || left.to - right.to);
    for (let index = 1; index < changes.length; index += 1) {
      const previous = changes[index - 1];
      const current = changes[index];
      if (current.from < previous.to || current.from === previous.from) return null;
    }
    return changes;
  }

  private beginLanguageRequest(
    kind: LanguageRequestKind,
    view: EditorView,
    document: Text = view.state.doc,
  ): LanguageRequestContext | null {
    if (
      !this.languageRequestsEnabled ||
      this.disposed ||
      this.composing ||
      view.composing ||
      this.view !== view ||
      view.state.doc !== document
    ) {
      return null;
    }

    const slot = this.languageRequests[kind];
    slot.controller?.abort();
    slot.token += 1;
    const controller = new AbortController();
    slot.controller = controller;
    return {
      kind,
      controller,
      token: slot.token,
      view,
      document,
      documentId: this.documentId,
      modelVersion: this.modelVersion,
    };
  }

  private finishLanguageRequest(request: LanguageRequestContext): void {
    const slot = this.languageRequests[request.kind];
    if (slot.token === request.token && slot.controller === request.controller) {
      slot.controller = null;
    }
  }

  private isLanguageDocumentCurrent(request: LanguageRequestContext): boolean {
    return (
      this.languageRequestsEnabled &&
      !this.disposed &&
      this.view === request.view &&
      request.view.state.doc === request.document &&
      this.documentId === request.documentId &&
      this.modelVersion === request.modelVersion
    );
  }

  private isLanguageRequestCurrent(request: LanguageRequestContext): boolean {
    const slot = this.languageRequests[request.kind];
    return (
      this.isLanguageDocumentCurrent(request) &&
      !request.controller.signal.aborted &&
      slot.token === request.token &&
      slot.controller === request.controller
    );
  }

  private abortLanguageRequests(): void {
    for (const kind of LANGUAGE_REQUEST_KINDS) {
      const slot = this.languageRequests[kind];
      slot.token += 1;
      slot.controller?.abort();
      slot.controller = null;
    }
  }

  private emitText(text: string): void {
    const callback = this.mountOptions?.onTextChange;
    if (!callback || this.disposed) return;

    try {
      this.modelVersion += 1;
      callback(text, this.modelVersion);
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
    try {
      this.mountOptions?.onRuntimeError(error);
    } catch {
      // A consumer error must not prevent adapter cleanup by the host.
    }
  }

  private requireView(): EditorView {
    if (!this.view || this.disposed) throw new Error('CodeMirror source editor is not mounted');
    return this.view;
  }
}

export function createCodeMirrorSourceEditor(): EditorAdapter {
  return new CodeMirrorSourceEditorAdapter();
}
