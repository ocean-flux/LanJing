<script lang="ts">
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
  const surface = $derived(getDeepLinkSurface());

  let open = $state(false);
  let selectedIds = $state<string[]>([]);
  let phase = $state<'pick' | 'grant' | 'working' | 'done'>('pick');
  let prepared = $state<Array<{ item: CatalogItem; candidate: InstallCandidate }>>([]);
  let grant = $state<CapabilityGrantPreset>('none');
  let error = $state<string | null>(null);
  let successMessage = $state<string | null>(null);

  const isInteractive = $derived(
    surface.kind === 'loading' ||
      surface.kind === 'pick' ||
      surface.kind === 'reject' ||
      surface.kind === 'fetch-error' ||
      surface.kind === 'catalog-error',
  );

  $effect(() => {
    open = isInteractive;
    if (surface.kind === 'pick') {
      selectedIds = [];
      phase = 'pick';
      prepared = [];
      grant = 'none';
      error = null;
      successMessage = null;
    } else if (surface.kind === 'loading') {
      phase = 'pick';
      error = null;
      successMessage = null;
    }
  });

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
    if (next) {
      open = true;
      return;
    }
    open = false;
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
    const next: Array<{ item: CatalogItem; candidate: InstallCandidate }> = [];
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
    class="flex w-full flex-col gap-0 overflow-hidden p-0 data-[side=bottom]:!h-[90dvh] data-[side=bottom]:max-h-[90dvh] data-[side=right]:sm:max-w-xl data-[side=right]:lg:max-w-2xl"
    data-testid="deeplink-install-host"
  >
    <SheetHeader class="border-b border-hairline pr-12">
      <SheetTitle>{sheetTitle()}</SheetTitle>
      {#if surface.kind === 'pick'}
        <SheetDescription>{m.sources_deeplink_import_hint()}</SheetDescription>
      {/if}
    </SheetHeader>

    <div
      class="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden px-4 py-4 sm:px-5"
      data-testid="deeplink-install-body"
    >
      {#if surface.kind === 'loading'}
        <p class="text-sm text-ink-muted" role="status">{m.sources_deeplink_fetching()}</p>
      {:else if surface.kind === 'reject'}
        <div
          class="border-danger/35 bg-danger/10 text-danger rounded-xl border px-4 py-3 text-sm"
          role="alert"
          data-testid="deeplink-reject"
        >
          {rejectMessage()}
        </div>
      {:else if surface.kind === 'fetch-error'}
        <div
          class="border-danger/35 bg-danger/10 text-danger rounded-xl border px-4 py-3 text-sm break-words"
          role="alert"
        >
          {m.sources_deeplink_fetch_failed({ detail: surface.message })}
        </div>
      {:else if surface.kind === 'catalog-error'}
        <div
          class="border-danger/35 bg-danger/10 text-danger rounded-xl border px-4 py-3 text-sm"
          role="alert"
        >
          {catalogErrorMessage()}
        </div>
      {:else if surface.kind === 'pick' && (phase === 'pick' || phase === 'working')}
        <SourcePickList
          items={pickItems}
          truncated={pickTruncated}
          totalCount={pickTotal}
          bind:selectedIds
          disabled={phase === 'working'}
        />
        {#if error}
          <div
            class="border-danger/35 bg-danger/10 text-danger rounded-lg border px-3 py-2 text-sm break-words"
            role="alert"
          >
            {error}
          </div>
        {/if}
      {:else if phase === 'grant' || phase === 'working' || phase === 'done'}
        <div class="glass-panel space-y-3 rounded-xl border border-hairline p-4">
          <p class="text-sm font-semibold text-ink">
            {m.sources_deeplink_selected_summary({ count: prepared.length })}
          </p>
          <ul class="max-h-40 space-y-1 overflow-y-auto text-sm text-ink-muted">
            {#each prepared as entry (entry.item.id)}
              <li class="truncate">{entry.item.name}</li>
            {/each}
          </ul>
          {#if hasUnsupportedSystem}
            <p
              class="border-danger/35 bg-danger/10 text-danger rounded-lg border px-3 py-2 text-xs"
            >
              {m.sources_install_system_unsupported()}
            </p>
          {:else if needsNetwork}
            <p
              class="rounded-lg border border-lantern/35 bg-lantern-soft/25 px-3 py-2 text-xs font-medium text-ink"
            >
              {m.sources_deeplink_network_required_notice()}
            </p>
            <label class="flex flex-col gap-1.5 text-sm text-ink" for="deeplink-network-grant">
              <span class="text-xs font-medium text-ink-muted"
                >{m.sources_install_network_grant()}</span
              >
              <select
                id="deeplink-network-grant"
                bind:value={grant}
                disabled={phase === 'working'}
                class="glass-control min-h-11 w-full rounded-lg border border-hairline px-2.5 text-sm text-ink outline-none focus-visible:border-lantern-strong/50 focus-visible:shadow-[var(--focus-ring)]"
                data-testid="deeplink-network-grant"
              >
                <option value="none">{m.sources_install_grant_prompt()}</option>
                <option value="network_only">{m.sources_install_grant_network_only()}</option>
              </select>
            </label>
          {:else}
            <p class="text-xs text-ink-muted">{m.sources_install_network_not_required()}</p>
          {/if}
        </div>
        {#if error}
          <div
            class="border-danger/35 bg-danger/10 text-danger rounded-lg border px-3 py-2 text-sm break-words whitespace-pre-wrap"
            role="alert"
          >
            {error}
          </div>
        {/if}
        {#if successMessage}
          <div
            class="rounded-lg border border-positive/40 bg-positive/10 px-3 py-2.5 text-sm text-positive"
            role="status"
          >
            {successMessage}
          </div>
        {/if}
      {/if}
    </div>

    <SheetFooter
      class="sticky bottom-0 z-10 border-t border-hairline bg-surface-1/95 px-4 py-3 backdrop-blur-sm sm:px-5"
    >
      {#if surface.kind === 'pick' && phase === 'pick'}
        <div class="flex w-full flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
          <p class="text-sm text-ink-muted" data-testid="deeplink-selected-count">
            {m.sources_deeplink_selected_count({ count: selectedCount })}
          </p>
          <Button
            type="button"
            class="min-h-11 w-full active:scale-[0.98] sm:w-auto"
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
            class="min-h-11 w-full sm:w-auto"
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
            class="min-h-11 w-full active:scale-[0.98] sm:w-auto"
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
          class="min-h-11 w-full sm:w-auto"
          onclick={() => handleOpenChange(false)}
        >
          {m.action_close()}
        </Button>
      {:else if phase === 'working' || surface.kind === 'loading'}
        <p class="w-full text-sm text-ink-muted" role="status">
          {surface.kind === 'loading'
            ? m.sources_deeplink_fetching()
            : m.sources_install_installing()}
        </p>
      {/if}
      <div class="h-[env(safe-area-inset-bottom)] lg:hidden" aria-hidden="true"></div>
    </SheetFooter>
  </SheetContent>
</Sheet>
