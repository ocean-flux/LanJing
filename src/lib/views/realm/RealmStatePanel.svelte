<script lang="ts">
  import { resolve } from '$app/paths';
  import Activity from '@lucide/svelte/icons/activity';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import FolderPlus from '@lucide/svelte/icons/folder-plus';
  import Search from '@lucide/svelte/icons/search';
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';
  import { LanJingMark } from '$lib/components/brand';
  import { m } from '$lib/i18n';
  import type { RealmState, SourceCardState } from '$lib/app/shell-types';

  type Props = {
    state: RealmState;
    sources?: SourceCardState[];
  };

  let { state, sources = [] }: Props = $props();

  const sourceWarnings = $derived(
    sources.filter((source) => source.status === 'failed' || source.status === 'partial'),
  );

  const actions = $derived.by(() => {
    if (state.kind === 'source-no-resource') {
      return [
        state.primaryAction,
        m.action_open_discover(),
        m.action_check_source(),
        state.secondaryAction,
      ];
    }

    if (state.kind === 'source-warning') {
      return [state.primaryAction, m.action_retry(), state.secondaryAction];
    }

    return [state.primaryAction, state.secondaryAction];
  });

  function actionPath(action: string) {
    if (
      action === m.action_add_source() ||
      action === m.action_view_source_status() ||
      action === m.action_check_source() ||
      action === m.action_retry()
    ) {
      return '/sources';
    }

    if (action === m.action_import_local()) {
      return '/library';
    }

    return '/apps';
  }

  function sourceStatusLabel(status: SourceCardState['status']) {
    if (status === 'ready') {
      return m.status_ready();
    }

    if (status === 'partial') {
      return m.status_partial();
    }

    if (status === 'failed') {
      return m.status_failed();
    }

    if (status === 'disabled') {
      return m.status_disabled();
    }

    return m.status_unchecked();
  }
</script>

<!-- 全宽 denselist 节奏：无 560 高 marketing 卡、无双栏卡堆 -->
<section class="flex w-full flex-col gap-3" data-testid="realm-state-panel">
  <header class="flex flex-wrap items-start justify-between gap-3 border-b border-hairline pb-2">
    <div class="min-w-0">
      <div class="flex items-center gap-2 text-ink-muted">
        <span
          class="grid h-8 w-8 place-items-center rounded-lg border border-hairline bg-surface-1"
        >
          <LanJingMark width={24} height={18} label="LanJing" />
        </span>
        <span class="text-xs font-medium tracking-wide">{m.realm_brand()}</span>
      </div>
      <p class="mt-2 text-xs font-medium text-ink-muted">{m.realm_offline_media_realm()}</p>
      <h1 class="mt-1 text-base font-semibold tracking-tight text-ink sm:text-lg">
        {state.title}
      </h1>
      <p class="mt-1 max-w-prose text-xs leading-5 text-ink-muted sm:text-sm">
        {state.description}
      </p>
    </div>
    {#if state.sourceSummary}
      <div
        class="inline-flex items-center gap-1.5 rounded-full border border-hairline bg-surface-1 px-2.5 py-1 text-xs text-ink-muted"
      >
        <Activity size={14} aria-hidden="true" />
        {state.sourceSummary}
      </div>
    {/if}
  </header>

  <div class="flex flex-wrap gap-2">
    {#each actions as action, index (action)}
      <a
        href={resolve(actionPath(action) as '/')}
        class={[
          'motion-dock-wake inline-flex min-h-10 items-center gap-2 rounded-lg px-3 text-sm font-semibold',
          index === 0
            ? 'lantern-action'
            : 'border border-hairline bg-surface-1 text-ink hover:bg-surface-3',
        ]}
      >
        {#if action === m.action_search_content()}
          <Search size={15} aria-hidden="true" />
        {:else if action === m.action_import_local() || action === m.action_add_source()}
          <FolderPlus size={15} aria-hidden="true" />
        {:else if action === m.action_view_source_status()}
          <ShieldAlert size={15} aria-hidden="true" />
        {:else}
          <ArrowRight size={15} aria-hidden="true" />
        {/if}
        {action}
      </a>
    {/each}
  </div>

  <p class="text-xs text-ink-subtle">{m.realm_no_fake_shelves()}</p>

  <div class="grid gap-3 lg:grid-cols-2">
    <section class="border-t border-hairline pt-2" aria-labelledby="realm-media-waiting">
      <h2 id="realm-media-waiting" class="text-sm font-semibold text-ink">
        {m.realm_media_waiting()}
      </h2>
      <p class="mt-1 text-xs leading-5 text-ink-muted">{m.realm_no_fake_shelves()}</p>
      <div class="mt-2 grid gap-1">
        {#each actions as action, index (action)}
          <a
            href={resolve(actionPath(action) as '/')}
            class={[
              'motion-nav-capsule flex items-start gap-2 rounded-lg px-2 py-2 text-left outline-none hover:bg-lantern-soft focus-visible:shadow-[var(--focus-ring)]',
              index === 0 && 'bg-lantern-soft/40',
            ]}
          >
            <span
              class="grid h-8 w-8 shrink-0 place-items-center rounded-md border border-hairline bg-surface-2 text-ink"
            >
              {#if action === m.action_search_content()}
                <Search size={14} aria-hidden="true" />
              {:else if action === m.action_import_local() || action === m.action_add_source()}
                <FolderPlus size={14} aria-hidden="true" />
              {:else if action === m.action_view_source_status()}
                <ShieldAlert size={14} aria-hidden="true" />
              {:else}
                <ArrowRight size={14} aria-hidden="true" />
              {/if}
            </span>
            <span class="min-w-0">
              <span class="block text-sm font-medium text-ink">{action}</span>
              <span class="mt-0.5 block text-xs leading-5 text-ink-subtle">
                {m.realm_no_fake_shelves()}
              </span>
            </span>
          </a>
        {/each}
      </div>
    </section>

    <section class="border-t border-hairline pt-2" aria-labelledby="realm-source-status">
      <h2 id="realm-source-status" class="text-sm font-semibold text-ink">
        {m.realm_source_status_hint()}
      </h2>
      <p class="mt-1 text-xs leading-5 text-ink-muted">{m.realm_media_waiting()}</p>

      {#if sourceWarnings.length > 0}
        <div class="mt-2 grid gap-1.5">
          {#each sourceWarnings as source (source.id)}
            <div class="rounded-lg border border-hairline px-3 py-2 text-sm">
              <div class="flex items-start justify-between gap-3">
                <span class="block font-medium text-ink">{source.name}</span>
                <span
                  class={[
                    'rounded-full px-2 py-0.5 text-[0.68rem] font-medium',
                    source.status === 'failed'
                      ? 'bg-warning/15 text-warning'
                      : 'bg-warning/10 text-warning',
                  ]}
                >
                  {sourceStatusLabel(source.status)}
                </span>
              </div>
              <span class="mt-1 block text-xs text-ink-muted">{source.summary}</span>
            </div>
          {/each}
        </div>
      {:else if sources.length === 0}
        <div class="media-void mt-2 rounded-lg px-3 py-3 text-sm">
          <p class="font-medium text-ink">{m.realm_title_no_source()}</p>
          <p class="mt-1 text-xs text-ink-muted">{m.realm_desc_no_source()}</p>
        </div>
      {:else}
        <div class="mt-2 grid gap-1">
          {#each sources.slice(0, 3) as source (source.id)}
            <div
              class="flex items-center justify-between gap-3 border-b border-hairline px-1 py-2 text-sm last:border-b-0"
            >
              <span class="min-w-0">
                <span class="block font-medium text-ink">{source.name}</span>
                <span class="mt-0.5 block text-xs text-ink-subtle">{source.summary}</span>
              </span>
              <span
                class={[
                  'rounded-full px-2 py-0.5 text-[0.68rem] font-medium',
                  source.status === 'ready'
                    ? 'bg-positive/15 text-positive'
                    : 'bg-surface-2 text-ink-muted',
                ]}
              >
                {sourceStatusLabel(source.status)}
              </span>
            </div>
          {/each}
        </div>
      {/if}
    </section>
  </div>
</section>
