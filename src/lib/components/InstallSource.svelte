<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { Textarea } from '$lib/components/ui/textarea';
  import { m } from '$lib/i18n';
  import {
    installCandidate,
    prepareInstall,
    type CapabilityGrantPreset,
    type InstallCandidate,
  } from '$lib/stores/rules.svelte';

  let sourceJson = $state('');
  let candidate = $state<InstallCandidate | null>(null);
  let grant = $state<CapabilityGrantPreset>('none');
  let loading = $state(false);
  let error = $state<string | null>(null);
  let success = $state<string | null>(null);

  const requiresNetworkGrant = $derived(candidate?.required_grant.network ?? false);
  const canInstall = $derived(
    candidate !== null && !loading && (!requiresNetworkGrant || grant === 'network_only'),
  );

  async function handlePrepare(): Promise<void> {
    loading = true;
    error = null;
    success = null;
    candidate = null;
    grant = 'none';

    try {
      candidate = await prepareInstall(sourceJson);
    } catch (caught) {
      error = String(caught);
    } finally {
      loading = false;
    }
  }

  async function handleInstall(): Promise<void> {
    if (!candidate || (candidate.required_grant.network && grant !== 'network_only')) return;
    loading = true;
    error = null;
    success = null;

    try {
      const source = await installCandidate(candidate.id, grant);
      success = m.sources_install_success({ id: source.source_id });
      candidate = null;
      sourceJson = '';
      grant = 'none';
    } catch (caught) {
      error = String(caught);
    } finally {
      loading = false;
    }
  }
</script>

<section
  class="flex w-full flex-col gap-4"
  data-testid="install-source"
  aria-labelledby="install-source-title"
>
  <header>
    <h2 id="install-source-title" class="text-base font-semibold text-ink">
      {m.sources_install_title()}
    </h2>
  </header>

  <div class="glass-panel flex flex-col gap-3 rounded-xl border border-hairline p-4">
    <label for="rule-json" class="text-sm font-medium text-ink"
      >{m.sources_install_json_label()}</label
    >
    <Textarea
      id="rule-json"
      bind:value={sourceJson}
      placeholder={m.sources_install_json_placeholder()}
      rows={7}
      disabled={loading}
      class="glass-control min-h-32 border-hairline px-3 py-2 text-sm text-ink placeholder:text-ink-subtle focus-visible:border-lantern-strong/50 focus-visible:ring-2 focus-visible:ring-lantern/35"
    />
    <Button
      type="button"
      onclick={handlePrepare}
      disabled={loading || !sourceJson.trim()}
      class="min-h-11 w-full sm:w-auto sm:self-start"
    >
      {loading ? m.sources_install_preparing() : m.sources_install_prepare()}
    </Button>
  </div>

  {#if error}
    <div
      class="border-danger/35 bg-danger/10 text-danger rounded-lg border px-3 py-2.5 text-sm"
      role="alert"
    >
      {error}
    </div>
  {/if}

  {#if success}
    <div
      class="rounded-lg border border-positive/40 bg-positive/10 px-3 py-2.5 text-sm text-positive"
      role="status"
    >
      {success}
    </div>
  {/if}

  {#if candidate}
    <section
      class="grid gap-4 md:grid-cols-[minmax(0,1fr)_minmax(16rem,0.72fr)]"
      data-testid="install-candidate-preview"
      aria-labelledby="install-candidate-title"
    >
      <div class="glass-panel space-y-3 rounded-xl border border-hairline p-4">
        <h3 id="install-candidate-title" class="text-sm font-semibold text-ink">
          {m.sources_install_preview_title()}
        </h3>
        <dl class="grid grid-cols-[minmax(7rem,auto)_minmax(0,1fr)] gap-x-3 gap-y-2 text-sm">
          <dt class="text-ink-muted">{m.sources_install_source_name()}</dt>
          <dd class="min-w-0 font-medium break-words text-ink">{candidate.profile.title}</dd>
          {#if candidate.profile.version}
            <dt class="text-ink-muted">{m.sources_install_version()}</dt>
            <dd class="min-w-0 font-medium break-words text-ink">{candidate.profile.version}</dd>
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
        class="glass-panel space-y-3 rounded-xl border border-hairline p-4"
        data-testid="install-actions"
      >
        {#if requiresNetworkGrant}
          <p
            class="rounded-lg border border-lantern/35 bg-lantern-soft/25 px-3 py-2 text-xs font-medium text-ink"
          >
            {m.sources_install_network_required_notice()}
          </p>
          <label class="flex flex-col gap-1.5 text-sm text-ink" for="network-grant">
            <span class="text-xs font-medium text-ink-muted"
              >{m.sources_install_network_grant()}</span
            >
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
          onclick={handleInstall}
          disabled={!canInstall}
          class="min-h-11 w-full"
        >
          {loading ? m.sources_install_installing() : m.sources_install_action()}
        </Button>
      </div>
    </section>
  {/if}
</section>
