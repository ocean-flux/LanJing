<script lang="ts">
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { MediaQuery } from 'svelte/reactivity';
  import Icon from '$lib/components/Icon.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Sheet, SheetContent, SheetHeader, SheetTitle } from '$lib/components/ui/sheet';
  import * as Tabs from '$lib/components/ui/tabs';
  import { Input } from '$lib/components/ui/input';
  import { Label } from '$lib/components/ui/label';
  import { Textarea } from '$lib/components/ui/textarea';
  import {
    registerLeaveGuard,
    requestSurfaceClose,
    type LeaveFocusTarget,
  } from '$lib/app/leave-coordinator.svelte';
  import { getPlatformContext } from '$lib/app/platform-context.svelte';
  import { IsMobile } from '$lib/hooks/is-mobile.svelte';
  import { m } from '$lib/i18n';
  import { SourceAuthoringDocument, type SourceAuthoringPatch } from '$lib/rules/authoring';
  import { RuleEditorSession } from '$lib/stores/rule-editor-session.svelte';
  import SourceDocumentEditor, {
    type SourceDocumentEditorLabels,
  } from '../SourceDocumentEditor.svelte';
  import RuleConflictPanel from './RuleConflictPanel.svelte';
  import RuleDocumentList from './RuleDocumentList.svelte';
  import RuleFieldEditor from './RuleFieldEditor.svelte';
  import RuleInspector from './RuleInspector.svelte';
  import RuleTreeEditor from './RuleTreeEditor.svelte';

  type Props = {
    session?: RuleEditorSession;
  };

  let { session = new RuleEditorSession() }: Props = $props();
  const snapshot = $derived(session.snapshot);
  let documentsOpen = $state(false);
  let inspectorOpen = $state(false);
  let showDismissedConflict = $state(false);
  let legacyTitle = $state('');
  let legacyText = $state('');
  let legacyCreateFailed = $state(false);

  const isMobile = new IsMobile();
  const platformContext = getPlatformContext();
  const nativeMobile = $derived(
    platformContext.platform === 'ios' || platformContext.platform === 'android',
  );
  const inspectorCollapsed = new MediaQuery('max-width: 1199px');
  const initialDocumentId = page.url.searchParams.get('document_id');
  const legacySourceId = page.url.searchParams.get('legacy_source_id');

  const selectedDocumentId = $derived(snapshot.document?.summary.document_id ?? null);
  const legadoDocuments = $derived(
    snapshot.documents.filter((document) => document.format === 'legado'),
  );
  const hasDocument = $derived(snapshot.document !== null);
  const authoringText = $derived(snapshot.text);
  const authoringEpoch = $derived(snapshot.document_epoch);
  const authoringView = $derived(
    hasDocument ? SourceAuthoringDocument.open(authoringText, authoringEpoch) : null,
  );
  const editorHost = $derived(session.editorHost);
  const authoringBlocked = $derived(
    snapshot.diagnostics.some(
      (diagnostic) => diagnostic.severity === 'error' || diagnostic.support === 'blocked',
    ),
  );
  const documentBusy = $derived(
    snapshot.documents_loading || snapshot.rebase_pending || snapshot.install_pending,
  );
  const canSave = $derived(
    hasDocument &&
      snapshot.dirty &&
      !authoringBlocked &&
      !snapshot.locked &&
      snapshot.conflict === null &&
      !snapshot.disposed,
  );
  const canPrepare = $derived(
    hasDocument &&
      !snapshot.dirty &&
      !authoringBlocked &&
      snapshot.conflict === null &&
      !snapshot.prepare_pending &&
      !snapshot.locked &&
      !snapshot.disposed,
  );
  const conflictVisible = $derived(
    snapshot.conflict !== null && (!snapshot.conflict.dismissed || showDismissedConflict),
  );
  const activeEditorSurface = $derived(
    snapshot.document !== null &&
      editorHost !== null &&
      authoringView !== null &&
      !snapshot.locked &&
      !(snapshot.conflict && conflictVisible)
      ? snapshot.mode
      : undefined,
  );
  const legacyCanCreate = $derived(
    legacyTitle.trim().length > 0 && legacyText.trim().length > 0 && !documentBusy,
  );

  const editorLabels: SourceDocumentEditorLabels = {
    editor: m.sources_rules_editor_label(),
    status: {
      idle: m.sources_rules_editor_status_idle(),
      loading: m.sources_rules_editor_status_loading(),
      ready: m.sources_rules_editor_status_ready(),
      fallback: m.sources_rules_editor_status_fallback(),
      disposed: m.sources_rules_editor_status_disposed(),
    },
    fallbackReason: (reason) => {
      if (reason === 'unsupported-platform') return m.sources_rules_editor_fallback_unsupported();
      if (reason === 'load-failed') return m.sources_rules_editor_fallback_load();
      if (reason === 'mount-failed') return m.sources_rules_editor_fallback_mount();
      return m.sources_rules_editor_fallback_runtime();
    },
    languageInputLimit: (limit) =>
      limit === 'document_too_large'
        ? m.sources_rules_editor_too_large()
        : m.sources_rules_editor_too_complex(),
    retry: m.sources_rules_editor_retry(),
    retryError: m.sources_rules_toolbar_action_error(),
    advancedStateReset: m.sources_rules_editor_advanced_reset(),
  };

  onMount(() => {
    const unregister = registerLeaveGuard({
      canLeave: () => session.canLeave(),
      resolveLeave: (action) => session.resolveLeave(action),
      focusEditor: () => session.focusEditor(),
    });

    void initialize();

    return () => {
      unregister();
      void session.dispose();
    };
  });

  async function initialize(): Promise<void> {
    try {
      const documents = await session.refreshDocuments();
      if (initialDocumentId) {
        await session.openDocument(initialDocumentId);
      } else if (!legacySourceId) {
        const firstLegadoDocument = documents.find((document) => document.format === 'legado');
        if (firstLegadoDocument) await session.openDocument(firstLegadoDocument.document_id);
      }
    } catch {
      // typed 且不含凭证的操作错误只由 session snapshot 持有。
    }
  }

  function setMode(value: unknown): void {
    if (value === 'original' || value === 'form' || value === 'tree') {
      session.setMode(value);
    }
  }

  function applyPatch(patch: SourceAuthoringPatch): void {
    session.applyPatch(patch);
  }

  function requestCreateDocument(replay: () => void, focusTarget: LeaveFocusTarget): void {
    requestSurfaceClose(replay, focusTarget);
  }

  function requestOpenDocument(documentId: string, focusTarget: LeaveFocusTarget): void {
    if (documentId === selectedDocumentId) {
      documentsOpen = false;
      session.focusEditor();
      return;
    }
    const replay = async () => {
      const result = await session.openDocument(documentId);
      if (result.status !== 'opened') throw new Error('rule_document_open_rejected');
      documentsOpen = false;
      session.focusEditor();
    };
    requestSurfaceClose(replay, focusTarget);
  }

  async function createDocument(title: string, text: string): Promise<void> {
    const outcome = await session.createDocument(title, text);
    if (outcome.status !== 'saved' || !outcome.document) {
      throw new Error('rule_document_create_rejected');
    }
    documentsOpen = false;
  }

  async function renameDocument(title: string): Promise<void> {
    const outcome = await session.renameCurrentDocument(title);
    if (outcome.status !== 'saved') throw new Error('rule_document_rename_rejected');
  }

  async function deleteDocument(): Promise<void> {
    const outcome = await session.deleteCurrentDocument();
    if (outcome.status !== 'saved') throw new Error('rule_document_delete_rejected');
  }

  function closeWorkspace(event: MouseEvent): void {
    requestSurfaceClose(() => goto(resolve('/sources')), event.currentTarget as HTMLButtonElement);
  }

  function save(): void {
    void session.save().catch(() => undefined);
  }

  function prepare(): void {
    void session.prepare().catch(() => undefined);
  }

  function shortcutTarget(event: KeyboardEvent): Element | null {
    for (const entry of event.composedPath()) {
      if (entry instanceof Element) return entry;
    }
    return null;
  }

  function isEditableShortcutTarget(target: Element): boolean {
    if (
      target.closest(
        'input, textarea, select, [role="combobox"], [role="listbox"], [role="option"]',
      )
    ) {
      return true;
    }

    let current: Element | null = target;
    while (current) {
      if (current.hasAttribute('contenteditable')) {
        return current.getAttribute('contenteditable') !== 'false';
      }
      current = current.parentElement;
    }
    return false;
  }

  function isModalShortcutTarget(target: Element): boolean {
    return target.closest('[role="dialog"], [role="alertdialog"], [aria-modal="true"]') !== null;
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.defaultPrevented || event.isComposing || (!event.ctrlKey && !event.metaKey)) return;
    const target = shortcutTarget(event);
    if (!target || isModalShortcutTarget(target) || isEditableShortcutTarget(target)) return;

    const key = event.key.toLowerCase();
    if (key === 's') {
      event.preventDefault();
      if (canSave) save();
      return;
    }
    if ((key !== 'z' && key !== 'y') || !target.closest('[data-rule-editor-surface]')) return;

    event.preventDefault();
    if (key === 'y' || event.shiftKey) session.redo();
    else session.undo();
  }

  async function forkConflict(title: string): Promise<boolean> {
    const outcome = await session.forkConflict(title);
    return outcome.status === 'saved';
  }

  async function mergeConflict(): Promise<boolean> {
    const outcome = await session.mergeConflict();
    return outcome.status === 'saved';
  }

  function continueConflict(): void {
    showDismissedConflict = false;
    session.continueEditingConflict();
  }

  async function createLegacyDocument(): Promise<void> {
    if (!legacyCanCreate) return;
    legacyCreateFailed = false;
    try {
      const outcome = await session.createDocument(legacyTitle.trim(), legacyText);
      if (outcome.status !== 'saved' || !outcome.document) {
        legacyCreateFailed = true;
        return;
      }
      legacyText = '';
    } catch {
      legacyCreateFailed = true;
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<section class="mx-auto flex w-full max-w-[100rem] flex-col gap-4" data-rule-workspace>
  <PageHeader
    title={m.sources_rules_page_title()}
    description={m.sources_rules_page_description()}
  />

  <div class="flex flex-wrap items-center justify-between gap-2">
    <Button href={resolve('/sources')} variant="ghost" class="min-h-11">
      <Icon name="arrow-left" class="size-4" />
      <span>{m.sources_rules_back_to_sources()}</span>
    </Button>
    <Button type="button" variant="outline" class="min-h-11" onclick={closeWorkspace}>
      <Icon name="x" class="size-4" />
      <span>{m.sources_rules_surface_close()}</span>
    </Button>
  </div>

  <div
    class="glass-panel grid min-h-[40rem] min-w-0 overflow-hidden rounded-xl border border-hairline min-[1200px]:grid-cols-[18rem_minmax(0,1fr)_20rem] md:h-[calc(100dvh-12rem)] md:grid-cols-[16rem_minmax(0,1fr)]"
  >
    {#if !isMobile.current}
      <div class="hidden min-h-0 min-w-0 border-r border-hairline md:block">
        <RuleDocumentList
          documents={legadoDocuments}
          {selectedDocumentId}
          loading={snapshot.documents_loading}
          loadError={snapshot.operation_error === 'document_load_failed' &&
            legadoDocuments.length === 0}
          busy={documentBusy}
          onRetry={async () => {
            await session.refreshDocuments();
          }}
          onRequestCreate={requestCreateDocument}
          onOpen={requestOpenDocument}
          onCreate={createDocument}
          onRename={renameDocument}
          onDelete={deleteDocument}
        />
      </div>
    {/if}

    <section
      class="flex min-h-0 min-w-0 flex-col overflow-hidden bg-surface-1/50"
      aria-label={m.sources_rules_page_title()}
    >
      <div
        class="flex min-w-0 flex-nowrap items-center gap-2 border-b border-hairline p-2 sm:flex-wrap sm:p-3"
      >
        <Button
          type="button"
          variant="outline"
          class="min-h-11 min-w-11 px-0 sm:px-2.5 md:hidden"
          aria-label={m.sources_rules_documents_sheet_open()}
          onclick={() => (documentsOpen = true)}
        >
          <Icon name="list-bullets" class="size-4" />
          <span class="hidden sm:inline">{m.sources_rules_documents_sheet_open()}</span>
        </Button>

        {#if hasDocument}
          <Tabs.Root class="min-w-0 flex-1" bind:value={() => snapshot.mode, setMode}>
            <Tabs.List class="min-h-11 w-full sm:w-auto" aria-label={m.sources_rules_mode_label()}>
              <Tabs.Trigger value="original" class="min-h-11">
                <Icon name="code" class="hidden size-4 sm:inline-block" />
                <span>{m.sources_rules_mode_original()}</span>
              </Tabs.Trigger>
              <Tabs.Trigger value="form" class="min-h-11" disabled={authoringBlocked}>
                <Icon name="list-bullets" class="hidden size-4 sm:inline-block" />
                <span>{m.sources_rules_mode_form()}</span>
              </Tabs.Trigger>
              <Tabs.Trigger value="tree" class="min-h-11" disabled={authoringBlocked}>
                <Icon name="tree-structure" class="hidden size-4 sm:inline-block" />
                <span>{m.sources_rules_mode_tree()}</span>
              </Tabs.Trigger>
            </Tabs.List>
          </Tabs.Root>
        {/if}

        {#if inspectorCollapsed.current && hasDocument}
          <Button
            type="button"
            variant="outline"
            size="icon"
            class="min-h-11 min-w-11"
            aria-label={m.sources_rules_inspector_sheet_open()}
            onclick={() => (inspectorOpen = true)}
          >
            <Icon name="shield-check" class="size-4" />
          </Button>
        {/if}
      </div>

      {#if hasDocument}
        <div
          class="flex min-w-0 flex-wrap items-center gap-2 border-b border-hairline px-2 py-2 sm:px-3"
        >
          <div class="mr-auto min-w-0">
            <p class="truncate text-sm font-semibold text-ink">
              {snapshot.document?.summary.title}
            </p>
            <p class="text-xs text-ink-muted">
              {m.sources_rules_documents_revision({ revision: snapshot.saved_revision })}
              · {snapshot.dirty ? m.sources_rules_toolbar_dirty() : m.sources_rules_toolbar_saved()}
            </p>
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            class="min-h-11 min-w-11"
            aria-label={m.sources_rules_toolbar_undo()}
            disabled={!snapshot.history.can_undo}
            onclick={() => session.undo()}
          >
            <Icon name="arrow-counter-clockwise" class="size-4" />
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            class="min-h-11 min-w-11"
            aria-label={m.sources_rules_toolbar_redo()}
            disabled={!snapshot.history.can_redo}
            onclick={() => session.redo()}
          >
            <Icon name="arrow-clockwise" class="size-4" />
          </Button>
          <Button
            type="button"
            variant="outline"
            class="min-h-11"
            aria-label={m.sources_rules_toolbar_format()}
            disabled={snapshot.locked || snapshot.conflict !== null}
            onclick={() => session.format()}
          >
            <Icon name="brackets-curly" class="size-4" />
            <span class="hidden sm:inline">{m.sources_rules_toolbar_format()}</span>
          </Button>
          <Button type="button" class="min-h-11" disabled={!canSave} onclick={save}>
            <Icon name="floppy-disk" class="size-4" />
            <span>
              {snapshot.save_phase === 'idle'
                ? m.sources_rules_toolbar_save()
                : m.sources_rules_toolbar_saving()}
            </span>
          </Button>
          <Button
            type="button"
            variant="outline"
            class="min-h-11"
            disabled={!canPrepare}
            onclick={prepare}
          >
            <Icon name="shield-check" class="size-4" />
            <span>
              {snapshot.prepare_pending
                ? m.sources_rules_toolbar_preparing()
                : m.sources_rules_toolbar_prepare()}
            </span>
          </Button>
        </div>
      {/if}

      {#if snapshot.dirty && nativeMobile}
        <p
          class="m-3 flex items-start gap-2 rounded-lg border border-lantern/35 bg-lantern-soft/25 px-3 py-2 text-sm leading-6 text-ink"
          data-native-mobile-save-risk={platformContext.platform}
        >
          <Icon name="warning-circle" class="mt-1 size-4 shrink-0 text-lantern-strong" />
          <span>{m.sources_rules_mobile_kill_warning()}</span>
        </p>
      {/if}

      {#if snapshot.operation_error && snapshot.operation_error !== 'install_failed'}
        <p
          class="border-danger/35 bg-danger/10 text-danger m-3 rounded-lg border px-3 py-2 text-sm"
          role="alert"
          data-operation-error={snapshot.operation_error}
        >
          {snapshot.operation_error === 'save_failed'
            ? m.sources_rules_toolbar_save_error()
            : m.sources_rules_toolbar_action_error()}
        </p>
      {/if}

      {#if snapshot.locked}
        <p
          class="border-danger/35 bg-danger/10 text-danger m-3 flex items-start gap-2 rounded-lg border px-3 py-2 text-sm"
          role="alert"
        >
          <Icon name="lock" class="mt-0.5 size-4 shrink-0" />
          <span>{m.sources_rules_locked()}</span>
        </p>
      {/if}

      <div
        class="min-h-0 min-w-0 flex-1 overflow-hidden"
        data-rule-editor-surface={activeEditorSurface}
      >
        {#if snapshot.conflict && conflictVisible}
          <div class="h-full overflow-y-auto p-3 sm:p-4">
            <RuleConflictPanel
              conflict={snapshot.conflict}
              localMaskedText={snapshot.text}
              busy={snapshot.rebase_pending}
              onTextEdit={(text) => session.editText(text)}
              onSetResolution={(path, action) =>
                session.setConflictCredentialResolution(path, action)}
              onReload={async () => {
                await session.reloadConflict();
              }}
              onFork={forkConflict}
              onMerge={mergeConflict}
              onContinue={continueConflict}
            />
          </div>
        {:else if snapshot.locked}
          <div class="grid h-full min-h-64 place-items-center px-5 text-center" role="status">
            <div>
              <Icon name="lock" class="text-danger mx-auto size-7" />
              <p class="mt-3 max-w-md text-sm leading-6 text-ink-muted">
                {m.sources_rules_locked()}
              </p>
            </div>
          </div>
        {:else if snapshot.document && editorHost && authoringView}
          {#if snapshot.conflict?.dismissed}
            <div
              class="flex flex-wrap items-center justify-between gap-2 border-b border-lantern/35 bg-lantern-soft/20 px-3 py-2"
            >
              <p class="text-sm text-ink">{m.sources_rules_conflict_title()}</p>
              <Button
                type="button"
                variant="outline"
                class="min-h-11"
                onclick={() => (showDismissedConflict = true)}
              >
                <Icon name="git-merge" class="size-4" />
                <span>{m.sources_rules_conflict_manual()}</span>
              </Button>
            </div>
          {/if}
          {#key snapshot.document.summary.document_id}
            {#if snapshot.mode === 'original'}
              <div class="h-full min-h-0 overflow-y-auto p-2 sm:p-3">
                <SourceDocumentEditor host={editorHost} labels={editorLabels} class="min-h-full" />
              </div>
            {:else if snapshot.mode === 'form'}
              <RuleFieldEditor
                document={authoringView}
                disabled={snapshot.locked || snapshot.conflict !== null}
                onPatch={applyPatch}
              />
            {:else}
              <RuleTreeEditor
                document={authoringView}
                disabled={snapshot.locked || snapshot.conflict !== null}
                onPatch={applyPatch}
              />
            {/if}
          {/key}
        {:else if legacySourceId}
          <div class="h-full overflow-y-auto p-4 sm:p-6">
            <section class="mx-auto max-w-2xl space-y-5">
              <div class="rounded-xl border border-lantern/35 bg-lantern-soft/20 p-4">
                <Icon name="file-text" class="size-6 text-lantern-strong" />
                <h2 class="mt-3 font-semibold text-ink">{m.sources_rules_legacy_title()}</h2>
                <p class="mt-1 text-sm leading-6 text-ink-muted">
                  {m.sources_rules_legacy_description()}
                </p>
                <code class="mt-2 block font-mono text-xs break-all text-ink-subtle">
                  {m.sources_rules_legacy_source({ id: legacySourceId })}
                </code>
              </div>
              <div class="space-y-4">
                <div>
                  <h3 class="font-semibold text-ink">{m.sources_rules_legacy_paste_title()}</h3>
                  <p class="mt-1 text-sm text-ink-muted">
                    {m.sources_rules_legacy_paste_description()}
                  </p>
                </div>
                <div class="space-y-1.5">
                  <Label for="legacy-rule-title">{m.sources_rules_documents_title_label()}</Label>
                  <Input
                    id="legacy-rule-title"
                    class="min-h-11"
                    placeholder={m.sources_rules_documents_title_placeholder()}
                    bind:value={legacyTitle}
                  />
                </div>
                <div class="space-y-1.5">
                  <Label for="legacy-rule-text">{m.sources_rules_documents_text_label()}</Label>
                  <Textarea
                    id="legacy-rule-text"
                    class="min-h-72 resize-y font-mono text-sm"
                    spellcheck="false"
                    placeholder={m.sources_rules_documents_text_placeholder()}
                    bind:value={legacyText}
                  />
                </div>
                {#if legacyCreateFailed}
                  <p class="text-danger text-sm" role="alert">
                    {m.sources_rules_toolbar_action_error()}
                  </p>
                {/if}
                <Button
                  type="button"
                  class="min-h-11 w-full sm:w-auto"
                  disabled={!legacyCanCreate}
                  onclick={createLegacyDocument}
                >
                  <Icon name="file-plus" class="size-4" />
                  <span>{m.sources_rules_documents_create()}</span>
                </Button>
              </div>
            </section>
          </div>
        {:else}
          <div
            class="grid h-full min-h-64 place-items-center px-5 text-center text-sm text-ink-muted"
            role="status"
          >
            {m.sources_rules_no_document()}
          </div>
        {/if}
      </div>
    </section>

    {#if !inspectorCollapsed.current}
      <div class="hidden min-h-0 min-w-0 border-l border-hairline min-[1200px]:block">
        <RuleInspector {session} {snapshot} />
      </div>
    {/if}
  </div>
</section>

<Sheet bind:open={() => isMobile.current && documentsOpen, (open) => (documentsOpen = open)}>
  <SheetContent
    side="bottom"
    class="h-[88dvh] max-h-[88dvh] gap-0 overflow-hidden p-0 pb-[env(safe-area-inset-bottom)] md:hidden"
  >
    <SheetHeader class="border-b border-hairline pr-12">
      <SheetTitle>{m.sources_rules_documents_title()}</SheetTitle>
    </SheetHeader>
    <div class="min-h-0 flex-1 overflow-hidden">
      <RuleDocumentList
        documents={legadoDocuments}
        {selectedDocumentId}
        loading={snapshot.documents_loading}
        loadError={snapshot.operation_error === 'document_load_failed' &&
          legadoDocuments.length === 0}
        busy={documentBusy}
        onRetry={async () => {
          await session.refreshDocuments();
        }}
        onRequestCreate={requestCreateDocument}
        onOpen={requestOpenDocument}
        onCreate={createDocument}
        onRename={renameDocument}
        onDelete={deleteDocument}
      />
    </div>
  </SheetContent>
</Sheet>

<Sheet
  bind:open={() => inspectorCollapsed.current && inspectorOpen, (open) => (inspectorOpen = open)}
>
  <SheetContent
    side={isMobile.current ? 'bottom' : 'right'}
    class="h-[88dvh] max-h-[88dvh] gap-0 overflow-hidden p-0 pb-[env(safe-area-inset-bottom)] data-[side=right]:h-full data-[side=right]:max-h-dvh data-[side=right]:sm:max-w-md"
  >
    <SheetHeader class="border-b border-hairline pr-12">
      <SheetTitle>{m.sources_rules_inspector_title()}</SheetTitle>
    </SheetHeader>
    <div class="min-h-0 flex-1 overflow-hidden">
      <RuleInspector {session} {snapshot} />
    </div>
  </SheetContent>
</Sheet>
