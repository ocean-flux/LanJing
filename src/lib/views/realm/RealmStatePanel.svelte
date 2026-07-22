<script lang="ts">
  import { resolve } from '$app/paths';
  import Activity from '@lucide/svelte/icons/activity';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import FolderPlus from '@lucide/svelte/icons/folder-plus';
  import Search from '@lucide/svelte/icons/search';
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';
  import { LanJingMark } from '$lib/components/brand';
  import { m } from '$lib/i18n';
  import type { RealmAction, RealmState, SourceCardState } from '$lib/app/shell-types';

  type Props = {
    state: RealmState;
    sources?: SourceCardState[];
  };

  let { state, sources = [] }: Props = $props();

  const sourceWarnings = $derived(
    sources.filter((source) => source.status === 'failed' || source.status === 'partial'),
  );

  const actions = $derived.by((): RealmAction[] => {
    switch (state.kind) {
      case 'no-source':
        return [state.primaryAction, state.secondaryAction];
      case 'source-no-resource':
        return [
          state.primaryAction,
          { kind: 'open-discover', label: m.action_open_discover(), href: '/apps' },
          { kind: 'check-source', label: m.action_check_source(), href: '/sources' },
          state.secondaryAction,
        ];
      case 'source-warning':
        return [
          state.primaryAction,
          { kind: 'retry', label: m.action_retry(), href: '/sources' },
          state.secondaryAction,
        ];
      case 'has-content':
        return [state.primaryAction, state.secondaryAction];
    }
  });

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

<!-- 境场：单 hero 空态；有源/告警时才展开状态面，避免文案与 CTA 重复堆砌 -->
<section class="flex w-full flex-col gap-5" data-testid="realm-state-panel">
  <header
    class="realm-hero double-bezel relative overflow-hidden p-5 sm:p-7 md:p-8"
    data-testid="realm-hero"
  >
    <div class="realm-hero-glow pointer-events-none absolute inset-0" aria-hidden="true"></div>
    <div class="relative z-[1] flex max-w-2xl flex-col gap-5">
      <div class="flex flex-wrap items-center gap-3">
        <span
          class="grid h-11 w-11 place-items-center rounded-2xl border border-hairline bg-surface-2/80 shadow-[var(--surface-panel-shadow)]"
        >
          <LanJingMark width={28} height={20} label="LanJing" />
        </span>
        <div class="min-w-0">
          <p class="text-[0.7rem] font-medium tracking-[0.14em] text-ink-subtle uppercase">
            {m.realm_brand()}
          </p>
          <p class="mt-0.5 text-sm text-ink-muted">{m.realm_offline_media_realm()}</p>
        </div>
        {#if state.sourceSummary}
          <div
            class="ms-auto inline-flex items-center gap-1.5 rounded-full border border-hairline bg-surface-2/90 px-3 py-1 text-xs text-ink-muted"
          >
            <Activity size={14} aria-hidden="true" />
            {state.sourceSummary}
          </div>
        {/if}
      </div>

      <div class="min-w-0">
        <h1
          class="text-2xl font-semibold tracking-tight text-ink sm:text-3xl md:text-[2rem] md:leading-tight"
        >
          {state.title}
        </h1>
        <p class="mt-3 max-w-xl text-sm leading-6 text-ink-muted sm:text-[0.95rem]">
          {state.description}
        </p>
        <p class="mt-2 text-xs text-ink-subtle">{m.realm_no_fake_shelves()}</p>
      </div>

      <div class="flex flex-wrap gap-2.5">
        {#each actions as action, index (action.kind)}
          <a
            href={resolve(action.href as '/')}
            class={[
              'motion-dock-wake inline-flex min-h-11 items-center gap-2 rounded-full px-4 text-sm font-semibold outline-none transition-[transform,background-color,box-shadow] focus-visible:shadow-[var(--focus-ring)] active:scale-[0.98]',
              index === 0
                ? 'lantern-action'
                : 'border border-hairline-strong bg-surface-2/80 text-ink hover:bg-surface-3',
            ]}
          >
            {#if action.kind === 'search-content'}
              <Search size={16} aria-hidden="true" />
            {:else if action.kind === 'import-local' || action.kind === 'add-source'}
              <FolderPlus size={16} aria-hidden="true" />
            {:else if action.kind === 'view-source-status'}
              <ShieldAlert size={16} aria-hidden="true" />
            {:else}
              <ArrowRight size={16} aria-hidden="true" />
            {/if}
            {action.label}
          </a>
        {/each}
      </div>
    </div>
  </header>

  {#if state.kind === 'source-no-resource' || sources.length > 0}
    <div class="grid gap-3 lg:grid-cols-2">
      {#if state.kind === 'source-no-resource'}
        <section class="double-bezel p-4" aria-labelledby="realm-media-waiting">
          <h2 id="realm-media-waiting" class="text-sm font-semibold text-ink">
            {m.realm_media_waiting()}
          </h2>
          <p class="mt-1 text-xs leading-5 text-ink-muted">{m.realm_no_fake_shelves()}</p>
        </section>
      {/if}

      {#if sources.length > 0}
        <section
          class={['double-bezel p-4', state.kind === 'no-source' && 'lg:col-span-2']}
          aria-labelledby="realm-source-status"
        >
          <h2 id="realm-source-status" class="text-sm font-semibold text-ink">
            {m.realm_source_status_hint()}
          </h2>

          {#if sourceWarnings.length > 0}
            <div class="mt-3 grid gap-1.5">
              {#each sourceWarnings as source (source.id)}
                <div class="rounded-xl border border-hairline bg-surface-2/80 px-3 py-2.5 text-sm">
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
          {:else}
            <div class="mt-3 grid gap-1">
              {#each sources.slice(0, 3) as source (source.id)}
                <div
                  class="flex items-center justify-between gap-3 border-b border-hairline px-1 py-2.5 text-sm last:border-b-0"
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
      {/if}
    </div>
  {/if}
</section>

<style>
  .realm-hero {
    min-height: min(42dvh, 22rem);
  }

  .realm-hero-glow {
    background:
      radial-gradient(
        80% 70% at 0% 0%,
        color-mix(in oklab, var(--lantern) 18%, transparent),
        transparent 55%
      ),
      radial-gradient(
        60% 50% at 100% 100%,
        color-mix(in oklab, var(--lantern-soft) 70%, transparent),
        transparent 50%
      );
  }
</style>
