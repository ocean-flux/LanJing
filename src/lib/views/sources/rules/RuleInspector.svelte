<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
  import { m } from '$lib/i18n';
  import type {
    RuleEditorSession,
    RuleEditorSessionSnapshot,
  } from '$lib/stores/rule-editor-session.svelte';
  import CandidatePreview from '../CandidatePreview.svelte';
  import { localizeSourceDiagnostic } from '../source-diagnostics';
  import RuleCredentialPanel from './RuleCredentialPanel.svelte';

  type Props = {
    session: RuleEditorSession;
    snapshot: RuleEditorSessionSnapshot;
  };

  let { session, snapshot }: Props = $props();
  let installFailed = $state(false);

  const authoringBlocked = $derived(
    snapshot.diagnostics.some(
      (diagnostic) => diagnostic.severity === 'error' || diagnostic.support === 'blocked',
    ),
  );
  const credentialBusy = $derived(
    snapshot.dirty ||
      snapshot.conflict !== null ||
      snapshot.save_phase !== 'idle' ||
      snapshot.rebase_pending ||
      snapshot.locked,
  );

  async function installPrepared(): Promise<void> {
    installFailed = false;
    try {
      const result = await session.installPrepared();
      if (result.status !== 'installed') installFailed = true;
    } catch {
      installFailed = true;
    }
  }
</script>

<aside class="flex min-h-0 min-w-0 flex-col" aria-labelledby="rule-inspector-title">
  <header class="border-b border-hairline px-4 py-3">
    <h2 id="rule-inspector-title" class="text-sm font-semibold text-ink">
      {m.sources_rules_inspector_title()}
    </h2>
  </header>

  <div class="min-h-0 flex-1 divide-y divide-hairline overflow-y-auto">
    <section class="px-4 py-3" aria-labelledby="rule-diagnostics-title">
      <h3 id="rule-diagnostics-title" class="text-sm font-semibold text-ink">
        {m.sources_rules_diagnostics_title()}
      </h3>

      {#if snapshot.diagnostics.length === 0 && snapshot.prepare_diagnostics.length === 0}
        <p class="mt-2 text-sm text-ink-muted" role="status">
          {m.sources_rules_diagnostics_empty()}
        </p>
      {:else}
        <ul class="mt-2 space-y-2">
          {#each snapshot.diagnostics as diagnostic (`authoring:${diagnostic.code}:${diagnostic.path}:${diagnostic.byte_offset}`)}
            <li
              class={[
                'rounded-lg border px-3 py-2 text-xs',
                diagnostic.severity === 'error' || diagnostic.support === 'blocked'
                  ? 'border-danger/35 bg-danger/10 text-danger'
                  : 'border-hairline bg-surface-2 text-ink-muted',
              ]}
              data-diagnostic-code={diagnostic.code}
              data-diagnostic-path={diagnostic.path}
              data-diagnostic-span-start={diagnostic.byte_offset}
              data-diagnostic-span-end={diagnostic.byte_offset + diagnostic.byte_length}
            >
              <div class="flex items-start gap-2">
                <Icon
                  name={diagnostic.severity === 'error' ? 'warning-circle' : 'shield-check'}
                  class="mt-0.5 size-4 shrink-0"
                />
                <div class="min-w-0">
                  <code class="font-mono break-all">{diagnostic.code}</code>
                  <p class="mt-1 break-words">{localizeSourceDiagnostic(diagnostic.code)}</p>
                  <p
                    class="mt-1 flex flex-wrap gap-x-2 gap-y-0.5 font-mono text-[0.6875rem] text-ink-muted"
                  >
                    <span>{m.sources_diagnostic_path({ path: diagnostic.path || '/' })}</span>
                    <span>
                      {m.sources_diagnostic_span({
                        start: diagnostic.byte_offset,
                        end: diagnostic.byte_offset + diagnostic.byte_length,
                      })}
                    </span>
                  </p>
                </div>
              </div>
            </li>
          {/each}
          {#each snapshot.prepare_diagnostics as diagnostic, index (`prepare:${index}:${diagnostic.code}:${diagnostic.span?.path ?? ''}:${diagnostic.span?.start ?? ''}:${diagnostic.span?.end ?? ''}`)}
            <li
              class={[
                'rounded-lg border px-3 py-2 text-xs',
                diagnostic.severity === 'error'
                  ? 'border-danger/35 bg-danger/10 text-danger'
                  : 'border-hairline bg-surface-2 text-ink-muted',
              ]}
              data-diagnostic-code={diagnostic.code}
              data-diagnostic-path={diagnostic.span?.path}
              data-diagnostic-span-start={diagnostic.span?.start}
              data-diagnostic-span-end={diagnostic.span?.end}
            >
              <code class="font-mono break-all">{diagnostic.code}</code>
              <p class="mt-1 break-words">{localizeSourceDiagnostic(diagnostic.code)}</p>
              {#if diagnostic.span}
                <p
                  class="mt-1 flex flex-wrap gap-x-2 gap-y-0.5 font-mono text-[0.6875rem] text-ink-muted"
                >
                  {#if diagnostic.span.path !== null}
                    <span>{m.sources_diagnostic_path({ path: diagnostic.span.path || '/' })}</span>
                  {/if}
                  <span>
                    {m.sources_diagnostic_span({
                      start: diagnostic.span.start,
                      end: diagnostic.span.end,
                    })}
                  </span>
                </p>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}

      {#if authoringBlocked}
        <p
          class="border-danger/35 bg-danger/10 text-danger mt-3 rounded-lg border px-3 py-2 text-xs"
        >
          {m.sources_rules_toolbar_blocked()}
        </p>
      {/if}
    </section>

    {#if snapshot.document}
      <RuleCredentialPanel
        slots={snapshot.document.credential_slots}
        revealedSlotIds={snapshot.revealed_slot_ids}
        busy={credentialBusy}
        onReveal={(slotId) => session.revealCredential(slotId)}
        onClearRevealed={() => session.clearRevealedCredentials()}
        readRevealed={(slotId) => session.readRevealedCredential(slotId)}
        onReplace={async (slotId, value) => {
          await session.replaceCredential(slotId, value);
        }}
        onClear={async (slotId) => {
          await session.clearCredential(slotId);
        }}
      />
    {/if}

    <section class="space-y-3 px-4 py-3" aria-labelledby="rule-candidate-title">
      <h3 id="rule-candidate-title" class="text-sm font-semibold text-ink">
        {m.sources_rules_candidate_title()}
      </h3>

      {#if snapshot.candidate}
        <p class="text-xs leading-5 text-ink-muted">
          {snapshot.candidate.expected_installed_revision > 0
            ? m.sources_rules_candidate_update({
                revision: snapshot.candidate.expected_installed_revision,
              })
            : m.sources_rules_candidate_install()}
        </p>
        <CandidatePreview
          candidate={snapshot.candidate}
          bind:grant={session.grant}
          loading={snapshot.install_pending}
          validated
          onInstall={installPrepared}
        />
      {:else}
        <p class="text-sm leading-6 text-ink-muted">
          {snapshot.dirty
            ? m.sources_rules_toolbar_prepare_dirty()
            : m.sources_rules_candidate_empty()}
        </p>
      {/if}

      {#if snapshot.installed_source && !snapshot.candidate}
        <p
          class="rounded-lg border border-positive/35 bg-positive/10 px-3 py-2 text-sm text-positive"
          role="status"
        >
          {m.sources_rules_candidate_success({ id: snapshot.installed_source.source_id })}
        </p>
      {/if}

      {#if installFailed || snapshot.operation_error === 'install_failed'}
        <p
          class="border-danger/35 bg-danger/10 text-danger rounded-lg border px-3 py-2 text-sm"
          role="alert"
        >
          {m.sources_rules_candidate_error()}
        </p>
      {/if}
    </section>
  </div>
</aside>
