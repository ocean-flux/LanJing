<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
  } from '$lib/components/ui/dialog';
  import { Input } from '$lib/components/ui/input';
  import { Label } from '$lib/components/ui/label';
  import { Textarea } from '$lib/components/ui/textarea';
  import { m } from '$lib/i18n';
  import type { SourceDocumentSummary } from '../rule-editor-api';

  type Props = {
    documents: readonly SourceDocumentSummary[];
    selectedDocumentId: string | null;
    loading?: boolean;
    loadError?: boolean;
    busy?: boolean;
    onRetry: () => void | Promise<void>;
    onRequestCreate: (
      replay: () => void,
      focusTarget: Pick<HTMLElement, 'focus' | 'isConnected'>,
    ) => void;
    onOpen: (documentId: string, focusTarget: Pick<HTMLElement, 'focus' | 'isConnected'>) => void;
    onCreate: (title: string, text: string) => void | Promise<void>;
    onRename: (title: string) => void | Promise<void>;
    onDelete: () => void | Promise<void>;
  };

  let {
    documents,
    selectedDocumentId,
    loading = false,
    loadError = false,
    busy = false,
    onRetry,
    onRequestCreate,
    onOpen,
    onCreate,
    onRename,
    onDelete,
  }: Props = $props();

  let createOpen = $state(false);
  let renameOpen = $state(false);
  let deleteOpen = $state(false);
  let createTitle = $state('');
  let createText = $state('');
  let renameTitle = $state('');
  let operationFailed = $state(false);
  let createTitleInput = $state<HTMLInputElement | null>(null);

  const createFocusTarget: Pick<HTMLElement, 'focus' | 'isConnected'> = {
    get isConnected() {
      return createTitleInput?.isConnected ?? false;
    },
    focus() {
      createTitleInput?.focus();
    },
  };

  const selectedDocument = $derived(
    documents.find((document) => document.document_id === selectedDocumentId) ?? null,
  );
  const canCreate = $derived(
    !busy && createTitle.trim().length > 0 && createText.trim().length > 0,
  );
  const canRename = $derived(!busy && selectedDocument !== null && renameTitle.trim().length > 0);

  function resetCreate(): void {
    createTitle = '';
    createText = '';
    operationFailed = false;
  }

  function openCreateDialog(): void {
    resetCreate();
    createOpen = true;
  }

  function requestCreate(): void {
    onRequestCreate(openCreateDialog, createFocusTarget);
  }

  function closeCreateDialog(): void {
    createOpen = false;
    resetCreate();
  }

  function setCreateOpen(open: boolean): void {
    createOpen = open;
    if (!open) resetCreate();
  }

  function openRenameDialog(): void {
    if (!selectedDocument) return;
    renameTitle = selectedDocument.title;
    operationFailed = false;
    renameOpen = true;
  }

  function openDeleteDialog(): void {
    if (!selectedDocument) return;
    operationFailed = false;
    deleteOpen = true;
  }

  async function createDocument(): Promise<void> {
    if (!canCreate) return;
    operationFailed = false;
    try {
      await onCreate(createTitle.trim(), createText);
      closeCreateDialog();
    } catch {
      operationFailed = true;
    }
  }

  async function renameDocument(): Promise<void> {
    if (!canRename) return;
    operationFailed = false;
    try {
      await onRename(renameTitle.trim());
      renameOpen = false;
      renameTitle = '';
    } catch {
      operationFailed = true;
    }
  }

  async function deleteDocument(): Promise<void> {
    if (!selectedDocument || busy) return;
    operationFailed = false;
    try {
      await onDelete();
      deleteOpen = false;
    } catch {
      operationFailed = true;
    }
  }
</script>

<section class="flex min-h-0 min-w-0 flex-col" aria-labelledby="rule-documents-title">
  <header class="flex items-start justify-between gap-3 border-b border-hairline px-3 py-3">
    <div class="min-w-0">
      <h2 id="rule-documents-title" class="text-sm font-semibold text-ink">
        {m.sources_rules_documents_title()}
      </h2>
      <p class="mt-1 text-xs leading-5 text-ink-muted">
        {m.sources_rules_documents_description()}
      </p>
    </div>
    <Button
      type="button"
      variant="outline"
      size="icon"
      class="min-h-11 min-w-11"
      aria-label={m.sources_rules_documents_new()}
      disabled={busy}
      onclick={requestCreate}
    >
      <Icon name="file-plus" class="size-4" />
    </Button>
  </header>

  {#if loading}
    <div class="flex min-h-40 items-center gap-2 px-4 text-sm text-ink-muted" role="status">
      <Icon name="arrow-clockwise" class="size-4" />
      <span>{m.sources_rules_documents_loading()}</span>
    </div>
  {:else if loadError}
    <div class="border-danger/35 bg-danger/10 m-3 rounded-xl border p-3" role="alert">
      <p class="text-danger text-sm">{m.sources_rules_documents_error()}</p>
      <Button type="button" variant="outline" class="mt-3 min-h-11" onclick={() => void onRetry()}>
        <Icon name="arrow-clockwise" class="size-4" />
        <span>{m.sources_rules_documents_retry()}</span>
      </Button>
    </div>
  {:else if documents.length === 0}
    <div class="grid min-h-52 place-items-center px-4 py-6 text-center" role="status">
      <div>
        <Icon name="file-text" class="mx-auto size-7 text-ink-muted" />
        <p class="mt-3 text-sm font-semibold text-ink">{m.sources_rules_documents_empty()}</p>
        <p class="mt-1 text-xs leading-5 text-ink-muted">
          {m.sources_rules_documents_empty_hint()}
        </p>
        <Button type="button" class="mt-4 min-h-11" disabled={busy} onclick={requestCreate}>
          <Icon name="file-plus" class="size-4" />
          <span>{m.sources_rules_documents_new()}</span>
        </Button>
      </div>
    </div>
  {:else}
    <ul class="min-h-0 flex-1 divide-y divide-hairline overflow-y-auto">
      {#each documents as document (document.document_id)}
        <li>
          <button
            type="button"
            class={[
              'flex min-h-14 w-full min-w-0 items-start gap-3 px-3 py-3 text-left transition-colors outline-none hover:bg-surface-2 focus-visible:shadow-[inset_var(--focus-ring)]',
              selectedDocumentId === document.document_id && 'bg-lantern-soft/30',
            ]}
            aria-label={m.sources_rules_documents_open({ title: document.title })}
            aria-current={selectedDocumentId === document.document_id ? 'true' : undefined}
            disabled={busy}
            onclick={(event) =>
              onOpen(document.document_id, event.currentTarget as HTMLButtonElement)}
          >
            <Icon
              name={document.state === 'linked' ? 'shield-check' : 'file-text'}
              class="mt-0.5 size-5 shrink-0 text-ink-muted"
            />
            <span class="min-w-0 flex-1">
              <span class="block text-sm font-semibold break-words text-ink">{document.title}</span>
              <span class="mt-1 flex flex-wrap items-center gap-1.5">
                <Badge variant={document.state === 'linked' ? 'secondary' : 'outline'}>
                  {document.state === 'linked'
                    ? m.sources_rules_documents_linked()
                    : m.sources_rules_documents_draft()}
                </Badge>
                <span class="text-[0.6875rem] text-ink-subtle">
                  {m.sources_rules_documents_revision({ revision: document.revision })}
                </span>
                {#if document.credential_slot_count > 0}
                  <span class="text-[0.6875rem] text-ink-subtle">
                    {m.sources_rules_documents_credentials({
                      count: document.credential_slot_count,
                    })}
                  </span>
                {/if}
              </span>
            </span>
            {#if selectedDocumentId === document.document_id}
              <span class="sr-only">{m.sources_rules_documents_selected()}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if selectedDocument}
    <footer class="grid grid-cols-2 gap-2 border-t border-hairline p-3">
      <Button
        type="button"
        variant="outline"
        class="min-h-11"
        disabled={busy}
        onclick={openRenameDialog}
      >
        <Icon name="pencil-simple" class="size-4" />
        <span>{m.sources_rules_documents_rename()}</span>
      </Button>
      <Button
        type="button"
        variant="destructive"
        class="min-h-11"
        disabled={busy}
        onclick={openDeleteDialog}
      >
        <Icon name="trash" class="size-4" />
        <span>{m.sources_rules_documents_delete()}</span>
      </Button>
    </footer>
  {/if}
</section>

<Dialog bind:open={() => createOpen, setCreateOpen}>
  <DialogContent class="sm:max-w-2xl">
    <DialogHeader>
      <DialogTitle>{m.sources_rules_documents_new_title()}</DialogTitle>
      <DialogDescription>{m.sources_rules_documents_new_description()}</DialogDescription>
    </DialogHeader>
    <div class="space-y-4 py-2">
      <div class="space-y-1.5">
        <Label for="rule-document-title">{m.sources_rules_documents_title_label()}</Label>
        <Input
          bind:ref={createTitleInput}
          id="rule-document-title"
          class="min-h-11"
          placeholder={m.sources_rules_documents_title_placeholder()}
          bind:value={createTitle}
        />
      </div>
      <div class="space-y-1.5">
        <Label for="rule-document-text">{m.sources_rules_documents_text_label()}</Label>
        <Textarea
          id="rule-document-text"
          class="min-h-64 resize-y font-mono text-sm"
          placeholder={m.sources_rules_documents_text_placeholder()}
          spellcheck="false"
          bind:value={createText}
        />
      </div>
      {#if operationFailed}
        <p
          class="border-danger/35 bg-danger/10 text-danger rounded-lg border px-3 py-2 text-sm"
          role="alert"
        >
          {m.sources_rules_toolbar_action_error()}
        </p>
      {/if}
    </div>
    <DialogFooter class="gap-2 sm:gap-2">
      <Button type="button" variant="outline" class="min-h-11" onclick={closeCreateDialog}>
        {m.sources_rules_action_cancel()}
      </Button>
      <Button type="button" class="min-h-11" disabled={!canCreate} onclick={createDocument}>
        {busy ? m.sources_rules_documents_creating() : m.sources_rules_documents_create()}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>

<Dialog bind:open={renameOpen}>
  <DialogContent class="sm:max-w-md">
    <DialogHeader>
      <DialogTitle>{m.sources_rules_documents_rename_title()}</DialogTitle>
      <DialogDescription>{m.sources_rules_documents_rename_description()}</DialogDescription>
    </DialogHeader>
    <div class="space-y-1.5 py-2">
      <Label for="rule-document-rename">{m.sources_rules_documents_title_label()}</Label>
      <Input id="rule-document-rename" class="min-h-11" bind:value={renameTitle} />
      {#if operationFailed}
        <p class="text-danger text-sm" role="alert">{m.sources_rules_toolbar_action_error()}</p>
      {/if}
    </div>
    <DialogFooter class="gap-2 sm:gap-2">
      <Button type="button" variant="outline" class="min-h-11" onclick={() => (renameOpen = false)}>
        {m.sources_rules_action_cancel()}
      </Button>
      <Button type="button" class="min-h-11" disabled={!canRename} onclick={renameDocument}>
        {m.sources_rules_documents_rename_save()}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>

<Dialog bind:open={deleteOpen}>
  <DialogContent class="sm:max-w-md">
    <DialogHeader>
      <DialogTitle>{m.sources_rules_documents_delete_title()}</DialogTitle>
      <DialogDescription>
        {m.sources_rules_documents_delete_description({ title: selectedDocument?.title ?? '' })}
      </DialogDescription>
    </DialogHeader>
    {#if operationFailed}
      <p class="text-danger text-sm" role="alert">{m.sources_rules_toolbar_action_error()}</p>
    {/if}
    <DialogFooter class="gap-2 sm:gap-2">
      <Button type="button" variant="outline" class="min-h-11" onclick={() => (deleteOpen = false)}>
        {m.sources_rules_action_cancel()}
      </Button>
      <Button
        type="button"
        variant="destructive"
        class="min-h-11"
        disabled={busy}
        onclick={deleteDocument}
      >
        {m.sources_rules_documents_delete_confirm()}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
