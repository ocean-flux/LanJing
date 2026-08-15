<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from '$lib/components/Icon.svelte';
  import { cn } from '$lib/utils.js';

  type Props = {
    title: string;
    description?: string;
    icon?: IconName;
    action?: Snippet;
    role?: 'status' | 'note';
    class?: string;
  };

  let { title, description, icon, action, role = 'status', class: className }: Props = $props();

  const uid = $props.id();
  const titleId = `${uid}-title`;
</script>

<section
  data-slot="empty-state"
  {role}
  aria-labelledby={titleId}
  class={cn(
    'glass-panel flex flex-col items-start rounded-[var(--radius-panel)] border border-hairline p-(--density-panel-padding-compact) sm:p-(--density-panel-padding)',
    className,
  )}
>
  {#if icon}
    <Icon name={icon} class="size-6 text-lantern-strong" />
  {/if}
  <h2 id={titleId} class={cn('text-base font-semibold text-ink', icon && 'mt-3')}>{title}</h2>
  {#if description}
    <p class="mt-1 max-w-[65ch] text-sm leading-5 text-ink-muted">{description}</p>
  {/if}
  {#if action}
    <div class="mt-4 flex flex-wrap items-center gap-2">
      {@render action()}
    </div>
  {/if}
</section>
