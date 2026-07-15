<script lang="ts">
  import { resolve } from '$app/paths';
  import { Activity, ArrowRight, FolderPlus, Search, ShieldAlert } from '@lucide/svelte';
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

<section
  class="grid min-h-full gap-4 py-4 lg:grid-cols-[minmax(0,1fr)_340px] xl:grid-cols-[minmax(0,1fr)_390px]"
>
  <div class="surface-panel media-void relative min-h-[560px] overflow-hidden p-5 md:p-8">
    <div class="relative flex min-h-[500px] flex-col justify-between gap-10">
      <div class="flex items-center justify-between gap-4">
        <div class="inline-flex items-center gap-3 text-muted-foreground">
          <span
            class="grid h-10 w-10 place-items-center rounded-xl border border-border bg-surface-1"
          >
            <LanJingMark width={30} height={23} label="LanJing" />
          </span>
          <span class="text-sm font-medium tracking-wide">{m.realm_brand()}</span>
        </div>

        {#if state.sourceSummary}
          <div
            class="inline-flex items-center gap-2 rounded-full border border-border bg-surface-1 px-3 py-2 text-xs text-muted-foreground"
          >
            <Activity size={14} aria-hidden="true" />
            {state.sourceSummary}
          </div>
        {/if}
      </div>

      <div class="max-w-5xl py-10 md:py-16">
        <p class="text-sm font-semibold text-muted-foreground">{m.realm_offline_media_realm()}</p>
        <h1
          class="mt-4 max-w-4xl text-balance text-5xl font-semibold tracking-[-0.055em] text-foreground md:text-7xl xl:text-8xl"
        >
          {state.title}
        </h1>
        <p class="mt-6 max-w-2xl text-base leading-7 text-muted-foreground md:text-lg">
          {state.description}
        </p>

        <div class="mt-10 flex flex-wrap gap-3">
          {#each actions as action, index (action)}
            <a
              href={resolve(actionPath(action) as '/')}
              class={[
                'motion-dock-wake inline-flex min-h-11 items-center gap-2 rounded-lg px-5 py-3 text-sm font-semibold',
                index === 0
                  ? 'lantern-action'
                  : 'border border-border bg-surface-1 text-foreground hover:bg-accent',
              ]}
            >
              {#if action === m.action_search_content()}
                <Search size={16} aria-hidden="true" />
              {:else if action === m.action_import_local() || action === m.action_add_source()}
                <FolderPlus size={16} aria-hidden="true" />
              {:else if action === m.action_view_source_status()}
                <ShieldAlert size={16} aria-hidden="true" />
              {:else}
                <ArrowRight size={16} aria-hidden="true" />
              {/if}
              {action}
            </a>
          {/each}
        </div>
      </div>

      <p class="max-w-xl text-sm text-muted-foreground">{m.realm_no_fake_shelves()}</p>
    </div>
  </div>

  <aside class="grid content-start gap-4">
    <div class="surface-panel p-5">
      <h2 class="text-sm font-semibold">{m.realm_media_waiting()}</h2>
      <p class="mt-2 text-xs leading-5 text-muted-foreground">{m.realm_no_fake_shelves()}</p>

      <div class="mt-5 grid gap-3">
        {#each actions as action, index (action)}
          <a
            href={resolve(actionPath(action) as '/')}
            class={[
              'motion-nav-capsule flex items-start gap-4 rounded-2xl border border-border bg-background/45 p-4 text-left',
              index === 0 ? 'ring-1 ring-inset ring-lantern/25' : 'hover:bg-accent',
            ]}
          >
            <span
              class="grid h-10 w-10 shrink-0 place-items-center rounded-xl border border-border bg-surface-2 text-foreground"
            >
              {#if action === m.action_search_content()}
                <Search size={16} aria-hidden="true" />
              {:else if action === m.action_import_local() || action === m.action_add_source()}
                <FolderPlus size={16} aria-hidden="true" />
              {:else if action === m.action_view_source_status()}
                <ShieldAlert size={16} aria-hidden="true" />
              {:else}
                <ArrowRight size={16} aria-hidden="true" />
              {/if}
            </span>
            <span class="min-w-0">
              <span class="block text-sm font-semibold">{action}</span>
              <span class="mt-1 block text-xs leading-5 text-muted-foreground">
                {m.realm_no_fake_shelves()}
              </span>
            </span>
          </a>
        {/each}
      </div>
    </div>

    <div class="surface-panel p-5">
      <h2 class="text-sm font-semibold">{m.realm_source_status_hint()}</h2>
      <p class="mt-2 text-xs leading-5 text-muted-foreground">{m.realm_media_waiting()}</p>

      {#if sourceWarnings.length > 0}
        <div class="mt-4 grid gap-3">
          {#each sourceWarnings as source (source.id)}
            <div class="rounded-xl border border-border bg-background/45 p-3 text-sm">
              <div class="flex items-start justify-between gap-3">
                <span class="block font-semibold">{source.name}</span>
                <span
                  class={[
                    'rounded-full px-2 py-1 text-[0.68rem] font-medium',
                    source.status === 'failed'
                      ? 'bg-warning/15 text-warning'
                      : 'bg-warning/10 text-warning',
                  ]}
                >
                  {sourceStatusLabel(source.status)}
                </span>
              </div>
              <span class="mt-1 block text-muted-foreground">{source.summary}</span>
            </div>
          {/each}
        </div>
      {:else if sources.length === 0}
        <div class="media-void mt-4 rounded-2xl p-4 text-sm">
          <p class="font-semibold text-foreground">{m.realm_title_no_source()}</p>
          <p class="mt-1 text-muted-foreground">{m.realm_desc_no_source()}</p>
        </div>
      {:else}
        <div class="mt-4 grid gap-2">
          {#each sources.slice(0, 3) as source (source.id)}
            <div
              class="flex items-center justify-between gap-4 rounded-xl border border-border bg-background/45 px-3 py-2 text-sm"
            >
              <span class="min-w-0">
                <span class="block font-semibold text-foreground">{source.name}</span>
                <span class="mt-0.5 block text-xs text-muted-foreground">{source.summary}</span>
              </span>
              <span
                class={[
                  'rounded-full px-2 py-1 text-[0.68rem] font-medium',
                  source.status === 'ready'
                    ? 'bg-positive/15 text-positive'
                    : 'bg-surface-2 text-muted-foreground',
                ]}
              >
                {sourceStatusLabel(source.status)}
              </span>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </aside>
</section>
