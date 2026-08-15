<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import Notice from '$lib/components/Notice.svelte';
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
  let fileName = $state<string | null>(null);
  let fileInput = $state<HTMLInputElement | null>(null);

  function resetSharedState(): void {
    sourceJson = '';
    maccmsUrl = '';
    candidate = null;
    grant = 'none';
    error = null;
    success = null;
    fileName = null;
  }

  function selectFormat(next: InstallFormat): void {
    if (next === format) return;
    format = next;
    resetSharedState();
  }

  /** 客户端守卫：多来源数组必须走深链选择，不能进入单来源准备流程。 */
  function isLegadoJsonArray(text: string): boolean {
    try {
      return Array.isArray(JSON.parse(text));
    } catch {
      return false;
    }
  }

  async function handlePrepare(): Promise<void> {
    loading = true;
    error = null;
    success = null;
    candidate = null;
    grant = 'none';

    try {
      if (format === 'legado') {
        if (isLegadoJsonArray(sourceJson)) {
          error = m.sources_install_array_not_supported();
          return;
        }
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
      fileName = null;
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
      fileName = file.name;
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
</script>

<section
  class="flex w-full min-w-0 flex-col gap-3"
  data-testid="install-source"
  aria-labelledby={showHeading ? 'install-source-title' : undefined}
  aria-busy={loading}
>
  {#if showHeading}
    <header class="border-b border-hairline pb-2">
      <h2 id="install-source-title" class="text-base font-semibold text-ink">
        {m.sources_install_title()}
      </h2>
    </header>
  {/if}

  <div
    class="flex flex-wrap gap-1"
    role="group"
    aria-label={m.sources_install_format_label()}
    data-testid="install-format"
  >
    <Button
      type="button"
      size="sm"
      variant={format === 'legado' ? 'secondary' : 'ghost'}
      aria-pressed={format === 'legado'}
      data-testid="install-format-legado"
      onclick={() => selectFormat('legado')}
    >
      {m.sources_install_format_legado()}
    </Button>
    <Button
      type="button"
      size="sm"
      variant={format === 'maccms' ? 'secondary' : 'ghost'}
      aria-pressed={format === 'maccms'}
      data-testid="install-format-maccms"
      onclick={() => selectFormat('maccms')}
    >
      {m.sources_install_format_maccms()}
    </Button>
  </div>

  <div
    class={[
      'grid min-w-0 gap-3',
      candidate && 'lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)] lg:gap-0',
    ]}
  >
    <div class={['flex min-w-0 flex-col gap-3', candidate && 'lg:pr-4']}>
      <div class="flex min-w-0 flex-col gap-2.5 border-t border-hairline pt-3">
        {#if format === 'legado'}
          <label for={fieldId} class="text-sm font-medium text-ink">
            {m.sources_install_json_label()}
          </label>
          <Textarea
            id={fieldId}
            bind:value={sourceJson}
            placeholder={m.sources_install_json_placeholder()}
            rows={7}
            disabled={loading}
            class="min-h-40 font-mono"
            data-testid="install-json-input"
          />
          {#if fileName}
            <p class="text-xs text-ink-muted" data-testid="install-file-name">
              {m.sources_install_file_name({ name: fileName })}
            </p>
          {/if}
          <div class="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-center">
            <Button
              type="button"
              variant="outline"
              onclick={handlePrepare}
              disabled={prepareDisabled}
              data-testid="install-legado-prepare"
              class="w-full sm:w-auto"
            >
              {loading ? m.sources_install_validating() : m.sources_install_validate()}
            </Button>
            <Button
              type="button"
              variant="ghost"
              onclick={openLocalFile}
              disabled={loading}
              class="w-full sm:w-auto"
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
          <label for={maccmsUrlId} class="text-sm font-medium text-ink">
            {m.sources_install_maccms_url_label()}
          </label>
          <Input
            id={maccmsUrlId}
            type="url"
            bind:value={maccmsUrl}
            placeholder={m.sources_install_maccms_url_placeholder()}
            disabled={loading}
            data-testid="install-maccms-url"
          />
          <div class="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-center">
            <Button
              type="button"
              variant="outline"
              onclick={handlePrepare}
              disabled={prepareDisabled}
              data-testid="install-maccms-prepare"
              class="w-full sm:w-auto"
            >
              {loading ? m.sources_install_validating() : m.sources_install_validate()}
            </Button>
          </div>
        {/if}
      </div>

      {#if error}
        <div data-testid="install-error">
          <Notice tone="danger" role="alert" icon="warning-circle">
            <span class="break-words">{error}</span>
          </Notice>
        </div>
      {/if}

      {#if success}
        <Notice tone="success" role="status" icon="check-circle">
          <span class="break-words">{success}</span>
        </Notice>
      {/if}
    </div>

    {#if candidate}
      <div class="min-w-0 border-t border-hairline pt-3 lg:border-t-0 lg:border-l lg:pt-0 lg:pl-4">
        <CandidatePreview
          {candidate}
          bind:grant
          {loading}
          {stickyActions}
          density="full"
          validated={true}
          onInstall={handleInstall}
        />
      </div>
    {/if}
  </div>
</section>
