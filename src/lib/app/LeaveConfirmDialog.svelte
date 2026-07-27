<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
  import { m } from '$lib/i18n';
  import {
    getLeaveCoordinatorSnapshot,
    resolvePendingLeave,
    type LeaveAction,
  } from './leave-coordinator.svelte';

  const snapshot = $derived(getLeaveCoordinatorSnapshot());
  const resolving = $derived(snapshot.phase === 'resolving');

  function choose(action: LeaveAction): void {
    void resolvePendingLeave(action);
  }

  function stopPointerDown(event: PointerEvent): void {
    event.stopPropagation();
  }
</script>

{#if snapshot.error && !snapshot.open}
  <p class="sr-only" role="alert" data-leave-error={snapshot.error}>
    {m.sources_rules_leave_error()}
  </p>
{/if}

<Dialog.Root open={snapshot.open}>
  <Dialog.Content
    class="sm:max-w-md"
    showCloseButton={false}
    escapeKeydownBehavior="ignore"
    interactOutsideBehavior="ignore"
    aria-busy={resolving}
    data-testid="leave-confirm-dialog"
    data-leave-intent={snapshot.kind ?? undefined}
    onpointerdown={stopPointerDown}
  >
    <Dialog.Header>
      <Dialog.Title>{m.sources_rules_leave_title()}</Dialog.Title>
      <Dialog.Description>{m.sources_rules_leave_description()}</Dialog.Description>
    </Dialog.Header>

    {#if snapshot.error}
      <p
        class="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive"
        role="alert"
        data-leave-error={snapshot.error}
      >
        {m.sources_rules_leave_error()}
      </p>
    {/if}

    <Dialog.Footer class="sm:grid sm:grid-cols-3">
      <Button
        variant="outline"
        size="lg"
        disabled={resolving}
        data-leave-action="continue"
        onclick={() => choose('continue')}
      >
        {m.sources_rules_leave_continue()}
      </Button>
      <Button
        variant="destructive"
        size="lg"
        disabled={resolving}
        data-leave-action="discard"
        onclick={() => choose('discard')}
      >
        {m.sources_rules_leave_discard()}
      </Button>
      <Button
        size="lg"
        disabled={resolving}
        data-leave-action="save"
        onclick={() => choose('save')}
      >
        {snapshot.action === 'save' ? m.sources_rules_leave_saving() : m.sources_rules_leave_save()}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
