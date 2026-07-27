<script lang="ts">
  import { SvelteSet } from 'svelte/reactivity';
  import Icon from '$lib/components/Icon.svelte';
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
  import * as Select from '$lib/components/ui/select';
  import { Textarea } from '$lib/components/ui/textarea';
  import { m } from '$lib/i18n';
  import type {
    RuleEditorConflictCredentialSnapshot,
    RuleEditorConflictSnapshot,
  } from '$lib/stores/rule-editor-session.svelte';
  import type { SourceDocumentCredentialResolutionAction } from '../rule-editor-api';

  type Props = {
    conflict: RuleEditorConflictSnapshot;
    localMaskedText: string;
    busy?: boolean;
    onTextEdit: (text: string) => void;
    onSetResolution: (path: string, action: SourceDocumentCredentialResolutionAction) => void;
    onReload: () => void | Promise<void>;
    onFork: (title: string) => Promise<boolean>;
    onMerge: () => Promise<boolean>;
    onContinue: () => void;
  };

  type ResolutionKind = SourceDocumentCredentialResolutionAction['kind'];
  type ResolutionOption = {
    value: ResolutionKind;
    label: string;
    disabled?: boolean;
  };
  type DiffRow = {
    line: number;
    local: string;
    current: string;
    changed: boolean;
  };

  type FailedOperation = 'reload' | 'fork' | 'merge' | null;

  let {
    conflict,
    localMaskedText,
    busy = false,
    onTextEdit,
    onSetResolution,
    onReload,
    onFork,
    onMerge,
    onContinue,
  }: Props = $props();

  let forkOpen = $state(false);
  let manualOpen = $state(false);
  let forkTitle = $state('');
  let failedOperation = $state<FailedOperation>(null);
  const replaceReadyPaths = new SvelteSet<string>();

  const resolutionOptions = $derived([
    { value: 'keep_current', label: m.sources_rules_conflict_keep_current() },
    { value: 'keep_local', label: m.sources_rules_conflict_keep_local() },
    { value: 'replace', label: m.sources_rules_conflict_replace() },
    { value: 'clear', label: m.sources_rules_conflict_clear() },
  ] satisfies ResolutionOption[]);

  const credentialResolutionOptions = $derived(
    conflict.credentials.map((credential) => ({
      path: credential.path,
      options: resolutionOptions.map((option) => ({
        ...option,
        disabled: !resolutionAvailable(credential, option.value),
      })),
    })),
  );

  const diffRows = $derived.by(() => {
    const localLines = localMaskedText.split('\n');
    const currentLines = conflict.current.masked_text.split('\n');
    const count = Math.max(localLines.length, currentLines.length);
    return Array.from({ length: count }, (_, index): DiffRow => ({
      line: index + 1,
      local: localLines[index] ?? '',
      current: currentLines[index] ?? '',
      changed: localLines[index] !== currentLines[index],
    }));
  });

  const resolutionsReady = $derived(
    conflict.credentials.every(
      (credential) =>
        credential.resolution !== null &&
        resolutionAvailable(credential, credential.resolution) &&
        (credential.resolution !== 'replace' || replaceReadyPaths.has(credential.path)),
    ),
  );
  const canFork = $derived(!busy && resolutionsReady && forkTitle.trim().length > 0);
  const canMerge = $derived(!busy && resolutionsReady);

  function resolutionAvailable(
    credential: RuleEditorConflictCredentialSnapshot,
    resolution: ResolutionKind,
  ): boolean {
    if (resolution === 'keep_current') return credential.current_present;
    if (resolution === 'keep_local') return credential.base_present;
    return true;
  }

  function optionsForCredential(path: string): ResolutionOption[] {
    return (
      credentialResolutionOptions.find((entry) => entry.path === path)?.options ?? resolutionOptions
    );
  }

  function conflictTitle(): string {
    if (conflict.kind === 'sentinel_rotation') return m.sources_rules_conflict_rotation_title();
    if (conflict.kind === 'snapshot_unavailable') {
      return m.sources_rules_conflict_snapshot_unavailable_title();
    }
    return m.sources_rules_conflict_title();
  }

  function conflictDescription(): string {
    if (conflict.kind === 'sentinel_rotation') {
      return m.sources_rules_conflict_rotation_description();
    }
    if (conflict.kind === 'snapshot_unavailable') {
      return m.sources_rules_conflict_snapshot_unavailable_description();
    }
    return m.sources_rules_conflict_description();
  }

  function reloadLabel(): string {
    if (conflict.kind === 'sentinel_rotation') return m.sources_rules_conflict_rotation_reload();
    if (conflict.kind === 'snapshot_unavailable') {
      return m.sources_rules_conflict_snapshot_unavailable_reload();
    }
    return m.sources_rules_conflict_reload();
  }

  function selectedResolutionLabel(credential: RuleEditorConflictCredentialSnapshot): string {
    return (
      resolutionOptions.find((option) => option.value === credential.resolution)?.label ??
      m.sources_rules_conflict_credentials_required()
    );
  }

  function setResolution(credential: RuleEditorConflictCredentialSnapshot, value: unknown): void {
    if (
      value !== 'keep_current' &&
      value !== 'keep_local' &&
      value !== 'replace' &&
      value !== 'clear'
    ) {
      return;
    }
    if (!resolutionAvailable(credential, value)) return;
    replaceReadyPaths.delete(credential.path);
    const action: SourceDocumentCredentialResolutionAction =
      value === 'replace' ? { kind: 'replace', value: '' } : { kind: value };
    onSetResolution(credential.path, action);
  }

  function setReplacement(path: string, event: Event): void {
    const input = event.currentTarget as HTMLInputElement;
    const value = input.value;
    if (value.length > 0) replaceReadyPaths.add(path);
    else replaceReadyPaths.delete(path);
    onSetResolution(path, { kind: 'replace', value });
  }

  function editManualText(event: Event): void {
    replaceReadyPaths.clear();
    onTextEdit((event.currentTarget as HTMLTextAreaElement).value);
  }

  async function reload(): Promise<void> {
    failedOperation = null;
    try {
      await onReload();
    } catch {
      failedOperation = 'reload';
    }
  }

  async function fork(): Promise<void> {
    if (!canFork) return;
    failedOperation = null;
    try {
      const saved = await onFork(forkTitle.trim());
      if (!saved) {
        failedOperation = 'fork';
        return;
      }
      forkOpen = false;
      forkTitle = '';
    } catch {
      failedOperation = 'fork';
    }
  }

  function openForkDialog(): void {
    failedOperation = null;
    forkOpen = true;
  }

  function openManualMerge(): void {
    failedOperation = null;
    for (const credential of conflict.credentials) {
      if (credential.resolution !== 'replace') continue;
      replaceReadyPaths.delete(credential.path);
      onSetResolution(credential.path, { kind: 'replace', value: '' });
    }
    manualOpen = true;
  }

  async function merge(): Promise<void> {
    if (!canMerge) return;
    failedOperation = null;
    try {
      const saved = await onMerge();
      if (!saved) {
        failedOperation = 'merge';
        return;
      }
      manualOpen = false;
    } catch {
      failedOperation = 'merge';
    }
  }
</script>

{#snippet resolutionFields()}
  <section class="space-y-3" aria-labelledby="conflict-credential-resolutions-title">
    <div>
      <h3 id="conflict-credential-resolutions-title" class="text-sm font-semibold text-ink">
        {m.sources_rules_conflict_credentials_title()}
      </h3>
      {#if conflict.credentials.length > 0}
        <p class="mt-1 text-xs text-ink-muted">
          {m.sources_rules_conflict_credentials_required()}
        </p>
      {/if}
    </div>

    {#each conflict.credentials as credential (credential.path)}
      {@const options = optionsForCredential(credential.path)}
      <div class="space-y-2 rounded-lg border border-hairline bg-surface-2 p-3">
        <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
          <code class="min-w-0 font-mono text-xs break-all text-ink">{credential.path}</code>
          <span class="text-[0.6875rem] text-ink-subtle">
            {credential.base_present ? m.sources_rules_conflict_keep_local() : '—'} /
            {credential.current_present ? m.sources_rules_conflict_keep_current() : '—'}
          </span>
        </div>
        <Select.Root
          type="single"
          items={options}
          bind:value={
            () => credential.resolution ?? undefined, (value) => setResolution(credential, value)
          }
        >
          <Select.Trigger
            class="min-h-11 w-full"
            aria-label={m.sources_rules_conflict_credential_action({ path: credential.path })}
          >
            <span>{selectedResolutionLabel(credential)}</span>
          </Select.Trigger>
          <Select.Content>
            <Select.Group>
              <Select.GroupHeading>
                {m.sources_rules_conflict_credential_action({ path: credential.path })}
              </Select.GroupHeading>
              {#each options as option (option.value)}
                <Select.Item value={option.value} label={option.label} disabled={option.disabled}>
                  {option.label}
                </Select.Item>
              {/each}
            </Select.Group>
          </Select.Content>
        </Select.Root>

        {#if credential.resolution === 'replace'}
          <label class="block">
            <span class="sr-only">
              {m.sources_rules_conflict_replace_value({ path: credential.path })}
            </span>
            <Input
              type="password"
              class="min-h-11"
              autocomplete="new-password"
              data-conflict-replace
              placeholder={m.sources_rules_conflict_replace_value({ path: credential.path })}
              onchange={(event) => setReplacement(credential.path, event)}
            />
          </label>
        {/if}
      </div>
    {/each}
  </section>
{/snippet}

<section class="space-y-4" aria-labelledby="rule-conflict-title" data-rule-conflict>
  <header class="rounded-xl border border-lantern/40 bg-lantern-soft/25 p-4">
    <div class="flex items-start gap-3">
      <Icon
        name={conflict.reload_required ? 'lock' : 'git-merge'}
        class="mt-0.5 size-5 shrink-0 text-lantern-strong"
      />
      <div class="min-w-0">
        <h2 id="rule-conflict-title" class="font-semibold text-ink">
          {conflictTitle()}
        </h2>
        <p class="mt-1 text-sm leading-6 text-ink-muted">
          {conflictDescription()}
        </p>
      </div>
    </div>
  </header>

  <div class="overflow-hidden rounded-xl border border-hairline bg-surface-1">
    <div class="grid grid-cols-2 border-b border-hairline text-xs font-semibold text-ink-muted">
      <p class="border-r border-hairline px-3 py-2">{m.sources_rules_conflict_local()}</p>
      <p class="px-3 py-2">{m.sources_rules_conflict_current()}</p>
    </div>
    <div
      class="max-h-72 overflow-auto font-mono text-xs"
      role="region"
      aria-label={conflictTitle()}
    >
      {#each diffRows as row (row.line)}
        <div class={['grid min-w-[42rem] grid-cols-2', row.changed && 'bg-lantern-soft/20']}>
          <pre class="m-0 min-h-6 border-r border-hairline px-2 py-1 whitespace-pre-wrap"><span
              class="mr-2 text-ink-subtle">{row.line}</span
            >{row.local}</pre>
          <pre class="m-0 min-h-6 px-2 py-1 whitespace-pre-wrap"><span class="mr-2 text-ink-subtle"
              >{row.line}</span
            >{row.current}</pre>
        </div>
      {/each}
    </div>
  </div>

  {#if !conflict.reload_required && !manualOpen}
    {@render resolutionFields()}
  {/if}

  {#if failedOperation === 'reload'}
    <p
      class="border-danger/35 bg-danger/10 text-danger rounded-lg border px-3 py-2 text-sm"
      role="alert"
    >
      {m.sources_rules_conflict_error()}
    </p>
  {/if}

  {#if conflict.reload_required}
    <Button type="button" class="min-h-11 w-full" disabled={busy} onclick={() => void reload()}>
      <Icon name="arrow-counter-clockwise" class="size-4" />
      <span>{reloadLabel()}</span>
    </Button>
  {:else}
    <div class="grid gap-2 sm:grid-cols-2">
      <Button
        type="button"
        variant="outline"
        class="min-h-11"
        disabled={busy}
        onclick={() => void reload()}
      >
        <Icon name="arrow-counter-clockwise" class="size-4" />
        <span>{m.sources_rules_conflict_reload()}</span>
      </Button>
      <Button
        type="button"
        variant="outline"
        class="min-h-11"
        disabled={busy}
        onclick={openForkDialog}
      >
        <Icon name="file-plus" class="size-4" />
        <span>{m.sources_rules_conflict_fork()}</span>
      </Button>
      <Button
        type="button"
        variant="outline"
        class="min-h-11"
        disabled={busy}
        onclick={openManualMerge}
      >
        <Icon name="git-merge" class="size-4" />
        <span>{m.sources_rules_conflict_manual()}</span>
      </Button>
      <Button type="button" variant="ghost" class="min-h-11" disabled={busy} onclick={onContinue}>
        <Icon name="pencil-simple" class="size-4" />
        <span>{m.sources_rules_conflict_continue()}</span>
      </Button>
    </div>
  {/if}
</section>

<Dialog bind:open={forkOpen}>
  <DialogContent class="sm:max-w-md">
    <DialogHeader>
      <DialogTitle>{m.sources_rules_conflict_fork_title()}</DialogTitle>
      <DialogDescription>{m.sources_rules_conflict_fork_description()}</DialogDescription>
    </DialogHeader>
    <div class="space-y-1.5 py-2">
      <Label for="conflict-fork-title">{m.sources_rules_conflict_fork_name()}</Label>
      <Input
        id="conflict-fork-title"
        class="min-h-11"
        aria-label={m.sources_rules_conflict_fork_name()}
        bind:value={forkTitle}
      />
    </div>
    {#if failedOperation === 'fork'}
      <p
        class="border-danger/35 bg-danger/10 text-danger rounded-lg border px-3 py-2 text-sm"
        role="alert"
      >
        {m.sources_rules_conflict_error()}
      </p>
    {/if}
    <DialogFooter class="gap-2 sm:gap-2">
      <Button type="button" variant="outline" class="min-h-11" onclick={() => (forkOpen = false)}>
        {m.sources_rules_action_cancel()}
      </Button>
      <Button type="button" class="min-h-11" disabled={!canFork} onclick={fork}>
        {m.sources_rules_conflict_fork_confirm()}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>

<Dialog bind:open={manualOpen}>
  <DialogContent class="max-h-[90dvh] overflow-y-auto sm:max-w-4xl">
    <DialogHeader>
      <DialogTitle>{m.sources_rules_conflict_manual_title()}</DialogTitle>
      <DialogDescription>{m.sources_rules_conflict_manual_description()}</DialogDescription>
    </DialogHeader>
    <div class="space-y-4 py-2">
      <div class="space-y-1.5">
        <Label for="conflict-manual-text">{m.sources_rules_conflict_manual_text()}</Label>
        <Textarea
          id="conflict-manual-text"
          aria-label={m.sources_rules_conflict_manual_text()}
          class="min-h-72 resize-y font-mono text-sm"
          spellcheck="false"
          value={localMaskedText}
          oninput={editManualText}
        />
      </div>
      {@render resolutionFields()}
    </div>
    {#if failedOperation === 'merge'}
      <p
        class="border-danger/35 bg-danger/10 text-danger rounded-lg border px-3 py-2 text-sm"
        role="alert"
      >
        {m.sources_rules_conflict_error()}
      </p>
    {/if}
    <DialogFooter class="gap-2 sm:gap-2">
      <Button type="button" variant="outline" class="min-h-11" onclick={() => (manualOpen = false)}>
        {m.sources_rules_action_cancel()}
      </Button>
      <Button type="button" class="min-h-11" disabled={!canMerge} onclick={merge}>
        {m.sources_rules_conflict_manual_confirm()}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
