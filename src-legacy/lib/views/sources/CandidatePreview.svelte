<script lang="ts">
  import Notice from '$lib/components/Notice.svelte';
  import { Button } from '$lib/components/ui/button';
  import { m } from '$lib/i18n';
  import type {
    CapabilityGrantPreset,
    InstallCandidate,
    StandardIntent,
  } from '$lib/stores/rules.svelte';
  import { classifyExpiresAt, truncateHash } from './candidate-preview';
  import { localizeImportDiagnostic } from './import-diagnostics';
  type CandidatePreviewDensity = 'compact' | 'full';

  type Props = {
    candidate: InstallCandidate;
    grant: CapabilityGrantPreset;
    loading?: boolean;
    stickyActions?: boolean;
    density?: CandidatePreviewDensity;
    /** When true, show subtle "validated only" status (prepare succeeded). */
    validated?: boolean;
    onInstall: () => void | Promise<void>;
  };

  let {
    candidate,
    grant = $bindable(),
    loading = false,
    stickyActions = false,
    density = 'compact',
    validated = true,
    onInstall,
  }: Props = $props();

  const uid = $props.id();
  const titleId = `${uid}-title`;
  const grantId = `${uid}-network-grant`;

  const intentLabels: Record<StandardIntent, () => string> = {
    Search: () => m.sources_intent_search(),
    Discover: () => m.sources_intent_discover(),
    ResolveItem: () => m.sources_intent_resolve_item(),
    ListUnits: () => m.sources_intent_list_units(),
    ResolveAsset: () => m.sources_intent_resolve_asset(),
    ContinueAction: () => m.sources_intent_continue_action(),
  };

  const groupLabel = $derived(candidate.profile.group?.trim() || null);
  const requiresNetworkGrant = $derived(candidate.required_grant.network);
  const requiresUnsupportedSystemGrant = $derived(
    candidate.required_grant.system.fs ||
      candidate.required_grant.system.env ||
      candidate.required_grant.system.process,
  );
  const canInstall = $derived(
    !loading &&
      !requiresUnsupportedSystemGrant &&
      (!requiresNetworkGrant || grant === 'network_only'),
  );

  const definitionHashLabel = $derived(truncateHash(candidate.definition_hash));
  const planHashLabel = $derived(truncateHash(candidate.plan_hash));
  const expiresHint = $derived(classifyExpiresAt(candidate.expires_at_ms));

  function intentLabel(intent: StandardIntent): string {
    return intentLabels[intent]?.() ?? intent;
  }

  function expiresText(): string {
    switch (expiresHint.kind) {
      case 'expired':
        return m.sources_install_expired();
      case 'minutes':
        return m.sources_install_expires_in_minutes({ minutes: expiresHint.minutes });
      case 'hours':
        return m.sources_install_expires_in_hours({ hours: expiresHint.hours });
      case 'absolute':
        return m.sources_install_expires_at({ time: expiresHint.isoDate });
    }
  }

  function handleInstall(): void {
    if (!canInstall) return;
    void onInstall();
  }
</script>

<section
  class={['flex min-w-0 flex-col', density === 'full' ? 'gap-3' : 'gap-2']}
  data-testid="install-candidate-preview"
  data-density={density}
  aria-labelledby={titleId}
>
  <div class={density === 'full' ? 'space-y-3' : 'space-y-2'}>
    <div class="flex flex-wrap items-baseline justify-between gap-2 border-b border-hairline pb-2">
      <h3 id={titleId} class="text-sm font-semibold text-ink">
        {m.sources_install_preview_title()}
      </h3>
      {#if validated}
        <p class="text-xs text-ink-muted" data-testid="install-validated-hint" role="status">
          {m.sources_install_validated_hint()}
        </p>
      {/if}
    </div>

    <dl class={['grid min-w-0 gap-x-4 gap-y-2 text-sm', density === 'full' && 'sm:grid-cols-2']}>
      <div class="min-w-0">
        <dt class="text-xs text-ink-muted">{m.sources_install_source_name()}</dt>
        <dd class="mt-0.5 min-w-0 font-medium wrap-break-word text-ink">
          {candidate.profile.title}
        </dd>
      </div>
      {#if groupLabel}
        <div class="min-w-0">
          <dt class="text-xs text-ink-muted">{m.sources_group_label()}</dt>
          <dd class="mt-0.5 min-w-0 font-medium wrap-break-word text-ink">{groupLabel}</dd>
        </div>
      {/if}
      {#if candidate.profile.version}
        <div class="min-w-0">
          <dt class="text-xs text-ink-muted">{m.sources_install_version()}</dt>
          <dd class="mt-0.5 min-w-0 font-medium wrap-break-word text-ink">
            {candidate.profile.version}
          </dd>
        </div>
      {/if}
      <div class="min-w-0">
        <dt class="text-xs text-ink-muted">{m.sources_install_network_grant()}</dt>
        <dd class="mt-0.5 font-medium text-ink">
          {requiresNetworkGrant
            ? m.sources_install_network_required()
            : m.sources_install_network_not_required()}
        </dd>
      </div>
      <div class="min-w-0">
        <dt class="text-xs text-ink-muted">{m.sources_install_definition_hash()}</dt>
        <dd
          class="mt-0.5 min-w-0 font-mono text-xs wrap-break-word text-ink"
          data-testid="install-definition-hash"
          title={candidate.definition_hash}
        >
          {definitionHashLabel}
        </dd>
      </div>
      <div class="min-w-0">
        <dt class="text-xs text-ink-muted">{m.sources_install_plan_hash()}</dt>
        <dd
          class="mt-0.5 min-w-0 font-mono text-xs wrap-break-word text-ink"
          data-testid="install-plan-hash"
          title={candidate.plan_hash}
        >
          {planHashLabel}
        </dd>
      </div>
      <div class="min-w-0">
        <dt class="text-xs text-ink-muted">{m.sources_install_expires()}</dt>
        <dd class="mt-0.5 min-w-0 text-ink" data-testid="install-expires">{expiresText()}</dd>
      </div>
    </dl>

    <div data-testid="install-supported-intents">
      <p class="text-xs font-medium text-ink-muted">{m.sources_supported_intents()}</p>
      {#if candidate.profile.supported_intents.length > 0}
        <ul class="mt-1 flex flex-wrap gap-1" aria-label={m.sources_supported_intents()}>
          {#each candidate.profile.supported_intents as intent (intent)}
            <li
              class="rounded-md border border-hairline bg-surface-2 px-1.5 py-0.5 text-xs text-ink"
            >
              {intentLabel(intent)}
            </li>
          {/each}
        </ul>
      {:else}
        <p class="mt-1 text-xs text-ink-subtle">{m.sources_no_intents()}</p>
      {/if}
    </div>

    {#if candidate.profile.risk_notes.length > 0}
      <div>
        <p class="text-xs font-medium text-ink-muted">{m.sources_risk_notes()}</p>
        <ul class="mt-1 list-disc space-y-0.5 pl-4 text-xs leading-5 break-words text-ink-muted">
          {#each candidate.profile.risk_notes as note (note)}
            <li>{note}</li>
          {/each}
        </ul>
      </div>
    {/if}

    {#if candidate.diagnostics.length > 0}
      <ul
        class="divide-y divide-hairline rounded-[var(--radius-control)] border border-hairline bg-surface-2/60 px-2.5 text-xs"
        data-testid="install-diagnostics"
      >
        {#each candidate.diagnostics as diagnostic, index (`${index}:${diagnostic.code}:${diagnostic.span?.path ?? ''}:${diagnostic.span?.start ?? ''}:${diagnostic.span?.end ?? ''}`)}
          <li
            class="space-y-1 py-2"
            data-diagnostic-code={diagnostic.code}
            data-diagnostic-path={diagnostic.span?.path}
            data-diagnostic-span-start={diagnostic.span?.start}
            data-diagnostic-span-end={diagnostic.span?.end}
          >
            <div class="flex flex-col gap-0.5 sm:flex-row sm:items-baseline sm:gap-2">
              <code class="min-w-0 font-mono font-medium break-all text-ink">{diagnostic.code}</code
              >
              <p class="min-w-0 text-ink-muted">{localizeImportDiagnostic(diagnostic.code)}</p>
            </div>
            {#if diagnostic.span}
              <p
                class="flex flex-wrap gap-x-2 gap-y-0.5 font-mono text-[0.6875rem] text-ink-subtle"
              >
                {#if diagnostic.span.path !== null}
                  <span class="min-w-0 break-all">
                    {m.sources_diagnostic_path({ path: diagnostic.span.path || '/' })}
                  </span>
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
  </div>

  <div
    class={[
      'space-y-2.5 border-t border-hairline pt-3',
      stickyActions &&
        'sticky bottom-0 bg-surface-1 pb-(--density-panel-padding-compact) lg:static lg:pb-0',
    ]}
    data-testid="install-actions"
  >
    {#if requiresUnsupportedSystemGrant}
      <Notice tone="danger" role="alert" icon="warning-circle">
        {m.sources_install_system_unsupported()}
      </Notice>
    {:else if requiresNetworkGrant}
      <Notice tone="info" role="note" icon="warning-circle">
        {m.sources_install_network_required_notice()}
      </Notice>
      <label class="flex flex-col gap-1.5 text-sm text-ink" for={grantId}>
        <span class="text-xs font-medium text-ink-muted">{m.sources_install_network_grant()}</span>
        <select
          id={grantId}
          bind:value={grant}
          disabled={loading}
          class="glass-control h-(--density-control-md) w-full rounded-md border border-hairline px-2.5 text-sm text-ink outline-none focus-visible:border-lantern-strong/60 focus-visible:shadow-[var(--focus-ring)]"
        >
          <option value="none">{m.sources_install_grant_prompt()}</option>
          <option value="network_only">{m.sources_install_grant_network_only()}</option>
        </select>
      </label>
    {:else}
      <p class="text-xs text-ink-muted">{m.sources_install_network_not_required()}</p>
    {/if}

    <Button
      type="button"
      onclick={handleInstall}
      disabled={!canInstall}
      class="w-full"
      data-testid="install-candidate-action"
    >
      {loading ? m.sources_install_installing() : m.sources_install_action()}
    </Button>
  </div>
</section>
