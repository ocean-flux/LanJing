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
    'surface-panel group/card motion-dock-wake relative overflow-hidden p-5 transition-colors',
    lead && 'lg:col-span-2 lg:min-h-80',
    selected && 'border-primary/60 bg-primary/10',
  ]}
  data-lead={lead}
>
  <div class="relative flex h-full flex-col">
    <div class="flex items-start justify-between gap-4">
      <MediaAppIcon name={app.key} size={lead ? 58 : 44} active={app.status !== 'unconnected'} />
      <span
        class="surface-control rounded-full px-3 py-1 text-xs font-semibold text-muted-foreground"
      >
        {statusLabels[app.status]}
      </span>
    </div>

    <h2 class={['mt-5 font-semibold tracking-tight', lead ? 'text-3xl md:text-4xl' : 'text-2xl']}>
      {app.label}
    </h2>
    <p class="mt-3 max-w-xl text-sm leading-6 text-muted-foreground">{app.description}</p>

    <div class="mt-auto pt-5">
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
