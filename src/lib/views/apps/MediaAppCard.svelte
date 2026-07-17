<script lang="ts">
  import { resolve } from '$app/paths';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import { MediaAppIcon } from '$lib/components/brand';
  import { Button } from '$lib/components/ui/button';
  import { m } from '$lib/i18n';
  import type { MediaAppCardState } from '$lib/app/shell-types';

  type Props = {
    app: MediaAppCardState;
    lead?: boolean;
    selected?: boolean;
    onselect?: (app: MediaAppCardState) => void;
  };

  let { app, lead = false, selected = false, onselect }: Props = $props();
  const statusLabels: Record<MediaAppCardState['status'], string> = {
    unconnected: m.app_status_unconnected(),
    explorable: m.app_status_explorable(),
    'has-content': m.app_status_has_content(),
    failed: m.app_status_failed(),
  };
</script>

<article
  class={[
    'group/card motion-dock-wake relative overflow-hidden rounded-xl border border-hairline bg-surface-1 p-3 transition-colors',
    lead && 'sm:col-span-2',
    selected && 'border-lantern/50 bg-lantern-soft/30',
  ]}
  data-lead={lead}
>
  <div class="relative flex h-full flex-col gap-2">
    <div class="flex items-start justify-between gap-3">
      <MediaAppIcon name={app.key} size={lead ? 40 : 32} active={app.status !== 'unconnected'} />
      <span
        class="rounded-full border border-hairline bg-surface-2 px-2 py-0.5 text-[0.68rem] font-medium text-ink-muted"
      >
        {statusLabels[app.status]}
      </span>
    </div>

    <h2
      class={['font-semibold tracking-tight text-ink', lead ? 'text-base sm:text-lg' : 'text-sm']}
    >
      {app.label}
    </h2>
    <p class="max-w-prose text-xs leading-5 text-ink-muted">{app.description}</p>

    <div class="mt-auto pt-2">
      {#if app.href}
        <a
          href={resolve(app.href as '/')}
          class="motion-nav-capsule inline-flex min-h-11 items-center gap-2 rounded-full bg-primary px-4 text-sm font-semibold text-primary-foreground hover:bg-primary/90"
        >
          {app.primaryAction}
          <ArrowRight size={16} aria-hidden="true" />
        </a>
      {:else}
        <Button
          type="button"
          variant="outline"
          class="motion-nav-capsule min-h-11 rounded-full bg-background/70 px-4 font-semibold"
          onclick={() => onselect?.(app)}
        >
          {app.primaryAction}
          <ArrowRight size={16} aria-hidden="true" />
        </Button>
      {/if}
    </div>
  </div>
</article>
