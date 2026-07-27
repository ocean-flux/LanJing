import { describe, expect, it, vi } from 'vitest';
import { authoringLimits } from '$lib/rules/authoring';
import type { RuntimePlatform } from '$lib/app/platform-runtime';
import { TextDocument } from 'vscode-languageserver-textdocument';
import type {
  SourceLanguageWorkerInput,
  SourceLanguageWorkerOutput,
} from './source-language-service-protocol';
import { SourceLanguageServiceClient, type SourceLanguageWorker } from './source-language-service';
import { createSourceLanguageWorkerRuntime } from './source-language-service.worker';
import type {
  EditorAdapter,
  EditorAdapterMount,
  EditorCommand,
  EditorKind,
} from './source-editor-adapters';
import { SourceDocumentEditorHost } from './source-document-editor-host';

class FakeEditorAdapter implements EditorAdapter {
  readonly kind: EditorKind;
  mountOptions: EditorAdapterMount | null = null;
  mounted = 0;
  disposed = 0;
  focused = 0;
  documents: Array<{ documentId: string; text: string; modelVersion: number }> = [];
  commands: EditorCommand[] = [];
  throwOnMount = false;

  constructor(kind: EditorKind) {
    this.kind = kind;
  }

  mount(options: EditorAdapterMount): void {
    this.mounted += 1;
    if (this.throwOnMount) throw new Error('mount failed');
    this.mountOptions = options;
  }

  setDocument(documentId: string, text: string, modelVersion: number): void {
    this.documents.push({ documentId, text, modelVersion });
  }

  focus(): void {
    this.focused += 1;
  }

  runCommand(command: EditorCommand): boolean {
    this.commands.push(command);
    return true;
  }

  dispose(): void {
    this.disposed += 1;
  }

  emitText(text: string, modelVersion: number): void {
    this.mountOptions?.onTextChange(text, modelVersion);
  }

  crash(error: unknown = new Error('editor crashed')): void {
    this.mountOptions?.onRuntimeError(error);
  }
}

class InProcessLanguageWorker implements SourceLanguageWorker {
  readonly posted: SourceLanguageWorkerInput[] = [];
  readonly #listeners = new Map<string, Set<EventListenerOrEventListenerObject>>();
  readonly #runtime = createSourceLanguageWorkerRuntime((message: SourceLanguageWorkerOutput) => {
    const event = new MessageEvent('message', { data: message });
    for (const listener of this.#listeners.get('message') ?? []) {
      if (typeof listener === 'function') listener(event);
      else listener.handleEvent(event);
    }
  });
  terminated = false;

  get listenerCount(): number {
    let count = 0;
    for (const listeners of this.#listeners.values()) count += listeners.size;
    return count;
  }

  postMessage(message: SourceLanguageWorkerInput): void {
    if (this.terminated) throw new Error('worker terminated');
    this.posted.push(message);
    void this.#runtime.handleMessage(message);
  }

  addEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
    const listeners = this.#listeners.get(type) ?? new Set<EventListenerOrEventListenerObject>();
    listeners.add(listener);
    this.#listeners.set(type, listeners);
  }

  removeEventListener(type: string, listener: EventListenerOrEventListenerObject): void {
    this.#listeners.get(type)?.delete(listener);
  }

  terminate(): void {
    if (this.terminated) return;
    this.terminated = true;
    this.#runtime.dispose();
  }

  fail(): void {
    const event = new Event('error');
    for (const listener of this.#listeners.get('error') ?? []) {
      if (typeof listener === 'function') listener(event);
      else listener.handleEvent(event);
    }
  }
}

function deferred<T>(): {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
  readonly reject: (error: unknown) => void;
} {
  let resolve = (value: T): void => void value;
  let reject = (error: unknown): void => void error;
  const promise = new Promise<T>((onResolve, onReject) => {
    resolve = onResolve;
    reject = onReject;
  });
  return { promise, resolve, reject };
}

function createHost(
  loadAdapter: (platform: RuntimePlatform) => Promise<EditorAdapter | null>,
  onTextEdit: (text: string) => void = (): void => undefined,
): SourceDocumentEditorHost {
  return new SourceDocumentEditorHost(
    {
      documentId: 'document:one',
      text: '{"value":1}',
      businessEpoch: 7,
    },
    { loadAdapter, onTextEdit },
  );
}

const CREDENTIAL_SENTINEL = '__LANJING_CREDENTIAL_SLOT_V1__:11111111-1111-4111-8111-111111111111';
const SEMANTIC_SOURCE = `{
"bookSourceType":0,
"bookSourceUrl":"https://example.test",
"":0
}`;
const SEMANTIC_DOCUMENT = TextDocument.create(
  'test://source-editor-host.json',
  'json',
  0,
  SEMANTIC_SOURCE,
);

async function createSemanticHost(kind: EditorKind): Promise<{
  readonly host: SourceDocumentEditorHost;
  readonly adapter: FakeEditorAdapter;
  readonly workers: InProcessLanguageWorker[];
  readonly detach: () => void;
}> {
  const adapter = new FakeEditorAdapter(kind);
  const workers: InProcessLanguageWorker[] = [];
  const host = new SourceDocumentEditorHost(
    {
      documentId: 'document:semantic',
      text: SEMANTIC_SOURCE,
      businessEpoch: 1,
    },
    {
      onTextEdit: (): void => undefined,
      loadAdapter: async () => adapter,
      createLanguageService: (onFailure) => {
        const worker = new InProcessLanguageWorker();
        workers.push(worker);
        return new SourceLanguageServiceClient({ workerFactory: () => worker, onFailure });
      },
    },
  );
  const detach = host.attach(document.createElement('div'));
  await host.start(kind === 'monaco' ? 'windows' : 'android');
  return { host, adapter, workers, detach };
}

describe('SourceDocumentEditorHost', () => {
  it('keeps only a controlled masked projection and returns surface edits to the session', async () => {
    const revealedSecret = 'never-enter-the-editor';
    const initialText = `{"header":"${CREDENTIAL_SENTINEL}","value":1}`;
    const editedText = `{"header":"${CREDENTIAL_SENTINEL}","value":2}`;
    const edits: string[] = [];
    const host = new SourceDocumentEditorHost(
      { documentId: 'document:controlled', text: initialText, businessEpoch: 3 },
      { onTextEdit: (text) => edits.push(text) },
    );
    const snapshots: string[] = [];
    const unsubscribe = host.subscribe((snapshot) => snapshots.push(snapshot.text));

    expect(snapshots).toEqual([initialText]);
    host.editText(editedText);
    expect(edits).toEqual([editedText]);
    expect(JSON.stringify({ edits, snapshot: host.getSnapshot() })).not.toContain(revealedSecret);
    expect(host.getSnapshot()).toMatchObject({
      text: editedText,
      businessEpoch: 3,
      adapterModelVersion: 1,
      languageGeneration: 0,
    });

    await host.setDocument({
      documentId: 'document:controlled',
      text: editedText,
      businessEpoch: 4,
    });
    expect(host.getSnapshot()).toMatchObject({ businessEpoch: 4, adapterModelVersion: 1 });
    expect(edits).toHaveLength(1);

    await host.setDocument({
      documentId: 'document:controlled',
      text: initialText,
      businessEpoch: 5,
    });
    expect(host.getSnapshot()).toMatchObject({
      text: initialText,
      businessEpoch: 5,
      adapterModelVersion: 2,
    });
    expect(edits).toHaveLength(1);
    expect(host.getSnapshot()).not.toHaveProperty('savedText');
    expect(host.getSnapshot()).not.toHaveProperty('savedRevision');
    expect(host.getSnapshot()).not.toHaveProperty('dirty');
    expect(host).not.toHaveProperty('save');
    expect(host).not.toHaveProperty('prepare');
    expect(host).not.toHaveProperty('markSaved');
    expect(() =>
      host.setDocument({
        documentId: 'document:controlled',
        text: initialText,
        businessEpoch: 4,
      }),
    ).toThrow('businessEpoch cannot move backwards');

    unsubscribe();
    host.dispose();
  });

  it('exposes only masked credential text to the adapter, worker, and edit callback', async () => {
    const revealedSecret = 'Bearer session-only-secret';
    const maskedText = `{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"Masked","header":"${CREDENTIAL_SENTINEL}"}`;
    const editedMaskedText = maskedText.replace('Masked', 'Edited');
    const adapter = new FakeEditorAdapter('monaco');
    const worker = new InProcessLanguageWorker();
    const edits: string[] = [];
    const host = new SourceDocumentEditorHost(
      { documentId: 'document:credential', text: maskedText, businessEpoch: 0 },
      {
        onTextEdit: (text) => edits.push(text),
        loadAdapter: async () => adapter,
        createLanguageService: (onFailure) =>
          new SourceLanguageServiceClient({ workerFactory: () => worker, onFailure }),
      },
    );
    host.attach(document.createElement('div'));
    await host.start('windows');

    expect(adapter.mountOptions?.text).toBe(maskedText);
    adapter.emitText(editedMaskedText, 1);
    expect(edits).toEqual([editedMaskedText]);
    const validation = await host.requestLanguage({ method: 'validation', payload: {} });
    const request = worker.posted.find((message) => message.type === 'request');
    expect(request?.type).toBe('request');
    if (!request || request.type !== 'request') throw new Error('expected a language request');
    expect(request.payload.text).toBe(editedMaskedText);
    expect(JSON.stringify(validation)).not.toContain(CREDENTIAL_SENTINEL);
    expect(
      JSON.stringify({
        adapterText: adapter.mountOptions?.text,
        edits,
        workerText: request.payload.text,
      }),
    ).not.toContain(revealedSecret);

    host.dispose();
    expect(worker.terminated).toBe(true);
  });

  it('preserves adapter-native history for a controlled echo and applies external text once', async () => {
    const adapter = new FakeEditorAdapter('monaco');
    const onTextEdit = vi.fn();
    const host = createHost(async () => adapter, onTextEdit);
    host.attach(document.createElement('div'));
    await host.start('windows');

    adapter.emitText('{"value":2}', 1);
    expect(onTextEdit).toHaveBeenCalledWith('{"value":2}');
    expect(host.getSnapshot()).toMatchObject({
      text: '{"value":2}',
      businessEpoch: 7,
      adapterModelVersion: 1,
    });

    await host.setDocument({
      documentId: 'document:one',
      text: '{"value":2}',
      businessEpoch: 8,
    });
    expect(adapter.documents).toEqual([]);
    expect(host.getSnapshot().adapterModelVersion).toBe(1);

    await host.setDocument({
      documentId: 'document:one',
      text: '{"value":3}',
      businessEpoch: 9,
    });
    expect(adapter.documents).toEqual([
      { documentId: 'document:one', text: '{"value":3}', modelVersion: 2 },
    ]);
    expect(onTextEdit).toHaveBeenCalledTimes(1);

    await expect(host.runCommand('undo')).resolves.toBe(true);
    await expect(host.runCommand('redo')).resolves.toBe(true);
    expect(adapter.commands).toEqual(['undo', 'redo']);
    host.dispose();
  });

  it('bridges the same canonical masked text to all language methods for both adapter kinds', async () => {
    const results: unknown[] = [];

    for (const kind of ['monaco', 'codemirror'] as const) {
      const { host, workers } = await createSemanticHost(kind);
      expect(workers).toHaveLength(0);

      const validation = await host.requestLanguage({ method: 'validation', payload: {} });
      const completion = await host.requestLanguage({
        method: 'completion',
        payload: { position: SEMANTIC_DOCUMENT.positionAt(SEMANTIC_SOURCE.indexOf('""') + 1) },
      });
      const completionItem = completion?.items.find((item) => item.label === 'bookSourceName');
      expect(completionItem).toBeDefined();
      if (!completionItem) throw new Error('expected a catalog-backed completion item');
      const completionResolve = await host.requestLanguage({
        method: 'completionResolve',
        payload: { item: completionItem },
      });
      const hover = await host.requestLanguage({
        method: 'hover',
        payload: {
          position: SEMANTIC_DOCUMENT.positionAt(SEMANTIC_SOURCE.indexOf('bookSourceUrl') + 2),
        },
      });
      const symbols = await host.requestLanguage({
        method: 'symbols',
        payload: { resultLimit: 100 },
      });
      const format = await host.requestLanguage({
        method: 'format',
        payload: { options: { tabSize: 2, insertSpaces: true } },
      });
      const folding = await host.requestLanguage({
        method: 'folding',
        payload: { rangeLimit: 100 },
      });
      const selectionRanges = await host.requestLanguage({
        method: 'selectionRanges',
        payload: {
          positions: [SEMANTIC_DOCUMENT.positionAt(SEMANTIC_SOURCE.indexOf('bookSourceUrl') + 2)],
        },
      });

      expect(hover).not.toBeNull();
      expect(symbols.length).toBeGreaterThan(0);
      expect(format.length).toBeGreaterThan(0);
      expect(completionResolve.label).toBe('bookSourceName');
      expect(folding.length).toBeGreaterThan(0);
      expect(selectionRanges).toHaveLength(1);
      results.push({
        validation,
        completion,
        completionResolve,
        hover,
        symbols,
        format,
        folding,
        selectionRanges,
      });

      expect(workers).toHaveLength(1);
      const worker = workers[0]!;
      expect(worker.posted).toHaveLength(8);
      for (const message of worker.posted) {
        expect(message.type).toBe('request');
        if (message.type === 'request') expect(message.payload.text).toBe(SEMANTIC_SOURCE);
      }
      host.dispose();
      expect(worker.terminated).toBe(true);
      expect(worker.listenerCount).toBe(0);
      expect(worker.posted.at(-1)?.type).toBe('dispose');
    }

    expect(results[0]).toEqual(results[1]);
  });

  it('drops stale language work on controlled text and document generation changes', async () => {
    const current = await createSemanticHost('monaco');
    const staleText = current.host.requestLanguage({ method: 'validation', payload: {} });
    const nextText = SEMANTIC_SOURCE.replace('example.test', 'next.example.test');
    await current.host.setDocument({
      documentId: 'document:semantic',
      text: nextText,
      businessEpoch: 2,
    });

    await expect(staleText).rejects.toMatchObject({ code: 'stale_document' });
    const firstWorker = current.workers[0]!;
    expect(firstWorker.posted.map((message) => message.type)).toContain('cancel');

    const staleDocument = current.host.requestLanguage({ method: 'symbols', payload: {} });
    await current.host.setDocument({
      documentId: 'document:next',
      text: '{"bookSourceName":"Next"}',
      businessEpoch: 0,
    });
    await expect(staleDocument).rejects.toMatchObject({ code: 'disposed' });
    expect(firstWorker.terminated).toBe(true);
    expect(current.host.getSnapshot()).toMatchObject({
      documentId: 'document:next',
      text: '{"bookSourceName":"Next"}',
      businessEpoch: 0,
      adapterModelVersion: 0,
      languageGeneration: 1,
      status: 'ready',
    });
    current.host.dispose();
  });

  it('falls back atomically when the lazy language worker cannot initialize', async () => {
    const adapter = new FakeEditorAdapter('codemirror');
    const host = new SourceDocumentEditorHost(
      { documentId: 'document:worker-init', text: SEMANTIC_SOURCE, businessEpoch: 1 },
      {
        onTextEdit: (): void => undefined,
        loadAdapter: async () => adapter,
        createLanguageService: () => {
          throw new Error('worker initialization failed');
        },
      },
    );
    host.attach(document.createElement('div'));
    await host.start('ios');

    await expect(host.requestLanguage({ method: 'validation', payload: {} })).rejects.toMatchObject(
      { code: 'worker_failed' },
    );
    expect(adapter.disposed).toBe(1);
    expect(host.getSnapshot()).toMatchObject({
      text: SEMANTIC_SOURCE,
      status: 'fallback',
      editorKind: 'fallback',
      fallbackReason: 'runtime-error',
      advancedStateReset: true,
    });
    host.dispose();
  });

  it('exposes authoring limits while preserving controlled text and skipping the worker', async () => {
    const oversizedText = 'x'.repeat(authoringLimits.max_utf8_bytes + 1);
    const adapter = new FakeEditorAdapter('monaco');
    const edits: string[] = [];
    const createLanguageService = vi.fn(() => {
      throw new Error('language service must not be created for limited text');
    });
    const host = new SourceDocumentEditorHost(
      { documentId: 'document:limited', text: oversizedText, businessEpoch: 1 },
      {
        onTextEdit: (text) => edits.push(text),
        loadAdapter: async () => adapter,
        createLanguageService,
      },
    );
    host.attach(document.createElement('div'));
    await host.start('windows');

    expect(host.getSnapshot()).toMatchObject({
      text: oversizedText,
      languageInputLimit: 'document_too_large',
    });
    await expect(host.requestLanguage({ method: 'validation', payload: {} })).rejects.toMatchObject(
      { code: 'document_too_large' },
    );
    expect(createLanguageService).not.toHaveBeenCalled();

    const tooDeep =
      '{"nested":' +
      '['.repeat(authoringLimits.max_depth + 1) +
      '0' +
      ']'.repeat(authoringLimits.max_depth + 1) +
      '}';
    host.editText(tooDeep);
    expect(edits).toEqual([tooDeep]);
    expect(host.getSnapshot()).toMatchObject({
      text: tooDeep,
      languageInputLimit: 'document_too_complex',
    });
    await expect(host.requestLanguage({ method: 'validation', payload: {} })).rejects.toMatchObject(
      { code: 'document_too_complex' },
    );
    expect(createLanguageService).not.toHaveBeenCalled();
    host.dispose();
  });

  it('disposes a delayed old import and never mounts it after a document switch', async () => {
    const firstImport = deferred<EditorAdapter>();
    const secondImport = deferred<EditorAdapter>();
    const firstAdapter = new FakeEditorAdapter('monaco');
    const secondAdapter = new FakeEditorAdapter('monaco');
    const loadAdapter = vi
      .fn<(platform: RuntimePlatform) => Promise<EditorAdapter | null>>()
      .mockReturnValueOnce(firstImport.promise)
      .mockReturnValueOnce(secondImport.promise);
    const host = createHost(loadAdapter);
    host.attach(document.createElement('div'));

    const firstStart = host.start('windows');
    const switched = host.setDocument({
      documentId: 'document:two',
      text: '{"value":2}',
      businessEpoch: 0,
    });
    await firstStart;
    await Promise.resolve();
    expect(loadAdapter).toHaveBeenCalledTimes(2);

    firstImport.resolve(firstAdapter);
    await Promise.resolve();
    await Promise.resolve();
    expect(firstAdapter.mounted).toBe(0);
    expect(firstAdapter.disposed).toBe(1);

    secondImport.resolve(secondAdapter);
    await switched;
    expect(secondAdapter.mounted).toBe(1);
    expect(secondAdapter.mountOptions).toMatchObject({
      documentId: 'document:two',
      text: '{"value":2}',
      modelVersion: 0,
    });
    expect(host.getSnapshot()).toMatchObject({
      documentId: 'document:two',
      businessEpoch: 0,
      languageGeneration: 1,
      status: 'ready',
      editorKind: 'monaco',
    });

    host.dispose();
    expect(secondAdapter.disposed).toBe(1);
  });

  it('keeps canonical masked text across runtime fallback, retry, and platform replacement', async () => {
    const firstAdapter = new FakeEditorAdapter('monaco');
    const retryAdapter = new FakeEditorAdapter('monaco');
    const mobileAdapter = new FakeEditorAdapter('codemirror');
    const adapters = [firstAdapter, retryAdapter, mobileAdapter];
    const loadAdapter = vi.fn(async () => adapters.shift() ?? null);
    const selections: Array<{ anchor: number; head: number }> = [];
    const edits: string[] = [];
    const host = new SourceDocumentEditorHost(
      { documentId: 'document:runtime', text: '{"value":1}', businessEpoch: 7 },
      {
        loadAdapter,
        onTextEdit: (text) => edits.push(text),
        onSelectionChange: (selection) => selections.push(selection),
      },
    );
    host.attach(document.createElement('div'));
    await host.start('windows');

    firstAdapter.emitText('{"value":2}', 1);
    firstAdapter.mountOptions?.onSelectionChange?.({ anchor: 2, head: 5 });
    expect(edits).toEqual(['{"value":2}']);
    expect(selections).toEqual([{ anchor: 2, head: 5 }]);
    firstAdapter.crash();
    expect(firstAdapter.disposed).toBe(1);
    expect(host.getSnapshot()).toMatchObject({
      text: '{"value":2}',
      businessEpoch: 7,
      adapterModelVersion: 1,
      languageGeneration: 1,
      status: 'fallback',
      editorKind: 'fallback',
      fallbackReason: 'runtime-error',
      advancedStateReset: true,
      focusTarget: 'fallback',
      focusRequestId: 1,
    });

    firstAdapter.emitText('{"stale":true}', 2);
    expect(host.getSnapshot().text).toBe('{"value":2}');
    expect(edits).toHaveLength(1);

    await host.retry();
    expect(retryAdapter.mountOptions).toMatchObject({ text: '{"value":2}', modelVersion: 1 });
    expect(retryAdapter.mountOptions).not.toHaveProperty('selection');
    expect(retryAdapter.focused).toBe(1);
    expect(host.getSnapshot()).toMatchObject({
      text: '{"value":2}',
      businessEpoch: 7,
      languageGeneration: 2,
      editorKind: 'monaco',
      status: 'ready',
      advancedStateReset: true,
    });

    await host.start('android');
    expect(retryAdapter.disposed).toBe(1);
    expect(mobileAdapter.mountOptions).toMatchObject({ text: '{"value":2}', modelVersion: 1 });
    expect(host.getSnapshot()).toMatchObject({
      text: '{"value":2}',
      languageGeneration: 3,
      editorKind: 'codemirror',
      status: 'ready',
    });

    host.dispose();
    expect(mobileAdapter.disposed).toBe(1);
  });

  it('releases a failed mount and supports a clean retry', async () => {
    const failedAdapter = new FakeEditorAdapter('monaco');
    failedAdapter.throwOnMount = true;
    const recoveredAdapter = new FakeEditorAdapter('monaco');
    const loadAdapter = vi
      .fn<(platform: RuntimePlatform) => Promise<EditorAdapter | null>>()
      .mockResolvedValueOnce(failedAdapter)
      .mockResolvedValueOnce(recoveredAdapter);
    const host = createHost(loadAdapter);
    host.attach(document.createElement('div'));

    await host.start('linux');
    expect(failedAdapter.disposed).toBe(1);
    expect(host.getSnapshot()).toMatchObject({
      status: 'fallback',
      editorKind: 'fallback',
      fallbackReason: 'mount-failed',
      advancedStateReset: true,
    });

    await host.retry();
    expect(recoveredAdapter.mounted).toBe(1);
    expect(host.getSnapshot()).toMatchObject({ status: 'ready', editorKind: 'monaco' });
    host.dispose();
  });

  it('disposes a late adapter after owner teardown without publishing stale readiness', async () => {
    const delayedImport = deferred<EditorAdapter>();
    const adapter = new FakeEditorAdapter('monaco');
    const host = createHost(async () => delayedImport.promise);
    host.attach(document.createElement('div'));
    const starting = host.start('macos');
    await Promise.resolve();

    host.dispose();
    await starting;
    delayedImport.resolve(adapter);
    await Promise.resolve();
    await Promise.resolve();

    expect(adapter.mounted).toBe(0);
    expect(adapter.disposed).toBe(1);
    expect(host.getSnapshot()).toMatchObject({ status: 'disposed', editorKind: 'fallback' });
  });

  it('detaches workers and adapters and exposes the native-history reset on reattach', async () => {
    const firstAdapter = new FakeEditorAdapter('codemirror');
    const secondAdapter = new FakeEditorAdapter('codemirror');
    const loadAdapter = vi
      .fn<(platform: RuntimePlatform) => Promise<EditorAdapter | null>>()
      .mockResolvedValueOnce(firstAdapter)
      .mockResolvedValueOnce(secondAdapter);
    const workers: InProcessLanguageWorker[] = [];
    const host = new SourceDocumentEditorHost(
      { documentId: 'document:detach', text: SEMANTIC_SOURCE, businessEpoch: 1 },
      {
        onTextEdit: (): void => undefined,
        loadAdapter,
        createLanguageService: (onFailure) => {
          const worker = new InProcessLanguageWorker();
          workers.push(worker);
          return new SourceLanguageServiceClient({ workerFactory: () => worker, onFailure });
        },
      },
    );
    const detach = host.attach(document.createElement('div'));
    await host.start('ios');
    await host.requestLanguage({ method: 'validation', payload: {} });
    expect(host.getSnapshot()).toMatchObject({ status: 'ready', languageGeneration: 0 });

    detach();
    expect(firstAdapter.disposed).toBe(1);
    expect(workers[0]?.terminated).toBe(true);
    expect(host.getSnapshot()).toMatchObject({
      status: 'idle',
      languageGeneration: 1,
      advancedStateReset: true,
    });

    host.attach(document.createElement('div'));
    await Promise.resolve();
    await Promise.resolve();
    expect(secondAdapter.mounted).toBe(1);
    expect(host.getSnapshot()).toMatchObject({
      status: 'ready',
      languageGeneration: 1,
      advancedStateReset: true,
    });
    host.dispose();
    expect(secondAdapter.disposed).toBe(1);
  });

  it('uses fallback for unknown and does not invent a reset when the OS later resolves', async () => {
    const adapter = new FakeEditorAdapter('monaco');
    const loadAdapter = vi.fn(async () => adapter);
    const host = createHost(loadAdapter);
    host.attach(document.createElement('div'));

    await host.start('unknown');
    expect(loadAdapter).not.toHaveBeenCalled();
    expect(host.getSnapshot()).toMatchObject({
      status: 'fallback',
      editorKind: 'fallback',
      fallbackReason: 'unsupported-platform',
      focusTarget: 'fallback',
      focusRequestId: 0,
      languageGeneration: 0,
      advancedStateReset: false,
    });

    host.focus();
    expect(host.getSnapshot()).toMatchObject({ focusTarget: 'fallback', focusRequestId: 1 });

    await host.start('windows');
    expect(loadAdapter).toHaveBeenCalledOnce();
    expect(adapter.mounted).toBe(1);
    expect(host.getSnapshot()).toMatchObject({
      status: 'ready',
      editorKind: 'monaco',
      languageGeneration: 0,
      advancedStateReset: false,
    });
    host.focus();
    expect(adapter.focused).toBe(1);
    host.dispose();
  });
});
