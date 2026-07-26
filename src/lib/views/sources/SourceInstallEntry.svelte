<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Textarea } from '$lib/components/ui/textarea';
  import { m } from '$lib/i18n';
  import {
    installCandidate,
    prepareInstall,
    prepareMaccmsInstall,
    type CapabilityGrantPreset,
    type InstallCandidate,
  } from '$lib/stores/rules.svelte';
  import CandidatePreview from './CandidatePreview.svelte';

  type InstallFormat = 'legado' | 'maccms';

  type Props = {
    showHeading?: boolean;
    stickyActions?: boolean;
    onInstalled?: () => void;
  };

  let { showHeading = true, stickyActions = false, onInstalled }: Props = $props();

  const fieldId = `rule-json-${Math.random().toString(36).slice(2, 10)}`;
  const fileInputId = `rule-file-${Math.random().toString(36).slice(2, 10)}`;
  const maccmsUrlId = `maccms-url-${Math.random().toString(36).slice(2, 10)}`;

  let format = $state<InstallFormat>('legado');
  let sourceJson = $state('');
  let maccmsUrl = $state('');
  let candidate = $state<InstallCandidate | null>(null);
  let grant = $state<CapabilityGrantPreset>('none');
  let loading = $state(false);
  let error = $state<string | null>(null);
  let success = $state<string | null>(null);
  let fileInput = $state<HTMLInputElement | null>(null);

  function resetSharedState(): void {
    sourceJson = '';
    maccmsUrl = '';
    candidate = null;
    grant = 'none';
    error = null;
    success = null;
  }

  function selectFormat(next: InstallFormat): void {
    if (next === format) return;
    format = next;
    resetSharedState();
  }

  async function handlePrepare(): Promise<void> {
    loading = true;
    error = null;
    success = null;
    candidate = null;
    grant = 'none';

    try {
      if (format === 'legado') {
        candidate = await prepareInstall(sourceJson);
      } else {
        candidate = await prepareMaccmsInstall(maccmsUrl.trim());
      }
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
      maccmsUrl = '';
      grant = 'none';
      onInstalled?.();
    } catch (caught) {
      error = String(caught);
    } finally {
      loading = false;
    }
  }

  function openLocalFile(): void {
    fileInput?.click();
  }

  async function handleFileChange(event: Event): Promise<void> {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;

    try {
      sourceJson = await file.text();
      error = null;
      success = null;
      candidate = null;
      grant = 'none';
    } catch (caught) {
      error = String(caught);
    }
  }

  const prepareDisabled = $derived(
    loading || (format === 'legado' ? !sourceJson.trim() : !maccmsUrl.trim()),
  );

  function formatPillClass(active: boolean): string {
    return `inline-flex min-h-11 shrink-0 items-center rounded-lg border px-3 text-sm font-medium outline-none focus-visible:shadow-[var(--focus-ring)] ${
      active
        ? 'border-hairline-strong bg-lantern-soft text-ink'
        : 'border-hairline bg-surface-1 text-ink-muted hover:bg-surface-2'
    }`;
  }
</script>

<section
  class="flex w-full flex-col gap-4"
  data-testid="install-source"
  aria-labelledby={showHeading ? 'install-source-title' : undefined}
>
  {#if showHeading}
    <header>
      <h2 id="install-source-title" class="text-base font-semibold text-ink">
        {m.sources_install_title()}
      </h2>
    </header>
  {/if}

  <div
    class="flex flex-wrap gap-2"
    role="toolbar"
    aria-label={m.sources_install_format_label()}
    data-testid="install-format"
  >
    <button
      type="button"
      class={formatPillClass(format === 'legado')}
      aria-pressed={format === 'legado'}
      data-testid="install-format-legado"
      onclick={() => selectFormat('legado')}
    >
      {m.sources_install_format_legado()}
    </button>
    <button
      type="button"
      class={formatPillClass(format === 'maccms')}
      aria-pressed={format === 'maccms'}
      data-testid="install-format-maccms"
      onclick={() => selectFormat('maccms')}
    >
      {m.sources_install_format_maccms()}
    </button>
  </div>

  <div
    class={[
      'grid min-w-0 gap-4',
      candidate && 'lg:grid-cols-[minmax(0,0.92fr)_minmax(0,1.08fr)] lg:items-start lg:gap-5',
    ]}
  >
    <div class="flex min-w-0 flex-col gap-4">
      <div class="glass-panel flex flex-col gap-3 rounded-xl border border-hairline p-4">
        {#if format === 'legado'}
          <label for={fieldId} class="text-sm font-medium text-ink"
            >{m.sources_install_json_label()}</label
          >
          <Textarea
            id={fieldId}
            bind:value={sourceJson}
            placeholder={m.sources_install_json_placeholder()}
            rows={7}
            disabled={loading}
            class="glass-control min-h-32 border-hairline px-3 py-2 text-sm text-ink placeholder:text-ink-subtle focus-visible:border-lantern-strong/50 focus-visible:ring-2 focus-visible:ring-lantern/35"
          />
          <div class="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-center">
            <Button
              type="button"
              onclick={handlePrepare}
              disabled={prepareDisabled}
              class="min-h-11 w-full active:scale-[0.98] sm:w-auto"
            >
              {loading ? m.sources_install_preparing() : m.sources_install_prepare()}
            </Button>
            <Button
              type="button"
              variant="outline"
              onclick={openLocalFile}
              disabled={loading}
              class="min-h-11 w-full sm:w-auto"
            >
              {m.sources_install_open_file()}
            </Button>
            <Input
              bind:ref={fileInput}
              id={fileInputId}
              type="file"
              accept=".json,application/json,text/plain"
              class="sr-only"
              tabindex={-1}
              aria-hidden="true"
              onchange={handleFileChange}
            />
          </div>
        {:else}
          <label for={maccmsUrlId} class="text-sm font-medium text-ink"
            >{m.sources_install_maccms_url_label()}</label
          >
          <Input
            id={maccmsUrlId}
            type="url"
            bind:value={maccmsUrl}
            placeholder={m.sources_install_maccms_url_placeholder()}
            disabled={loading}
            data-testid="install-maccms-url"
            class="glass-control min-h-11 border-hairline px-3 text-sm text-ink placeholder:text-ink-subtle focus-visible:border-lantern-strong/50 focus-visible:ring-2 focus-visible:ring-lantern/35"
          />
          <div class="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-center">
            <Button
              type="button"
              onclick={handlePrepare}
              disabled={prepareDisabled}
              data-testid="install-maccms-prepare"
              class="min-h-11 w-full active:scale-[0.98] sm:w-auto"
            >
              {loading ? m.sources_install_preparing() : m.sources_install_prepare()}
            </Button>
          </div>
        {/if}
      </div>

      {#if error}
        <div
          class="border-danger/35 bg-danger/10 text-danger rounded-lg border px-3 py-2.5 text-sm break-words"
          role="alert"
        >
          {error}
        </div>
      {/if}

      {#if success}
        <div
          class="rounded-lg border border-positive/40 bg-positive/10 px-3 py-2.5 text-sm break-words text-positive"
          role="status"
        >
          {success}
        </div>
      {/if}
    </div>

    {#if candidate}
      <CandidatePreview
        {candidate}
        bind:grant
        {loading}
        {stickyActions}
        onInstall={handleInstall}
      />
    {/if}
  </div>
</section>
