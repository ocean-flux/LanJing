<script lang="ts" module>
  export type NoticeTone = 'info' | 'success' | 'warning' | 'danger';
  export type NoticeRole = 'status' | 'alert' | 'note';

  const toneClasses: Record<NoticeTone, string> = {
    info: 'border-lantern-strong/30 bg-lantern-soft/60',
    success: 'border-positive/35 bg-positive/10',
    warning: 'border-warning/35 bg-warning/10',
    danger: 'border-destructive/35 bg-destructive/10',
  };

  const iconClasses: Record<NoticeTone, string> = {
    info: 'text-lantern-strong',
    success: 'text-positive',
    warning: 'text-warning',
    danger: 'text-destructive',
  };
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from '$lib/components/Icon.svelte';
  import { cn } from '$lib/utils.js';

  type Props = {
    tone: NoticeTone;
    role: NoticeRole;
    title?: string;
    icon?: IconName;
    action?: Snippet;
    class?: string;
    children: Snippet;
  };

  let { tone, role, title, icon, action, class: className, children }: Props = $props();
</script>

<div
  data-slot="notice"
  data-tone={tone}
  {role}
  class={cn(
    'flex items-start gap-2.5 rounded-[var(--radius-panel)] border p-(--density-panel-padding-compact) text-sm text-ink',
    toneClasses[tone],
    className,
  )}
>
  {#if icon}
    <Icon name={icon} class={cn('mt-0.5 size-4 shrink-0', iconClasses[tone])} />
  {/if}
  <div class="min-w-0 flex-1">
    {#if title}
      <p class="leading-5 font-semibold">{title}</p>
    {/if}
    <div class={cn('leading-5 text-ink-muted', title && 'mt-0.5')}>
      {@render children()}
    </div>
    {#if action}
      <div class="mt-3 flex flex-wrap items-center gap-2">
        {@render action()}
      </div>
    {/if}
  </div>
</div>
