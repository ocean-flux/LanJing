<script lang="ts">
  import {
    installCandidate,
    prepareInstall,
    type CapabilityGrantPreset,
    type InstallCandidate,
  } from '$lib/stores/rules.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Textarea } from '$lib/components/ui/textarea';
  import { m } from '$lib/i18n';

  let sourceJson = $state('');
  let candidate = $state<InstallCandidate | null>(null);
  let grant = $state<CapabilityGrantPreset>('none');
  let loading = $state(false);
  let error = $state<string | null>(null);
  let success = $state<string | null>(null);

  async function handlePrepare(): Promise<void> {
    loading = true;
    error = null;
    success = null;
    candidate = null;

    try {
      candidate = await prepareInstall(sourceJson);
      grant = candidate.required_grant.network ? 'network_only' : 'none';
    } catch (caught) {
      error = String(caught);
    } finally {
      loading = false;
    }
  }

  async function handleInstall(): Promise<void> {
    if (!candidate) return;
    loading = true;
    error = null;
    success = null;

    try {
      const source = await installCandidate(candidate.id, grant);
      success = m.debug_import_success({ id: source.source_id });
      candidate = null;
      sourceJson = '';
    } catch (caught) {
      error = String(caught);
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex h-full flex-col gap-4 overflow-auto">
  <h2 class="text-lg font-semibold">{m.debug_import_title()}</h2>

  <div class="flex flex-col gap-2">
    <label for="rule-json" class="text-sm font-medium">{m.debug_rule_json_label()}</label>
    <Textarea
      id="rule-json"
      bind:value={sourceJson}
      placeholder={m.debug_rule_json_placeholder()}
      rows={8}
      disabled={loading}
    />
  </div>

  <Button onclick={handlePrepare} disabled={loading || !sourceJson.trim()}>
    {loading ? m.debug_parse_loading() : m.debug_parse_preview()}
  </Button>

  {#if error}
    <div
      class="rounded-md border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
    >
      {error}
    </div>
  {/if}

  {#if success}
    <div
      class="rounded-md border border-emerald-500/30 bg-emerald-50 p-3 text-sm text-emerald-700 dark:bg-emerald-950/20 dark:text-emerald-400"
    >
      {success}
    </div>
  {/if}

  {#if candidate}
    <div class="space-y-3 rounded-md border bg-card p-4">
      <h3 class="text-base font-semibold">{m.debug_preview_title()}</h3>
      <div class="grid grid-cols-2 gap-2 text-sm">
        <span class="text-muted-foreground">{m.debug_source_url()}</span>
        <span>{candidate.profile.title}</span>
        <span class="text-muted-foreground">{m.debug_sandbox_network()}</span>
        <span>{candidate.required_grant.network ? m.debug_allowed() : m.debug_denied()}</span>
      </div>

      {#if candidate.profile.risk_notes.length > 0}
        <ul class="list-disc space-y-1 pl-5 text-xs text-muted-foreground">
          {#each candidate.profile.risk_notes as note (note)}
            <li>{note}</li>
          {/each}
        </ul>
      {/if}

      {#if candidate.diagnostics.length > 0}
        <ul class="space-y-1 text-xs text-muted-foreground">
          {#each candidate.diagnostics as diagnostic (diagnostic.code + diagnostic.message)}
            <li>{diagnostic.code}: {diagnostic.message}</li>
          {/each}
        </ul>
      {/if}

      <label class="flex items-center gap-2 text-sm">
        <span>{m.debug_sandbox_network()}</span>
        <select
          bind:value={grant}
          disabled={loading}
          class="h-9 rounded-md border bg-background px-2"
        >
          <option value="none">{m.debug_denied()}</option>
          <option value="network_only">{m.debug_allowed()}</option>
        </select>
      </label>

      <Button onclick={handleInstall} disabled={loading} variant="default" class="w-full">
        {loading ? m.debug_importing() : m.debug_install_candidate()}
      </Button>
    </div>
  {/if}
</div>
