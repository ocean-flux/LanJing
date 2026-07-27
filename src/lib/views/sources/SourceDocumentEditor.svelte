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
  import { Button } from '$lib/components/ui/button';
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
  class={[
    'glass-panel flex max-w-full min-w-0 flex-col gap-3 overflow-hidden rounded-xl border border-hairline p-3 sm:p-4',
    className,
  ]}
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

  <div id={statusId} class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-xs">
    <span class="font-semibold text-ink" role="status" aria-live="polite">
      {labels.status[snapshot.status]}
    </span>
  </div>

  {#if snapshot.languageInputLimit}
    <p
      class="border-danger/35 bg-danger/10 text-danger rounded-lg border px-3 py-2 text-sm break-words"
      role="alert"
      data-language-input-limit={snapshot.languageInputLimit}
    >
      {labels.languageInputLimit(snapshot.languageInputLimit)}
    </p>
  {/if}

  {#if retryFailed}
    <p
      class="border-danger/35 bg-danger/10 text-danger rounded-lg border px-3 py-2 text-sm break-words"
      role="alert"
      data-operation-error="retry"
    >
      {labels.retryError}
    </p>
  {/if}

  {#if snapshot.advancedStateReset}
    <p
      class="rounded-lg border border-lantern/35 bg-lantern-soft/25 px-3 py-2 text-sm text-ink"
      role="status"
      data-advanced-state-reset-notice
    >
      {labels.advancedStateReset}
    </p>
  {/if}

  {#if snapshot.fallbackReason}
    <div
      class="border-danger/35 bg-danger/10 flex min-w-0 flex-wrap items-start justify-between gap-2 rounded-lg border px-3 py-2"
    >
      <p class="text-danger min-w-0 flex-1 text-sm break-words" role="alert">
        {labels.fallbackReason(snapshot.fallbackReason)}
      </p>
      {#if retryable}
        <Button
          bind:ref={retryButton}
          type="button"
          variant="outline"
          class="min-h-11 max-w-full text-center whitespace-normal active:scale-[0.98]"
          onclick={() => void retry()}
        >
          {labels.retry}
        </Button>
      {/if}
    </div>
  {/if}

  <div class="grid min-h-[20rem] min-w-0 overflow-hidden rounded-xl" data-editor-stage>
    <div
      class={[
        'col-start-1 row-start-1 min-h-[20rem] min-w-0 overflow-hidden rounded-xl border border-hairline bg-surface-2',
        (snapshot.status === 'fallback' || snapshot.status === 'disposed') &&
          'pointer-events-none invisible',
      ]}
      role="group"
      aria-labelledby={labelId}
      aria-hidden={snapshot.status === 'fallback' || snapshot.status === 'disposed'}
      data-editor-adapter-container
      {@attach connectHost}
    ></div>

    {#if snapshot.status === 'fallback'}
      <div class="col-start-1 row-start-1 min-h-0 min-w-0">
        <JsonHighlightEditor
          bind:ref={fallbackTextarea}
          value={snapshot.text}
          onValueChange={editFallbackText}
          ariaLabel={labels.editor}
          rows={18}
          class="glass-control h-full min-h-[20rem] rounded-xl border-hairline focus-within:border-lantern-strong/50"
        />
      </div>
    {:else if snapshot.status === 'idle' || snapshot.status === 'loading'}
      <div
        class="pointer-events-none col-start-1 row-start-1 grid min-h-[20rem] place-items-center px-4 text-center text-sm text-ink-muted"
        aria-hidden="true"
      >
        {labels.status[snapshot.status]}
      </div>
    {:else if snapshot.status === 'disposed'}
      <div
        class="col-start-1 row-start-1 grid min-h-[20rem] place-items-center px-4 text-center text-sm text-ink-muted"
        aria-hidden="true"
      >
        {labels.status.disposed}
      </div>
    {/if}
  </div>
</section>
