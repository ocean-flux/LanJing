import { afterEach, describe, expect, it, vi } from 'vitest';
import { authoringLimits } from '$lib/rules/authoring';
import type { CompletionItem } from 'vscode-json-languageservice';
import { TextDocument } from 'vscode-languageserver-textdocument';
import {
  SOURCE_LANGUAGE_PROTOCOL_VERSION,
  type SourceLanguageMethod,
  type SourceLanguageRequest,
  type SourceLanguageRequestPayloadMap,
  type SourceLanguageResult,
  type SourceLanguageVersion,
  type SourceLanguageWorkerInput,
  type SourceLanguageWorkerOutput,
} from './source-language-service-protocol';
import { SourceLanguageServiceClient, type SourceLanguageWorker } from './source-language-service';
import {
  createSourceLanguageWorkerRuntime,
  type SourceLanguageWorkerRuntime,
} from './source-language-service.worker';

const BASE_VERSION: SourceLanguageVersion = {
  documentId: 'source-document-1',
  documentEpoch: 4,
  modelVersion: 1,
};
const CREDENTIAL_MARKER = '__LANJING_CREDENTIAL_SLOT_V1__:11111111-1111-4111-8111-111111111111';
const VALID_SOURCE = `{
  "bookSourceType": 0,
  "bookSourceUrl": "https://example.test",
  "bookSourceName": "Example",
  "ruleSearch": {
    "bookList": ".result",
    "name": ".title"
  }
}`;

function createRequest<M extends SourceLanguageMethod>(
  requestId: string,
  method: M,
  payload: SourceLanguageRequestPayloadMap[M],
  version: SourceLanguageVersion = BASE_VERSION,
): SourceLanguageRequest<M> {
  return {
    type: 'request',
    protocolVersion: SOURCE_LANGUAGE_PROTOCOL_VERSION,
    requestId,
    ...version,
    method,
    payload,
  } as SourceLanguageRequest<M>;
}

async function dispatch<M extends SourceLanguageMethod>(
  runtime: SourceLanguageWorkerRuntime,
  messages: SourceLanguageWorkerOutput[],
  request: SourceLanguageRequest<M>,
): Promise<SourceLanguageResult<M>> {
  const priorCount = messages.length;
  await runtime.handleMessage(request);
  expect(messages).toHaveLength(priorCount + 1);
  return messages[priorCount] as SourceLanguageResult<M>;
}

class FakeWorker implements SourceLanguageWorker {
  readonly posted: SourceLanguageWorkerInput[] = [];
  readonly #listeners = new Map<string, Set<EventListenerOrEventListenerObject>>();
  terminated = false;

  get listenerCount(): number {
    let count = 0;
    for (const listeners of this.#listeners.values()) count += listeners.size;
    return count;
  }

  postMessage(message: SourceLanguageWorkerInput): void {
    this.posted.push(message);
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
    this.terminated = true;
  }

  emit(message: unknown): void {
    const event = new MessageEvent('message', { data: message });
    for (const listener of this.#listeners.get('message') ?? []) {
      if (typeof listener === 'function') listener(event);
      else listener.handleEvent(event);
    }
  }
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('source language worker protocol', () => {
  it('serves every local JSON language method with schema and catalog hints', async () => {
    const fetchTrap = vi.fn(() => {
      throw new Error('network is forbidden');
    });
    const webSocketTrap = vi.fn();
    vi.stubGlobal('fetch', fetchTrap);
    vi.stubGlobal(
      'WebSocket',
      class {
        constructor(...args: unknown[]) {
          webSocketTrap(...args);
        }
      },
    );

    const messages: SourceLanguageWorkerOutput[] = [];
    const runtime = createSourceLanguageWorkerRuntime((message) => messages.push(message));

    const externalSchemaSource = VALID_SOURCE.replace(
      '{',
      '{\n  "$schema": "https://schemas.example.invalid/remote.json",',
    );
    const externalSchemaValidation = await dispatch(
      runtime,
      messages,
      createRequest(
        'external-schema',
        'validation',
        { text: externalSchemaSource },
        { ...BASE_VERSION, modelVersion: 0 },
      ),
    );
    expect(externalSchemaValidation).toMatchObject({ ok: true, method: 'validation' });

    const validation = await dispatch(
      runtime,
      messages,
      createRequest('validation', 'validation', { text: VALID_SOURCE }),
    );
    expect(validation).toMatchObject({ ok: true, method: 'validation' });
    if (!validation.ok) throw new Error('validation failed');
    expect(validation.result.filter((diagnostic) => diagnostic.severity === 'error')).toEqual([]);

    const completionText = '{\n  "": 0\n}';
    const completionDocument = TextDocument.create(
      'test://completion.json',
      'json',
      2,
      completionText,
    );
    const completion = await dispatch(
      runtime,
      messages,
      createRequest(
        'completion',
        'completion',
        {
          text: completionText,
          position: completionDocument.positionAt(completionText.indexOf('""') + 1),
        },
        { ...BASE_VERSION, modelVersion: 2 },
      ),
    );
    expect(completion).toMatchObject({ ok: true, method: 'completion' });
    if (!completion.ok || !completion.result) throw new Error('completion failed');
    const knownField = completion.result.items.find((item) => item.label === 'bookSourceName');
    expect(knownField).toMatchObject({
      label: 'bookSourceName',
      detail: 'Legado · basic · executable',
    });

    const completionResolve = await dispatch(
      runtime,
      messages,
      createRequest(
        'completion-resolve',
        'completionResolve',
        { text: completionText, item: knownField as CompletionItem },
        { ...BASE_VERSION, modelVersion: 2 },
      ),
    );
    expect(completionResolve).toMatchObject({
      ok: true,
      method: 'completionResolve',
      result: { label: 'bookSourceName' },
    });

    const richDocument = TextDocument.create('test://source.json', 'json', 3, VALID_SOURCE);
    const hover = await dispatch(
      runtime,
      messages,
      createRequest(
        'hover',
        'hover',
        {
          text: VALID_SOURCE,
          position: richDocument.positionAt(VALID_SOURCE.indexOf('bookSourceName') + 2),
        },
        { ...BASE_VERSION, modelVersion: 3 },
      ),
    );
    expect(hover).toMatchObject({ ok: true, method: 'hover' });
    if (!hover.ok) throw new Error('hover failed');
    expect(JSON.stringify(hover.result)).toContain('显示');

    const symbols = await dispatch(
      runtime,
      messages,
      createRequest(
        'symbols',
        'symbols',
        { text: VALID_SOURCE, resultLimit: 100 },
        { ...BASE_VERSION, modelVersion: 3 },
      ),
    );
    expect(symbols).toMatchObject({ ok: true, method: 'symbols' });
    if (!symbols.ok) throw new Error('symbols failed');
    expect(symbols.result.map((symbol) => symbol.name)).toContain('bookSourceName');

    const unformatted =
      '{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"Example"}';
    const format = await dispatch(
      runtime,
      messages,
      createRequest(
        'format',
        'format',
        { text: unformatted, options: { tabSize: 2, insertSpaces: true } },
        { ...BASE_VERSION, modelVersion: 4 },
      ),
    );
    expect(format).toMatchObject({ ok: true, method: 'format' });
    if (!format.ok) throw new Error('format failed');
    expect(format.result.length).toBeGreaterThan(0);

    const folding = await dispatch(
      runtime,
      messages,
      createRequest(
        'folding',
        'folding',
        { text: VALID_SOURCE, rangeLimit: 100 },
        { ...BASE_VERSION, modelVersion: 5 },
      ),
    );
    expect(folding).toMatchObject({ ok: true, method: 'folding' });
    if (!folding.ok) throw new Error('folding failed');
    expect(folding.result.length).toBeGreaterThan(0);

    const selectionRanges = await dispatch(
      runtime,
      messages,
      createRequest(
        'selection-ranges',
        'selectionRanges',
        {
          text: VALID_SOURCE,
          positions: [richDocument.positionAt(VALID_SOURCE.indexOf('Example'))],
        },
        { ...BASE_VERSION, modelVersion: 5 },
      ),
    );
    expect(selectionRanges).toMatchObject({ ok: true, method: 'selectionRanges' });
    if (!selectionRanges.ok) throw new Error('selection ranges failed');
    expect(selectionRanges.result).toHaveLength(1);

    expect(fetchTrap).not.toHaveBeenCalled();
    expect(webSocketTrap).not.toHaveBeenCalled();
    runtime.dispose();
  });

  it('keeps complex rule values opaque and never emits a credential marker', async () => {
    const messages: SourceLanguageWorkerOutput[] = [];
    const runtime = createSourceLanguageWorkerRuntime((message) => messages.push(message));
    const text = `{
      "bookSourceType": 0,
      "bookSourceUrl": "https://example.test",
      "bookSourceName": "Masked",
      "header": "${CREDENTIAL_MARKER}",
      "ruleSearch": { "opaqueTemplate": "{{$.items}}@js:result" }
    }`;
    const document = TextDocument.create('test://masked.json', 'json', 1, text);
    const opaqueValidation = await dispatch(
      runtime,
      messages,
      createRequest('masked-validation', 'validation', { text }),
    );
    expect(opaqueValidation).toMatchObject({ ok: true, method: 'validation' });
    if (!opaqueValidation.ok) throw new Error('opaque validation failed');
    expect(
      opaqueValidation.result.filter((diagnostic) =>
        diagnostic.path.startsWith('/ruleSearch/opaqueTemplate'),
      ),
    ).toEqual([]);
    expect(JSON.stringify(opaqueValidation)).not.toContain(CREDENTIAL_MARKER);

    const requests = [
      createRequest('masked-completion', 'completion', {
        text,
        position: document.positionAt(text.indexOf(CREDENTIAL_MARKER) + CREDENTIAL_MARKER.length),
      }),
      createRequest('masked-hover', 'hover', {
        text,
        position: document.positionAt(text.indexOf(CREDENTIAL_MARKER) + 5),
      }),
      createRequest('masked-symbols', 'symbols', { text }),
      createRequest('masked-format', 'format', {
        text,
        options: { tabSize: 2, insertSpaces: true },
      }),
      createRequest('masked-folding', 'folding', { text }),
      createRequest('masked-selection', 'selectionRanges', {
        text,
        positions: [document.positionAt(text.indexOf('opaqueTemplate') + 2)],
      }),
    ] as const;

    for (const request of requests) {
      const response = await dispatch(runtime, messages, request);
      expect(response.ok).toBe(true);
      expect(JSON.stringify(response)).not.toContain(CREDENTIAL_MARKER);
    }

    const unsafeResolve = await dispatch(
      runtime,
      messages,
      createRequest('masked-resolve', 'completionResolve', {
        text,
        item: { label: CREDENTIAL_MARKER, insertText: CREDENTIAL_MARKER },
      }),
    );
    expect(unsafeResolve).toMatchObject({
      ok: false,
      error: { code: 'unsafe_credential_marker' },
    });
    expect(JSON.stringify(unsafeResolve)).not.toContain(CREDENTIAL_MARKER);
    runtime.dispose();
  });

  it('honors protocol version, cancellation, and disposal without late results', async () => {
    const messages: SourceLanguageWorkerOutput[] = [];
    const runtime = createSourceLanguageWorkerRuntime((message) => messages.push(message));

    await runtime.handleMessage({
      ...createRequest('wrong-version', 'validation', { text: VALID_SOURCE }),
      protocolVersion: 2,
    });
    expect(messages.at(-1)).toMatchObject({
      protocolVersion: SOURCE_LANGUAGE_PROTOCOL_VERSION,
      requestId: 'wrong-version',
      ok: false,
      error: { code: 'protocol_mismatch' },
    });

    const currentVersion = { ...BASE_VERSION, modelVersion: 2 };
    await dispatch(
      runtime,
      messages,
      createRequest('current-model', 'validation', { text: VALID_SOURCE }, currentVersion),
    );
    const staleModel = await dispatch(
      runtime,
      messages,
      createRequest('stale-model', 'validation', { text: VALID_SOURCE }, BASE_VERSION),
    );
    expect(staleModel).toMatchObject({ ok: false, error: { code: 'stale_document' } });

    const newerEpoch = { ...BASE_VERSION, documentEpoch: 5, modelVersion: 1 };
    await dispatch(
      runtime,
      messages,
      createRequest('current-epoch', 'validation', { text: VALID_SOURCE }, newerEpoch),
    );
    const staleEpoch = await dispatch(
      runtime,
      messages,
      createRequest(
        'stale-epoch',
        'validation',
        { text: VALID_SOURCE },
        { ...BASE_VERSION, modelVersion: 99 },
      ),
    );
    expect(staleEpoch).toMatchObject({ ok: false, error: { code: 'stale_document' } });

    messages.length = 0;
    const cancelVersion = { ...BASE_VERSION, documentEpoch: 6, modelVersion: 1 };
    const pending = runtime.handleMessage(
      createRequest('cancelled', 'validation', { text: VALID_SOURCE }, cancelVersion),
    );
    await runtime.handleMessage({
      type: 'cancel',
      protocolVersion: SOURCE_LANGUAGE_PROTOCOL_VERSION,
      requestId: 'cancelled',
      ...cancelVersion,
    });
    await pending;
    expect(messages).toEqual([]);
    expect(runtime.activeRequestCount).toBe(0);

    const disposedPending = runtime.handleMessage(
      createRequest(
        'disposed',
        'validation',
        { text: VALID_SOURCE },
        { ...BASE_VERSION, documentEpoch: 7 },
      ),
    );
    runtime.dispose();
    await disposedPending;
    await runtime.handleMessage(
      createRequest(
        'after-dispose',
        'validation',
        { text: VALID_SOURCE },
        { ...BASE_VERSION, documentEpoch: 8 },
      ),
    );
    expect(messages).toEqual([]);
    expect(runtime.activeRequestCount).toBe(0);
  });
});

describe('SourceLanguageServiceClient', () => {
  it('drops stale document versions and ignores their late worker results', async () => {
    const worker = new FakeWorker();
    const client = new SourceLanguageServiceClient({ workerFactory: () => worker });
    const first = client.request({
      ...BASE_VERSION,
      method: 'validation',
      payload: { text: VALID_SOURCE },
    });
    const secondVersion = { ...BASE_VERSION, modelVersion: 2 };
    const second = client.request({
      ...secondVersion,
      method: 'validation',
      payload: { text: VALID_SOURCE },
    });

    await expect(first).rejects.toMatchObject({ code: 'stale_document' });
    expect(worker.posted.map((message) => message.type)).toEqual(['request', 'cancel', 'request']);
    const firstRequest = worker.posted[0];
    const secondRequest = worker.posted[2];
    if (firstRequest.type !== 'request' || secondRequest.type !== 'request') {
      throw new Error('expected request messages');
    }

    worker.emit({
      type: 'result',
      protocolVersion: SOURCE_LANGUAGE_PROTOCOL_VERSION,
      requestId: firstRequest.requestId,
      documentId: firstRequest.documentId,
      documentEpoch: firstRequest.documentEpoch,
      modelVersion: firstRequest.modelVersion,
      method: firstRequest.method,
      ok: true,
      result: [],
    });
    expect(client.pendingRequestCount).toBe(1);

    worker.emit({
      type: 'result',
      protocolVersion: SOURCE_LANGUAGE_PROTOCOL_VERSION,
      requestId: secondRequest.requestId,
      ...secondVersion,
      method: 'validation',
      ok: true,
      result: [],
    });
    await expect(second).resolves.toEqual([]);
    expect(client.pendingRequestCount).toBe(0);
    client.dispose();
  });

  it('posts cancellation once and releases the pending request', async () => {
    const worker = new FakeWorker();
    const client = new SourceLanguageServiceClient({ workerFactory: () => worker });
    const abort = new AbortController();
    const request = client.request(
      {
        ...BASE_VERSION,
        method: 'hover',
        payload: { text: VALID_SOURCE, position: { line: 1, character: 3 } },
      },
      { signal: abort.signal },
    );

    abort.abort();
    await expect(request).rejects.toMatchObject({ code: 'cancelled' });
    expect(worker.posted.map((message) => message.type)).toEqual(['request', 'cancel']);
    expect(client.pendingRequestCount).toBe(0);
    client.dispose();
  });

  it('rejects a credential marker from an unsafe worker result', async () => {
    const worker = new FakeWorker();
    const client = new SourceLanguageServiceClient({ workerFactory: () => worker });
    const request = client.request({
      ...BASE_VERSION,
      method: 'hover',
      payload: { text: VALID_SOURCE, position: { line: 1, character: 3 } },
    });
    const posted = worker.posted[0];
    if (posted?.type !== 'request') throw new Error('expected a request message');

    worker.emit({
      type: 'result',
      protocolVersion: SOURCE_LANGUAGE_PROTOCOL_VERSION,
      requestId: posted.requestId,
      documentId: posted.documentId,
      documentEpoch: posted.documentEpoch,
      modelVersion: posted.modelVersion,
      method: posted.method,
      ok: true,
      result: { contents: [CREDENTIAL_MARKER] },
    });

    await expect(request).rejects.toMatchObject({ code: 'unsafe_credential_marker' });
    expect(client.pendingRequestCount).toBe(0);
    client.dispose();
  });

  it('does not post authoring byte or complexity limit failures', async () => {
    const worker = new FakeWorker();
    const client = new SourceLanguageServiceClient({ workerFactory: () => worker });

    await expect(
      client.request({
        ...BASE_VERSION,
        method: 'validation',
        payload: { text: 'x'.repeat(authoringLimits.max_utf8_bytes + 1) },
      }),
    ).rejects.toMatchObject({ code: 'document_too_large' });

    const tooDeep =
      '{"nested":' +
      '['.repeat(authoringLimits.max_depth + 1) +
      '0' +
      ']'.repeat(authoringLimits.max_depth + 1) +
      '}';
    await expect(
      client.request({
        ...BASE_VERSION,
        modelVersion: 2,
        method: 'validation',
        payload: { text: tooDeep },
      }),
    ).rejects.toMatchObject({ code: 'document_too_complex' });

    expect(worker.posted).toEqual([]);
    client.dispose();
  });

  it('disposes pending work and rejects future requests without recreating a worker', async () => {
    const worker = new FakeWorker();
    const workerFactory = vi.fn(() => worker);
    const client = new SourceLanguageServiceClient({ workerFactory });
    const pending = client.request({
      ...BASE_VERSION,
      method: 'symbols',
      payload: { text: VALID_SOURCE },
    });

    client.dispose();
    await expect(pending).rejects.toMatchObject({ code: 'disposed' });
    await expect(
      client.request({
        ...BASE_VERSION,
        method: 'symbols',
        payload: { text: VALID_SOURCE },
      }),
    ).rejects.toMatchObject({ code: 'disposed' });

    expect(worker.posted.map((message) => message.type)).toEqual(['request', 'dispose']);
    expect(worker.terminated).toBe(true);
    expect(worker.listenerCount).toBe(0);
    expect(workerFactory).toHaveBeenCalledTimes(1);
    expect(client.pendingRequestCount).toBe(0);
  });
});
