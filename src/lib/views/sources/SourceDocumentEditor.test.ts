import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { setPlatformContext } from '$lib/app/platform-context.svelte';
import { describe, expect, it, vi } from 'vitest';
import type {
  SourceDocumentEditorHost,
  SourceDocumentEditorSnapshot,
} from './source-document-editor-host';
import SourceDocumentEditor, {
  type SourceDocumentEditorLabels,
} from './SourceDocumentEditor.svelte';

type SnapshotListener = (snapshot: SourceDocumentEditorSnapshot) => void;

const labels: SourceDocumentEditorLabels = {
  editor: 'Rule JSON',
  status: {
    idle: 'Idle',
    loading: 'Loading editor',
    ready: 'Editor ready',
    fallback: 'Basic editor',
    disposed: 'Editor closed',
  },
  fallbackReason: (reason) => {
    if (reason === 'runtime-error') return 'Smart editor failed to start.';
    if (reason === 'load-failed') return 'Worker unavailable.';
    if (reason === 'mount-failed') return 'Runtime failure.';
    if (reason === 'unsupported-platform') return 'Basic editor is used on this platform.';
    return reason;
  },
  languageInputLimit: (limit) =>
    limit === 'document_too_large' ? 'Document is too large.' : 'Document is too complex.',
  retry: 'Retry smart editor',
  retryError: 'Could not retry the smart editor.',
  advancedStateReset: 'Advanced editor history was reset.',
};

function createHost(initial: Partial<SourceDocumentEditorSnapshot> = {}) {
  let snapshot: SourceDocumentEditorSnapshot = {
    documentId: 'source-1',
    text: '{"name":"masked"}',
    businessEpoch: 1,
    adapterModelVersion: 1,
    languageGeneration: 0,
    languageInputLimit: null,
    editorKind: 'fallback',
    status: 'idle',
    advancedStateReset: false,
    focusTarget: 'editor',
    focusRequestId: 0,
    ...initial,
  };
  const listeners = new Set<SnapshotListener>();
  const detach = vi.fn();
  const unsubscribe = vi.fn();

  const emit = (patch: Partial<SourceDocumentEditorSnapshot>) => {
    snapshot = { ...snapshot, ...patch };
    for (const listener of listeners) listener(snapshot);
  };

  const implementation = {
    getSnapshot: vi.fn(() => snapshot),
    subscribe: vi.fn((listener: SnapshotListener) => {
      listeners.add(listener);
      listener(snapshot);
      return () => {
        listeners.delete(listener);
        unsubscribe();
      };
    }),
    attach: vi.fn(() => detach),
    start: vi.fn(() => undefined),
    retry: vi.fn(async () => undefined),
    editText: vi.fn((text: string) => {
      emit({ text, adapterModelVersion: snapshot.adapterModelVersion + 1 });
    }),
    focus: vi.fn(() => undefined),
    dispose: vi.fn(() => undefined),
  };

  return {
    host: implementation as unknown as SourceDocumentEditorHost,
    implementation,
    emit,
    detach,
    unsubscribe,
  };
}

const ContextualSourceDocumentEditor: typeof SourceDocumentEditor = (...args) => {
  setPlatformContext({ platform: 'android' });
  return SourceDocumentEditor(...args);
};

describe('SourceDocumentEditor', () => {
  it('prefers the explicit platform override and exposes runtime status only', async () => {
    const fake = createHost();
    const view = render(ContextualSourceDocumentEditor, {
      props: { host: fake.host, platform: 'windows', labels },
    });

    await waitFor(() => {
      expect(fake.implementation.attach).toHaveBeenCalledTimes(1);
      expect(fake.implementation.start).toHaveBeenCalledWith('windows');
    });
    expect(screen.getAllByText('Idle').length).toBeGreaterThan(0);

    fake.emit({ status: 'loading' });
    expect((await screen.findAllByText('Loading editor')).length).toBeGreaterThan(0);

    fake.emit({ status: 'ready', editorKind: 'monaco' });
    await waitFor(() => {
      expect(screen.getByText('Editor ready')).toBeTruthy();
      expect(screen.getByRole('group', { name: 'Rule JSON' }).getAttribute('aria-hidden')).not.toBe(
        'true',
      );
    });
    expect(screen.queryByRole('textbox', { name: 'Rule JSON' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Save' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Prepare' })).toBeNull();

    await view.rerender({ host: fake.host, platform: 'ios', labels });
    await waitFor(() => {
      expect(fake.implementation.start).toHaveBeenLastCalledWith('ios');
    });
    expect(fake.implementation.attach).toHaveBeenCalledTimes(1);
    expect(fake.implementation.dispose).not.toHaveBeenCalled();
  });

  it('uses the context platform and falls back to unknown without a provider', async () => {
    const contextual = createHost();
    const contextualView = render(ContextualSourceDocumentEditor, {
      props: { host: contextual.host, labels },
    });

    await waitFor(() => {
      expect(contextual.implementation.start).toHaveBeenCalledWith('android');
    });
    contextualView.unmount();
    expect(contextual.implementation.dispose).not.toHaveBeenCalled();

    const contextless = createHost();
    render(SourceDocumentEditor, {
      props: { host: contextless.host, labels },
    });

    await waitFor(() => {
      expect(contextless.implementation.start).toHaveBeenCalledWith('unknown');
    });
  });

  it('keeps fallback masked text in the host without recreating business controls', async () => {
    const maskedText =
      '{"header":"__LANJING_CREDENTIAL_SLOT_V1__:11111111-1111-4111-8111-111111111111"}';
    const editedText =
      '{"header":"__LANJING_CREDENTIAL_SLOT_V1__:22222222-2222-4222-8222-222222222222"}';
    const fake = createHost({
      status: 'fallback',
      editorKind: 'fallback',
      text: maskedText,
      businessEpoch: 4,
      fallbackReason: 'unsupported-platform',
      advancedStateReset: false,
      focusTarget: 'fallback',
    });
    render(SourceDocumentEditor, {
      props: { host: fake.host, platform: 'unknown', labels },
    });

    const textarea = await screen.findByRole('textbox', { name: 'Rule JSON' });
    expect((textarea as HTMLTextAreaElement).value).toBe(maskedText);
    expect(screen.getByRole('alert').textContent).toContain(
      'Basic editor is used on this platform.',
    );
    expect(screen.queryByRole('button', { name: 'Retry smart editor' })).toBeNull();
    expect(screen.queryByText('Unsaved')).toBeNull();
    expect(screen.queryByText(/Revision/)).toBeNull();

    await fireEvent.compositionStart(textarea);
    await fireEvent.input(textarea, { target: { value: editedText } });
    await fireEvent.compositionEnd(textarea);
    expect(fake.implementation.editText).toHaveBeenCalledWith(editedText);
    await waitFor(() => {
      expect(
        (screen.getByRole('textbox', { name: 'Rule JSON' }) as HTMLTextAreaElement).value,
      ).toBe(editedText);
    });
  });

  it('renders a localized typed input limit while retaining fallback text', async () => {
    const fake = createHost({
      status: 'fallback',
      editorKind: 'fallback',
      text: '{"oversized":true}',
      languageInputLimit: 'document_too_large',
      focusTarget: 'fallback',
    });
    render(SourceDocumentEditor, {
      props: { host: fake.host, platform: 'unknown', labels },
    });

    const textarea = await screen.findByRole('textbox', { name: 'Rule JSON' });
    expect((textarea as HTMLTextAreaElement).value).toBe('{"oversized":true}');
    expect(
      screen.getAllByRole('alert').some((alert) => alert.textContent?.includes('too large')),
    ).toBe(true);
    expect(screen.queryByRole('button', { name: 'Save' })).toBeNull();
  });

  it('reports retry failure without an unhandled rejection and clears it on success', async () => {
    const fake = createHost({
      status: 'fallback',
      editorKind: 'fallback',
      fallbackReason: 'load-failed',
      focusTarget: 'retry',
    });
    fake.implementation.retry.mockRejectedValueOnce(new Error()).mockResolvedValueOnce(undefined);
    const unhandledRejection = vi.fn((event: PromiseRejectionEvent) => event.preventDefault());
    window.addEventListener('unhandledrejection', unhandledRejection);

    try {
      render(SourceDocumentEditor, {
        props: { host: fake.host, platform: 'android', labels },
      });
      const button = await screen.findByRole('button', { name: 'Retry smart editor' });

      await fireEvent.click(button);
      const alert = await screen.findByText(labels.retryError);
      expect(alert.getAttribute('role')).toBe('alert');
      expect(alert.getAttribute('data-operation-error')).toBe('retry');
      expect(fake.implementation.retry).toHaveBeenCalledTimes(1);
      expect(unhandledRejection).not.toHaveBeenCalled();

      await fireEvent.click(button);
      await waitFor(() => {
        expect(screen.queryByText(labels.retryError)).toBeNull();
      });
      expect(fake.implementation.retry).toHaveBeenCalledTimes(2);
      expect(unhandledRejection).not.toHaveBeenCalled();
    } finally {
      window.removeEventListener('unhandledrejection', unhandledRejection);
    }
  });

  it('makes retry and the explicit advanced-history reset observable', async () => {
    const fake = createHost({
      status: 'fallback',
      editorKind: 'fallback',
      fallbackReason: 'load-failed',
      advancedStateReset: true,
      focusTarget: 'retry',
    });
    render(SourceDocumentEditor, {
      props: { host: fake.host, platform: 'android', labels },
    });

    await fireEvent.click(await screen.findByRole('button', { name: 'Retry smart editor' }));
    expect(fake.implementation.retry).toHaveBeenCalledTimes(1);

    fake.emit({ status: 'loading', fallbackReason: undefined });
    expect((await screen.findAllByText('Loading editor')).length).toBeGreaterThan(0);

    fake.emit({
      status: 'ready',
      editorKind: 'codemirror',
      advancedStateReset: true,
      focusTarget: 'editor',
    });
    await waitFor(() => {
      expect(screen.getByText('Editor ready')).toBeTruthy();
      expect(screen.queryByRole('textbox', { name: 'Rule JSON' })).toBeNull();
    });
    expect(screen.getByText('Advanced editor history was reset.')).toBeTruthy();
  });

  it('restores fallback, retry, and adapter focus only for new focus requests', async () => {
    const fake = createHost({
      status: 'fallback',
      editorKind: 'fallback',
      fallbackReason: 'mount-failed',
      focusTarget: 'fallback',
    });
    render(SourceDocumentEditor, {
      props: { host: fake.host, platform: 'unknown', labels },
    });

    const textarea = await screen.findByRole('textbox', { name: 'Rule JSON' });
    fake.emit({ focusTarget: 'fallback', focusRequestId: 1 });
    await waitFor(() => expect(document.activeElement).toBe(textarea));

    const retry = screen.getByRole('button', { name: 'Retry smart editor' });
    fake.emit({ focusTarget: 'retry', focusRequestId: 2 });
    await waitFor(() => expect(document.activeElement).toBe(retry));

    fake.emit({ status: 'ready', editorKind: 'monaco', focusTarget: 'editor', focusRequestId: 3 });
    await waitFor(() => expect(fake.implementation.focus).toHaveBeenCalledTimes(1));

    fake.emit({ adapterModelVersion: 9 });
    await Promise.resolve();
    expect(fake.implementation.focus).toHaveBeenCalledTimes(1);
  });

  it('replaces and unmounts external hosts with listener and attachment cleanup only', async () => {
    const first = createHost({ status: 'ready' });
    const second = createHost({ status: 'idle' });
    const view = render(SourceDocumentEditor, {
      props: { host: first.host, platform: 'linux', labels },
    });

    await waitFor(() => expect(first.implementation.attach).toHaveBeenCalledTimes(1));
    await view.rerender({ host: second.host, platform: 'linux', labels });

    await waitFor(() => {
      expect(first.unsubscribe).toHaveBeenCalledTimes(1);
      expect(first.detach).toHaveBeenCalledTimes(1);
      expect(first.implementation.dispose).not.toHaveBeenCalled();
      expect(second.implementation.attach).toHaveBeenCalledTimes(1);
      expect(second.implementation.start).toHaveBeenCalledWith('linux');
    });

    view.unmount();
    expect(second.unsubscribe).toHaveBeenCalledTimes(1);
    expect(second.detach).toHaveBeenCalledTimes(1);
    expect(second.implementation.dispose).not.toHaveBeenCalled();
  });
});
