<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { Attachment } from 'svelte/attachments';
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
  import { m } from '$lib/i18n';
  import type { CredentialSlotSummary } from '../rule-editor-api';

  type Props = {
    slots: readonly CredentialSlotSummary[];
    revealedSlotIds: readonly string[];
    busy?: boolean;
    onReveal: (slotId: string) => Promise<void>;
    onClearRevealed: () => Promise<void>;
    readRevealed: (slotId: string) => string | null;
    onReplace: (slotId: string, value: string) => Promise<void>;
    onClear: (slotId: string) => Promise<void>;
  };

  let {
    slots,
    revealedSlotIds,
    busy = false,
    onReveal,
    onClearRevealed,
    readRevealed,
    onReplace,
    onClear,
  }: Props = $props();

  let panelElement: HTMLElement | null = null;
  let replaceOpen = $state(false);
  let clearOpen = $state(false);
  let activeSlot = $state<CredentialSlotSummary | null>(null);
  let replacementInput = $state<HTMLInputElement | null>(null);
  let dialogOwnerSignature = $state<string | null>(null);
  let actionError = $state(false);
  let copiedSlotId = $state<string | null>(null);

  const capturePanel: Attachment<HTMLElement> = (element) => {
    const boundaryAttribute = 'data-credential-dialog-boundary';
    let dialogBoundaryObserver: MutationObserver | null = null;
    let lastBoundary: string | null = null;
    let disposed = false;
    panelElement = element;

    // 延迟到首轮 DOM 属性稳定后，只把真正的 busy/owner 变化视为失效。
    queueMicrotask(() => {
      if (disposed) return;
      lastBoundary = element.getAttribute(boundaryAttribute);
      dialogBoundaryObserver = new MutationObserver(() => {
        if (disposed) return;
        const currentBoundary = element.getAttribute(boundaryAttribute);
        if (currentBoundary === lastBoundary) return;
        lastBoundary = currentBoundary;
        closeCredentialDialogs();
      });
      dialogBoundaryObserver.observe(element, {
        attributes: true,
        attributeFilter: [boundaryAttribute],
      });
    });

    return () => {
      disposed = true;
      dialogBoundaryObserver?.disconnect();
      for (const input of element.querySelectorAll<HTMLInputElement>(
        '[data-revealed-credential]',
      )) {
        input.value = '';
      }
      if (panelElement === element) panelElement = null;
    };
  };

  const slotOwnerSignature = $derived(
    slots
      .map((slot) => slot.slot_id)
      .sort()
      .join('\u001f'),
  );
  const activeSlotCurrent = $derived(
    activeSlot !== null &&
      dialogOwnerSignature === slotOwnerSignature &&
      slots.some((slot) => slot.slot_id === activeSlot?.slot_id),
  );
  const canReplace = $derived(!busy && activeSlotCurrent);

  onDestroy(() => {
    wipeVisibleCredentialDom();
    closeCredentialDialogs();
  });

  function isRevealed(slotId: string): boolean {
    return revealedSlotIds.includes(slotId);
  }

  function wipeVisibleCredentialDom(): void {
    for (const input of panelElement?.querySelectorAll<HTMLInputElement>(
      '[data-revealed-credential]',
    ) ?? []) {
      input.value = '';
    }
  }

  function revealedValue(slotId: string): Attachment<HTMLInputElement> {
    return (input) => {
      input.value = readRevealed(slotId) ?? '';
      return () => {
        input.value = '';
      };
    };
  }

  async function reveal(slotId: string): Promise<void> {
    actionError = false;
    copiedSlotId = null;
    try {
      await onReveal(slotId);
    } catch {
      actionError = true;
    }
  }

  async function hideRevealed(): Promise<void> {
    wipeVisibleCredentialDom();
    copiedSlotId = null;
    actionError = false;
    try {
      await onClearRevealed();
    } catch {
      actionError = true;
    }
  }

  async function copyRevealed(slotId: string): Promise<void> {
    const value = readRevealed(slotId);
    if (value === null) return;
    actionError = false;
    try {
      await navigator.clipboard.writeText(value);
      copiedSlotId = slotId;
    } catch {
      actionError = true;
    }
  }

  function openReplace(slot: CredentialSlotSummary): void {
    clearOpen = false;
    wipeReplacementInput();
    activeSlot = slot;
    dialogOwnerSignature = slotOwnerSignature;
    actionError = false;
    replaceOpen = true;
  }

  function openClear(slot: CredentialSlotSummary): void {
    replaceOpen = false;
    wipeReplacementInput();
    activeSlot = slot;
    dialogOwnerSignature = slotOwnerSignature;
    actionError = false;
    clearOpen = true;
  }

  function setReplacementInput(input: HTMLInputElement | null): void {
    if (replacementInput && replacementInput !== input) replacementInput.value = '';
    replacementInput = input;
  }

  function wipeReplacementInput(): void {
    if (replacementInput) replacementInput.value = '';
    replacementInput = null;
  }

  function closeCredentialDialogs(): void {
    wipeReplacementInput();
    replaceOpen = false;
    clearOpen = false;
    activeSlot = null;
    dialogOwnerSignature = null;
  }

  async function replaceCredential(): Promise<void> {
    if (!activeSlot || !replacementInput) return;
    const slotId = activeSlot.slot_id;
    const value = replacementInput.value;
    if (!value.trim()) return;
    closeCredentialDialogs();
    actionError = false;
    wipeVisibleCredentialDom();
    try {
      await onClearRevealed();
      await onReplace(slotId, value);
    } catch {
      actionError = true;
    }
  }

  async function clearCredential(): Promise<void> {
    if (!activeSlot) return;
    const slotId = activeSlot.slot_id;
    closeCredentialDialogs();
    actionError = false;
    wipeVisibleCredentialDom();
    try {
      await onClearRevealed();
      await onClear(slotId);
    } catch {
      actionError = true;
    }
  }
</script>

<section
  {@attach capturePanel}
  data-credential-dialog-boundary={`${busy ? 'busy' : 'idle'}\u001e${slotOwnerSignature}`}
  class="flex min-w-0 flex-col"
  aria-labelledby="rule-credentials-title"
>
  <header class="px-4 py-3">
    <h3 id="rule-credentials-title" class="text-sm font-semibold text-ink">
      {m.sources_rules_credentials_title()}
    </h3>
    <p class="mt-1 text-xs leading-5 text-ink-muted">
      {m.sources_rules_credentials_description()}
    </p>
  </header>

  {#if slots.length === 0}
    <p class="px-4 pb-4 text-sm text-ink-muted" role="status">
      {m.sources_rules_credentials_empty()}
    </p>
  {:else}
    <ul class="divide-y divide-hairline border-t border-hairline">
      {#each slots as slot (slot.slot_id)}
        <li class="space-y-3 px-4 py-3" data-credential-slot-id={slot.slot_id}>
          <div class="flex min-w-0 items-start justify-between gap-3">
            <div class="min-w-0">
              <div class="flex flex-wrap items-center gap-1.5">
                <span class="text-sm font-semibold break-words text-ink">{slot.name}</span>
                <Badge variant={slot.has_value ? 'secondary' : 'outline'}>
                  {slot.has_value
                    ? m.sources_rules_credentials_present()
                    : m.sources_rules_credentials_missing()}
                </Badge>
              </div>
              <code class="mt-1 block font-mono text-[0.6875rem] break-all text-ink-subtle">
                {slot.path}
              </code>
            </div>
            <Icon
              name={slot.has_value ? 'lock' : 'file-text'}
              class="size-4 shrink-0 text-ink-muted"
            />
          </div>

          {#if isRevealed(slot.slot_id)}
            <label class="block">
              <span class="sr-only">{slot.name}</span>
              <Input
                type="text"
                readonly
                autocomplete="off"
                class="min-h-11 font-mono"
                data-revealed-credential
                {@attach revealedValue(slot.slot_id)}
              />
            </label>
          {/if}

          <div class="grid grid-cols-2 gap-2">
            {#if slot.has_value && !isRevealed(slot.slot_id)}
              <Button
                type="button"
                variant="outline"
                class="min-h-11"
                disabled={busy}
                onclick={() => void reveal(slot.slot_id)}
              >
                <Icon name="eye" class="size-4" />
                <span>{m.sources_rules_credentials_reveal({ name: slot.name })}</span>
              </Button>
            {:else if isRevealed(slot.slot_id)}
              <Button
                type="button"
                variant="outline"
                class="min-h-11"
                disabled={busy}
                onclick={() => void hideRevealed()}
              >
                <Icon name="lock" class="size-4" />
                <span>{m.sources_rules_credentials_hide({ name: slot.name })}</span>
              </Button>
              <Button
                type="button"
                variant="outline"
                class="min-h-11"
                disabled={busy}
                onclick={() => void copyRevealed(slot.slot_id)}
              >
                <Icon name="copy" class="size-4" />
                <span>
                  {copiedSlotId === slot.slot_id
                    ? m.sources_rules_credentials_copied()
                    : m.sources_rules_credentials_copy({ name: slot.name })}
                </span>
              </Button>
            {/if}
            <Button
              type="button"
              variant="outline"
              class="min-h-11"
              disabled={busy}
              onclick={() => openReplace(slot)}
            >
              <Icon name="pencil-simple" class="size-4" />
              <span>{m.sources_rules_credentials_replace({ name: slot.name })}</span>
            </Button>
            {#if slot.has_value}
              <Button
                type="button"
                variant="destructive"
                class="min-h-11"
                disabled={busy}
                onclick={() => openClear(slot)}
              >
                <Icon name="trash" class="size-4" />
                <span>{m.sources_rules_credentials_clear({ name: slot.name })}</span>
              </Button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  {#if actionError}
    <p
      class="border-danger/35 bg-danger/10 text-danger m-4 rounded-lg border px-3 py-2 text-sm"
      role="alert"
    >
      {m.sources_rules_credentials_action_error()}
    </p>
  {/if}
</section>

{#if replaceOpen && !busy && activeSlotCurrent}
  <Dialog bind:open={replaceOpen}>
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{m.sources_rules_credentials_replace_title()}</DialogTitle>
        <DialogDescription>{m.sources_rules_credentials_replace_description()}</DialogDescription>
      </DialogHeader>
      <div class="space-y-1.5 py-2">
        <Label for="rule-credential-replacement">
          {m.sources_rules_credentials_value_label()}
        </Label>
        <Input
          bind:ref={() => replacementInput, setReplacementInput}
          id="rule-credential-replacement"
          type="password"
          class="min-h-11"
          autocomplete="new-password"
          placeholder={m.sources_rules_credentials_value_placeholder()}
        />
      </div>
      <DialogFooter class="gap-2 sm:gap-2">
        <Button type="button" variant="outline" class="min-h-11" onclick={closeCredentialDialogs}>
          {m.sources_rules_action_cancel()}
        </Button>
        <Button type="button" class="min-h-11" disabled={!canReplace} onclick={replaceCredential}>
          {m.sources_rules_credentials_replace_confirm()}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
{/if}

{#if clearOpen && !busy && activeSlotCurrent}
  <Dialog bind:open={clearOpen}>
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{m.sources_rules_credentials_clear_title()}</DialogTitle>
        <DialogDescription>
          {m.sources_rules_credentials_clear_description({ name: activeSlot?.name ?? '' })}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter class="gap-2 sm:gap-2">
        <Button type="button" variant="outline" class="min-h-11" onclick={closeCredentialDialogs}>
          {m.sources_rules_action_cancel()}
        </Button>
        <Button
          type="button"
          variant="destructive"
          class="min-h-11"
          disabled={busy}
          onclick={clearCredential}
        >
          {m.sources_rules_credentials_clear_confirm()}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
{/if}
