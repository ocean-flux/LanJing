<script lang="ts">
  import Notice from '$lib/components/Notice.svelte';
  import { Button } from '$lib/components/ui/button';
  import {
    Sheet,
    SheetContent,
    SheetDescription,
    SheetFooter,
    SheetHeader,
    SheetTitle,
  } from '$lib/components/ui/sheet';
  import { IsMobile } from '$lib/hooks/is-mobile.svelte';
  import { CATALOG_INSTALL_CAP, type CatalogItem } from '$lib/deeplink/catalog';
  import {
    completeDeepLinkPick,
    dismissDeepLinkSurface,
    getDeepLinkSurface,
  } from '$lib/deeplink/session.svelte';
  import { m } from '$lib/i18n';
  import {
    installCandidate,
    prepareInstall,
    type CapabilityGrantPreset,
    type InstallCandidate,
  } from '$lib/stores/rules.svelte';
  import SourcePickList from './SourcePickList.svelte';

  const isMobile = new IsMobile();
  const uid = $props.id();
  const grantId = `${uid}-network-grant`;
  const summaryId = `${uid}-selected-summary`;
  const surface = $derived(getDeepLinkSurface());

  type InstallPhase = 'pick' | 'grant' | 'working' | 'done';
  type PreparedEntry = { item: CatalogItem; candidate: InstallCandidate };

  let selectedIds = $state<string[]>([]);
  let phase = $state<InstallPhase>('pick');
  let prepared = $state<PreparedEntry[]>([]);
  let grant = $state<CapabilityGrantPreset>('none');
  let error = $state<string | null>(null);
  let successMessage = $state<string | null>(null);

  const open = $derived(
    surface.kind === 'loading' ||
      surface.kind === 'pick' ||
      surface.kind === 'reject' ||
      surface.kind === 'fetch-error' ||
      surface.kind === 'catalog-error',
  );

  const pickItems = $derived(surface.kind === 'pick' ? surface.catalog.items : []);
  const pickTruncated = $derived(surface.kind === 'pick' ? surface.catalog.truncated : false);
  const pickTotal = $derived(surface.kind === 'pick' ? surface.catalog.totalCount : 0);
  const selectedCount = $derived(selectedIds.length);
  const canConfirmSelection = $derived(
    phase === 'pick' &&
      selectedCount > 0 &&
      selectedCount <= CATALOG_INSTALL_CAP &&
      surface.kind === 'pick',
  );

  const needsNetwork = $derived(prepared.some((entry) => entry.candidate.required_grant.network));
  const hasUnsupportedSystem = $derived(
    prepared.some(
      (entry) =>
        entry.candidate.required_grant.system.fs ||
        entry.candidate.required_grant.system.env ||
        entry.candidate.required_grant.system.process,
    ),
  );
  const canInstallPrepared = $derived(
    phase === 'grant' &&
      prepared.length > 0 &&
      !hasUnsupportedSystem &&
      (!needsNetwork || grant === 'network_only'),
  );

  function sheetTitle(): string {
    if (surface.kind === 'reject') return m.sources_deeplink_reject_title();
    if (surface.kind === 'fetch-error' || surface.kind === 'catalog-error') {
      return m.sources_deeplink_error_title();
    }
    if (phase === 'grant') return m.sources_deeplink_grant_title();
    if (phase === 'done') return m.sources_deeplink_done_title();
    return m.sources_deeplink_import_title();
  }

  function rejectMessage(): string {
    if (surface.kind !== 'reject') return '';
    if (surface.reason === 'unsupported-import') {
      return m.sources_deeplink_reject_unsupported({
        type: surface.subject ?? 'unknown',
      });
    }
    if (surface.reason === 'missing-src') return m.sources_deeplink_reject_missing_src();
    if (surface.reason === 'unsupported-scheme') {
      return m.sources_deeplink_reject_scheme({
        scheme: surface.subject ?? 'unknown',
      });
    }
    return m.sources_deeplink_reject_invalid();
  }

  function catalogErrorMessage(): string {
    if (surface.kind !== 'catalog-error') return '';
    if (surface.reason === 'invalid-json') return m.sources_deeplink_catalog_invalid_json();
    if (surface.reason === 'empty') return m.sources_deeplink_catalog_empty();
    return m.sources_deeplink_catalog_not_book_source();
  }

  function handleOpenChange(next: boolean): void {
    if (next) return;
    if (surface.kind === 'pick' || surface.kind === 'loading') {
      completeDeepLinkPick();
    } else {
      dismissDeepLinkSurface();
    }
    phase = 'pick';
    prepared = [];
    selectedIds = [];
    grant = 'none';
    error = null;
    successMessage = null;
  }

  async function prepareSelected(): Promise<void> {
    if (surface.kind !== 'pick' || !canConfirmSelection) return;
    phase = 'working';
    error = null;
    const selected = new Set(selectedIds);
    const chosen = surface.catalog.items.filter((item) => selected.has(item.id));
    const next: PreparedEntry[] = [];
    try {
      for (const item of chosen) {
        const candidate = await prepareInstall(item.rawJson);
        next.push({ item, candidate });
      }
      prepared = next;
      grant = 'none';
      phase = 'grant';
    } catch (caught) {
      error = String(caught);
      prepared = [];
      phase = 'pick';
    }
  }

  async function installPrepared(): Promise<void> {
    if (!canInstallPrepared) return;
    phase = 'working';
    error = null;
    let successCount = 0;
    const failures: string[] = [];
    try {
      for (const entry of prepared) {
        try {
          await installCandidate(entry.candidate.id, grant);
          successCount += 1;
        } catch (caught) {
          failures.push(`${entry.item.name}: ${String(caught)}`);
        }
      }
      if (successCount > 0 && failures.length === 0) {
        successMessage = m.sources_deeplink_install_success({ count: successCount });
        phase = 'done';
        // 短暂展示成功后关闭并处理下一条
        window.setTimeout(() => {
          handleOpenChange(false);
        }, 900);
        return;
      }
      if (successCount > 0) {
        successMessage = m.sources_deeplink_install_partial({
          ok: successCount,
          fail: failures.length,
        });
        error = failures.join('\n');
        phase = 'done';
        return;
      }
      error = failures.join('\n') || m.sources_deeplink_install_failed();
      phase = 'grant';
    } catch (caught) {
      error = String(caught);
      phase = 'grant';
    }
  }
</script>

<Sheet {open} onOpenChange={handleOpenChange}>
  <SheetContent
    side={isMobile.current ? 'bottom' : 'right'}
    class="flex w-full flex-col gap-0 overflow-hidden p-0 data-[side=bottom]:!h-[90dvh] data-[side=bottom]:max-h-[90dvh] data-[side=right]:sm:max-w-2xl data-[side=right]:lg:max-w-4xl"
    data-testid="deeplink-install-host"
  >
    <SheetHeader class="border-b border-hairline pr-12">
      <SheetTitle>{sheetTitle()}</SheetTitle>
      {#if surface.kind === 'pick'}
        <SheetDescription>{m.sources_deeplink_import_hint()}</SheetDescription>
      {/if}
    </SheetHeader>

    <div
      class={[
        'flex min-h-0 flex-1 flex-col gap-3 p-(--density-panel-padding-compact) sm:p-(--density-panel-padding)',
        surface.kind === 'pick' &&
        (phase === 'pick' || (phase === 'working' && prepared.length === 0))
          ? 'overflow-hidden'
          : 'overflow-y-auto',
      ]}
      data-testid="deeplink-install-body"
      aria-busy={phase === 'working' || surface.kind === 'loading'}
    >
      {#if surface.kind === 'loading'}
        <Notice tone="info" role="status" icon="arrow-clockwise">
          {m.sources_deeplink_fetching()}
        </Notice>
      {:else if surface.kind === 'reject'}
        <div data-testid="deeplink-reject">
          <Notice tone="danger" role="alert" icon="warning-circle">
            {rejectMessage()}
          </Notice>
        </div>
      {:else if surface.kind === 'fetch-error'}
        <Notice tone="danger" role="alert" icon="warning-circle">
          <span class="break-words">
            {m.sources_deeplink_fetch_failed({ detail: surface.message })}
          </span>
        </Notice>
      {:else if surface.kind === 'catalog-error'}
        <Notice tone="danger" role="alert" icon="warning-circle">
          {catalogErrorMessage()}
        </Notice>
      {:else if surface.kind === 'pick' && (phase === 'pick' || (phase === 'working' && prepared.length === 0))}
        <SourcePickList
          items={pickItems}
          truncated={pickTruncated}
          totalCount={pickTotal}
          bind:selectedIds
          disabled={phase === 'working'}
        />
        {#if error}
          <Notice tone="danger" role="alert" icon="warning-circle">
            <span class="break-words">{error}</span>
          </Notice>
        {/if}
      {:else if phase === 'grant' || phase === 'working' || phase === 'done'}
        <div class="grid min-h-0 gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(16rem,0.8fr)] lg:gap-0">
          <section class="min-w-0 lg:pr-4" aria-labelledby={summaryId}>
            <h3 id={summaryId} class="text-sm font-semibold text-ink">
              {m.sources_deeplink_selected_summary({ count: prepared.length })}
            </h3>
            <ul
              class="mt-2 max-h-52 divide-y divide-hairline overflow-y-auto text-sm text-ink-muted"
            >
              {#each prepared as entry (entry.item.id)}
                <li class="min-w-0 py-1.5 break-words">{entry.item.name}</li>
              {/each}
            </ul>
          </section>

          <section
            class="min-w-0 border-t border-hairline pt-3 lg:border-t-0 lg:border-l lg:pt-0 lg:pl-4"
          >
            {#if hasUnsupportedSystem}
              <Notice tone="danger" role="alert" icon="warning-circle">
                {m.sources_install_system_unsupported()}
              </Notice>
            {:else if needsNetwork}
              <Notice tone="info" role="note" icon="warning-circle">
                {m.sources_deeplink_network_required_notice()}
              </Notice>
              <label class="mt-3 flex flex-col gap-1.5 text-sm text-ink" for={grantId}>
                <span class="text-xs font-medium text-ink-muted">
                  {m.sources_install_network_grant()}
                </span>
                <select
                  id={grantId}
                  bind:value={grant}
                  disabled={phase === 'working'}
                  class="glass-control h-(--density-control-md) w-full rounded-md border border-hairline px-2.5 text-sm text-ink outline-none focus-visible:border-lantern-strong/60 focus-visible:shadow-[var(--focus-ring)]"
                  data-testid="deeplink-network-grant"
                >
                  <option value="none">{m.sources_install_grant_prompt()}</option>
                  <option value="network_only">{m.sources_install_grant_network_only()}</option>
                </select>
              </label>
            {:else}
              <p class="text-xs text-ink-muted">{m.sources_install_network_not_required()}</p>
            {/if}
          </section>
        </div>

        {#if error}
          <Notice tone="danger" role="alert" icon="warning-circle">
            <span class="break-words whitespace-pre-wrap">{error}</span>
          </Notice>
        {/if}
        {#if successMessage}
          <Notice tone="success" role="status" icon="check-circle">
            {successMessage}
          </Notice>
        {/if}
      {/if}
    </div>

    <SheetFooter
      class="bg-surface-1 px-(--density-panel-padding-compact) py-2.5 pb-[max(var(--density-panel-padding-compact),var(--safe-area-bottom))] sm:px-(--density-panel-padding)"
    >
      {#if surface.kind === 'pick' && phase === 'pick'}
        <div class="flex w-full flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
          <p class="text-sm text-ink-muted" data-testid="deeplink-selected-count">
            {m.sources_deeplink_selected_count({ count: selectedCount })}
          </p>
          <Button
            type="button"
            class="w-full sm:w-auto"
            disabled={!canConfirmSelection}
            onclick={() => void prepareSelected()}
            data-testid="deeplink-install-selected"
          >
            {m.sources_deeplink_install_selected()}
          </Button>
        </div>
      {:else if phase === 'grant'}
        <div class="flex w-full flex-col gap-2 sm:flex-row sm:justify-end">
          <Button
            type="button"
            variant="outline"
            class="w-full sm:w-auto"
            onclick={() => {
              phase = 'pick';
              prepared = [];
              grant = 'none';
              error = null;
            }}
          >
            {m.sources_deeplink_back_to_pick()}
          </Button>
          <Button
            type="button"
            class="w-full sm:w-auto"
            disabled={!canInstallPrepared}
            onclick={() => void installPrepared()}
            data-testid="deeplink-confirm-install"
          >
            {m.sources_install_action()}
          </Button>
        </div>
      {:else if surface.kind === 'reject' || surface.kind === 'fetch-error' || surface.kind === 'catalog-error' || phase === 'done'}
        <Button
          type="button"
          class="w-full sm:ml-auto sm:w-auto"
          onclick={() => handleOpenChange(false)}
        >
          {m.action_close()}
        </Button>
      {:else if phase === 'working' || surface.kind === 'loading'}
        <p class="w-full text-sm text-ink-muted" role="status">
          {surface.kind === 'loading'
            ? m.sources_deeplink_fetching()
            : prepared.length === 0
              ? m.sources_install_validating()
              : m.sources_install_installing()}
        </p>
      {/if}
    </SheetFooter>
  </SheetContent>
</Sheet>
