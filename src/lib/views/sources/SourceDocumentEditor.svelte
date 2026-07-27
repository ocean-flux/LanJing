<script lang="ts" module>
  import type {
    SourceDocumentEditorSnapshot,
    SourceEditorFallbackReason,
    SourceEditorStatus,
  } from './source-document-editor-host';

  export type SourceDocumentEditorStatus = SourceEditorStatus;
  export type SourceDocumentEditorInputLimit = NonNullable<
    SourceDocumentEditorSnapshot['languageInputLimit']
  >;

  export type SourceDocumentEditorLabels = {
    editor: string;
    status: Record<SourceDocumentEditorStatus, string>;
    fallbackReason: (reason: SourceEditorFallbackReason) => string;
    languageInputLimit: (limit: SourceDocumentEditorInputLimit) => string;
    retry: string;
    retryError: string;
    advancedStateReset: string;
  };
</script>

<script lang="ts">
  import { getPlatformContext } from '$lib/app/platform-context.svelte';
  import type { RuntimePlatform } from '$lib/app/platform-runtime';
  import Notice from '$lib/components/Notice.svelte';
  import { Button } from '$lib/components/ui/button';
  import { cn } from '$lib/utils.js';
  import { onDestroy, untrack } from 'svelte';
  import type { Attachment } from 'svelte/attachments';
  import type { SourceDocumentEditorHost } from './source-document-editor-host';
  import JsonHighlightEditor from './JsonHighlightEditor.svelte';

  type Props = {
    host: SourceDocumentEditorHost;
    platform?: RuntimePlatform;
    labels: SourceDocumentEditorLabels;
    class?: string;
  };

  let { host, platform, labels, class: className = '' }: Props = $props();

  const uid = $props.id();
  const labelId = `${uid}-label`;
  const statusId = `${uid}-status`;

  const platformContext = getPlatformContext();
  const activePlatform = $derived(platform ?? platformContext.platform);

  type HostSnapshot = ReturnType<SourceDocumentEditorHost['getSnapshot']>;
  type HostConnection = {
    readonly host: SourceDocumentEditorHost;
    readonly container: HTMLDivElement;
    readonly unsubscribe: () => void;
    readonly detach: () => void;
    releaseGeneration: number;
    closed: boolean;
  };

  let subscribedSnapshot = $state.raw<HostSnapshot | null>(null);
  let retryFailed = $state(false);
  const snapshot = $derived(subscribedSnapshot ?? host.getSnapshot());
  let fallbackTextarea = $state<HTMLTextAreaElement | null>(null);
  let retryButton = $state<HTMLElement | null>(null);
  let attachedHost = $state.raw<SourceDocumentEditorHost | null>(null);
  let focusHost: SourceDocumentEditorHost | null = null;
  let handledFocusRequestId = -1;
  let hostConnection: HostConnection | null = null;

  function closeHostConnection(connection: HostConnection): void {
    if (connection.closed) return;
    connection.closed = true;
    if (hostConnection === connection) hostConnection = null;
    if (attachedHost === connection.host) attachedHost = null;
    if (focusHost === connection.host) focusHost = null;
    connection.unsubscribe();
    connection.detach();
    subscribedSnapshot = null;
  }

  function scheduleHostConnectionClose(connection: HostConnection): void {
    const generation = ++connection.releaseGeneration;
    queueMicrotask(() => {
      if (connection.releaseGeneration === generation) closeHostConnection(connection);
    });
  }

  const connectHost: Attachment<HTMLDivElement> = (container) => {
    const currentHost = host;
    return untrack(() => {
      const existing = hostConnection;
      if (
        existing &&
        !existing.closed &&
        existing.host === currentHost &&
        existing.container === container
      ) {
        existing.releaseGeneration += 1;
        return () => scheduleHostConnectionClose(existing);
      }
      if (existing) closeHostConnection(existing);

      let active = true;
      const initialSnapshot = currentHost.getSnapshot();
      subscribedSnapshot = initialSnapshot;
      retryFailed = false;
      focusHost = currentHost;
      handledFocusRequestId = initialSnapshot.focusRequestId;
      attachedHost = currentHost;

      const unsubscribeHost = currentHost.subscribe((nextSnapshot) => {
        if (active) subscribedSnapshot = nextSnapshot;
      });
      const detachHost = currentHost.attach(container);
      const connection: HostConnection = {
        host: currentHost,
        container,
        unsubscribe: () => {
          active = false;
          unsubscribeHost();
        },
        detach: detachHost,
        releaseGeneration: 0,
        closed: false,
      };
      hostConnection = connection;
      return () => scheduleHostConnectionClose(connection);
    });
  };

  onDestroy(() => {
    if (hostConnection) closeHostConnection(hostConnection);
  });

  const startHost: Attachment<HTMLElement> = () => {
    const currentHost = attachedHost;
    const currentPlatform = activePlatform;
    if (currentHost) void currentHost.start(currentPlatform);
  };

  const restoreRequestedFocus: Attachment<HTMLElement> = () => {
    const currentHost = host;
    const requestId = snapshot.focusRequestId;
    const target = snapshot.focusTarget;
    const status = snapshot.status;

    if (focusHost !== currentHost) {
      focusHost = currentHost;
      handledFocusRequestId = requestId;
      return;
    }
    if (requestId === handledFocusRequestId) return;

    let cancelled = false;
    queueMicrotask(() => {
      if (cancelled || focusHost !== currentHost) return;

      if (target === 'retry') {
        retryButton?.focus();
      } else if (target === 'fallback' || (target === 'editor' && status === 'fallback')) {
        fallbackTextarea?.focus();
      } else if (target === 'editor' && status === 'ready') {
        currentHost.focus();
      }

      handledFocusRequestId = requestId;
    });

    return () => {
      cancelled = true;
    };
  };

  function editFallbackText(text: string): void {
    host.editText(text);
  }

  let retryGeneration = 0;

  async function retry(): Promise<void> {
    const generation = ++retryGeneration;
    const retryHost = host;
    retryFailed = false;

    try {
      await retryHost.retry();
      if (generation === retryGeneration && retryHost === host) retryFailed = false;
    } catch {
      if (generation === retryGeneration && retryHost === host) retryFailed = true;
    }
  }

  const retryable = $derived(
    snapshot.status === 'fallback' && snapshot.fallbackReason !== 'unsupported-platform',
  );
</script>

<section
  class={cn(
    'glass-panel flex h-full min-h-[20rem] max-w-full min-w-0 flex-1 flex-col overflow-hidden rounded-[var(--radius-panel)] border border-hairline',
    className,
  )}
  aria-labelledby={labelId}
  aria-describedby={statusId}
  data-source-document-editor
  data-status={snapshot.status}
  data-editor-kind={snapshot.editorKind}
  data-focus-target={snapshot.focusTarget}
  data-advanced-state-reset={snapshot.advancedStateReset ? 'true' : 'false'}
  {@attach restoreRequestedFocus}
  {@attach startHost}
>
  <h2 id={labelId} class="sr-only">{labels.editor}</h2>
  <div
    id={statusId}
    class="flex min-h-(--density-control-sm) min-w-0 flex-wrap items-center gap-x-2 gap-y-1 border-b border-hairline px-3 py-1.5 text-xs"
  >
    <span class="font-semibold text-ink" role="status" aria-live="polite">
      {labels.status[snapshot.status]}
    </span>
  </div>

  {#if snapshot.languageInputLimit}
    <div class="px-3 pt-2.5" data-language-input-limit={snapshot.languageInputLimit}>
      <Notice tone="danger" role="alert" icon="warning-circle">
        {labels.languageInputLimit(snapshot.languageInputLimit)}
      </Notice>
    </div>
  {/if}

  {#if retryFailed}
    <p
      class="mx-3 mt-2.5 rounded-[var(--radius-control)] border border-destructive/35 bg-destructive/10 px-3 py-2 text-sm break-words text-destructive"
      role="alert"
      data-operation-error="retry"
    >
      {labels.retryError}
    </p>
  {/if}

  {#if snapshot.advancedStateReset}
    <div class="px-3 pt-2.5" data-advanced-state-reset-notice>
      <Notice tone="info" role="status" icon="warning-circle">
        {labels.advancedStateReset}
      </Notice>
    </div>
  {/if}

  {#if snapshot.fallbackReason}
    <div class="space-y-2 px-3 pt-2.5">
      {#if retryable}
        <Notice tone="danger" role="alert" icon="warning-circle">
          {labels.fallbackReason(snapshot.fallbackReason)}
        </Notice>
        <Button
          bind:ref={retryButton}
          type="button"
          variant="outline"
          class="h-auto min-h-(--density-control-md) max-w-full text-center whitespace-normal"
          onclick={() => void retry()}
        >
          {labels.retry}
        </Button>
      {:else}
        <Notice tone="danger" role="alert" icon="warning-circle">
          {labels.fallbackReason(snapshot.fallbackReason)}
        </Notice>
      {/if}
    </div>
  {/if}

  <div
    class="editor-stage grid min-h-0 min-w-0 flex-1 overflow-hidden bg-surface-2"
    data-editor-stage
  >
    <div
      class={[
        'col-start-1 row-start-1 h-full min-h-0 min-w-0 overflow-hidden bg-surface-2',
        (snapshot.status === 'fallback' || snapshot.status === 'disposed') &&
          'pointer-events-none invisible',
      ]}
      role="group"
      aria-labelledby={labelId}
      aria-label={labels.editor}
      aria-hidden={snapshot.status === 'fallback' || snapshot.status === 'disposed'}
      data-editor-adapter-container
      {@attach connectHost}
    ></div>

    {#if snapshot.status === 'fallback'}
      <div class="col-start-1 row-start-1 h-full min-h-0 min-w-0">
        <JsonHighlightEditor
          bind:ref={fallbackTextarea}
          value={snapshot.text}
          onValueChange={editFallbackText}
          ariaLabel={labels.editor}
          rows={18}
          class="h-full min-h-0! rounded-none! border-0!"
        />
      </div>
    {:else if snapshot.status === 'idle' || snapshot.status === 'loading'}
      <div
        class="pointer-events-none col-start-1 row-start-1 grid h-full min-h-0 place-items-center px-3 text-center text-sm text-ink-muted"
        aria-hidden="true"
      >
        {labels.status[snapshot.status]}
      </div>
    {:else if snapshot.status === 'disposed'}
      <div
        class="col-start-1 row-start-1 grid h-full min-h-0 place-items-center px-3 text-center text-sm text-ink-muted"
        aria-hidden="true"
      >
        {labels.status.disposed}
      </div>
    {/if}
  </div>
</section>

<style>
  .editor-stage :global(.monaco-editor) {
    --vscode-editor-background: var(--surface-2);
    --vscode-editor-foreground: var(--ink);
    --vscode-editorGutter-background: var(--surface-2);
    --vscode-editorLineNumber-foreground: var(--ink-subtle);
    --vscode-editorLineNumber-activeForeground: var(--ink);
    --vscode-editorCursor-foreground: var(--lantern-strong);
    --vscode-editor-selectionBackground: var(--lantern-soft);
    --vscode-editor-inactiveSelectionBackground: var(--lantern-soft);
    --vscode-focusBorder: var(--lantern-strong);
    font-family: var(--font-code) !important;
  }

  .editor-stage :global(.monaco-editor .view-line),
  .editor-stage :global(.monaco-editor textarea) {
    font-family: var(--font-code) !important;
  }

  .editor-stage :global(.cm-editor) {
    height: 100%;
    min-height: 0;
    background: var(--surface-2);
    color: var(--ink);
    font-family: var(--font-code);
  }

  .editor-stage :global(.cm-content),
  .editor-stage :global(.cm-gutters) {
    font-family: var(--font-code);
  }

  .editor-stage :global(.cm-gutters) {
    border-color: var(--hairline);
    background: var(--surface-2);
    color: var(--ink-subtle);
  }

  .editor-stage :global(.cm-cursor) {
    border-left-color: var(--lantern-strong);
  }

  .editor-stage :global(.cm-selectionBackground),
  .editor-stage :global(.cm-content ::selection) {
    background: var(--lantern-soft) !important;
  }

  .editor-stage :global(.json-highlight-editor),
  .editor-stage :global(.json-highlight-editor pre),
  .editor-stage :global(.json-highlight-editor textarea) {
    height: 100%;
    min-height: 100%;
  }

  .editor-stage :global(.json-highlight-editor textarea) {
    resize: none;
  }

  @media (forced-colors: active) {
    .editor-stage {
      border-color: CanvasText;
      background: Canvas;
    }

    .editor-stage :global(.monaco-editor) {
      --vscode-editor-background: Canvas;
      --vscode-editor-foreground: CanvasText;
      --vscode-editorGutter-background: Canvas;
      --vscode-editorLineNumber-foreground: CanvasText;
      --vscode-editorLineNumber-activeForeground: Highlight;
      --vscode-editorCursor-foreground: Highlight;
      --vscode-editor-selectionBackground: Highlight;
      --vscode-editor-inactiveSelectionBackground: Highlight;
      --vscode-focusBorder: Highlight;
      forced-color-adjust: auto;
    }

    .editor-stage :global(.cm-editor),
    .editor-stage :global(.cm-gutters) {
      background: Canvas !important;
      color: CanvasText !important;
      forced-color-adjust: auto;
    }

    .editor-stage :global(.cm-cursor) {
      border-left-color: Highlight !important;
    }

    .editor-stage :global(.cm-selectionBackground),
    .editor-stage :global(.cm-content ::selection) {
      background: Highlight !important;
      color: HighlightText !important;
    }
  }
</style>
