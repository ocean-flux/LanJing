<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { m } from '$lib/i18n';
  import type { CapabilityGrantPreset, InstallCandidate } from '$lib/stores/rules.svelte';

  type Props = {
    candidate: InstallCandidate;
    grant: CapabilityGrantPreset;
    loading?: boolean;
    stickyActions?: boolean;
    onInstall: () => void | Promise<void>;
  };

  let {
    candidate,
    grant = $bindable(),
    loading = false,
    stickyActions = false,
    onInstall,
  }: Props = $props();

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
</script>

<section
  class="flex min-w-0 flex-col gap-4"
  data-testid="install-candidate-preview"
  aria-labelledby="install-candidate-title"
>
  <div class="glass-panel space-y-3 rounded-xl border border-hairline p-4">
    <h3 id="install-candidate-title" class="text-sm font-semibold text-ink">
      {m.sources_install_preview_title()}
    </h3>
    <dl class="grid grid-cols-[minmax(7rem,auto)_minmax(0,1fr)] gap-x-3 gap-y-2 text-sm">
      <dt class="text-ink-muted">{m.sources_install_source_name()}</dt>
      <dd class="min-w-0 font-medium wrap-break-word text-ink">{candidate.profile.title}</dd>
      {#if groupLabel}
        <dt class="text-ink-muted">{m.sources_group_label()}</dt>
        <dd class="min-w-0 font-medium wrap-break-word text-ink">{groupLabel}</dd>
      {/if}
      {#if candidate.profile.version}
        <dt class="text-ink-muted">{m.sources_install_version()}</dt>
        <dd class="min-w-0 font-medium wrap-break-word text-ink">{candidate.profile.version}</dd>
      {/if}
      <dt class="text-ink-muted">{m.sources_install_network_grant()}</dt>
      <dd class="font-medium text-ink">
        {requiresNetworkGrant
          ? m.sources_install_network_required()
          : m.sources_install_network_not_required()}
      </dd>
    </dl>

    {#if candidate.profile.risk_notes.length > 0}
      <ul class="list-disc space-y-1 pl-5 text-xs leading-5 text-ink-muted">
        {#each candidate.profile.risk_notes as note (note)}
          <li>{note}</li>
        {/each}
      </ul>
    {/if}

    {#if candidate.diagnostics.length > 0}
      <ul
        class="space-y-1 rounded-lg border border-hairline bg-surface-2 px-2.5 py-2 text-xs text-ink-muted"
      >
        {#each candidate.diagnostics as diagnostic (diagnostic.code + diagnostic.message)}
          <li>
            <span class="font-medium text-ink">{diagnostic.code}</span>: {diagnostic.message}
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <div
    class={[
      'glass-panel space-y-3 rounded-xl border border-hairline p-4',
      stickyActions && 'sticky bottom-0 z-10 lg:static lg:z-auto',
    ]}
    data-testid="install-actions"
  >
    {#if requiresUnsupportedSystemGrant}
      <p class="border-danger/35 bg-danger/10 text-danger rounded-lg border px-3 py-2 text-xs">
        {m.sources_install_system_unsupported()}
      </p>
    {:else if requiresNetworkGrant}
      <p
        class="rounded-lg border border-lantern/35 bg-lantern-soft/25 px-3 py-2 text-xs font-medium text-ink"
      >
        {m.sources_install_network_required_notice()}
      </p>
      <label class="flex flex-col gap-1.5 text-sm text-ink" for="network-grant">
        <span class="text-xs font-medium text-ink-muted">{m.sources_install_network_grant()}</span>
        <select
          id="network-grant"
          bind:value={grant}
          disabled={loading}
          class="glass-control min-h-11 w-full rounded-lg border border-hairline px-2.5 text-sm text-ink outline-none focus-visible:border-lantern-strong/50 focus-visible:shadow-[var(--focus-ring)]"
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
      onclick={() => void onInstall()}
      disabled={!canInstall}
      class="min-h-11 w-full active:scale-[0.98]"
    >
      {loading ? m.sources_install_installing() : m.sources_install_action()}
    </Button>
    {#if stickyActions}
      <div class="h-[env(safe-area-inset-bottom)] lg:hidden" aria-hidden="true"></div>
    {/if}
  </div>
</section>
