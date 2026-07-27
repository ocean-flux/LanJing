<script lang="ts">
  import { resolve } from '$app/paths';
  import Icon from '$lib/components/Icon.svelte';
  import { m } from '$lib/i18n';
  import { getPrimaryNavigationItems } from './shell-navigation';
  import type { ShellRoute } from './shell-types';

  type Props = {
    active?: ShellRoute;
    settingsActive?: boolean;
  };

  let { active, settingsActive = false }: Props = $props();
  const navItems = $derived(getPrimaryNavigationItems());
</script>

<nav
  class="glass-chrome z-[var(--layer-chrome)] grid min-h-[var(--density-control-lg)] shrink-0 grid-cols-5 border-t border-hairline"
  style:padding-right="var(--safe-area-right)"
  style:padding-bottom="max(var(--safe-area-bottom), var(--shell-bottom-safe-padding))"
  style:padding-left="var(--safe-area-left)"
  aria-label={m.nav_bottom()}
  data-bottom-nav="visible"
>
  {#each navItems as item (item.key)}
    <a
      href={resolve(item.href)}
      class="inline-flex min-h-[var(--density-control-lg)] min-w-0 flex-col items-center justify-center gap-0 px-1 py-0.5 text-[0.6875rem] leading-none font-medium text-ink-muted outline-none hover:bg-surface-2 hover:text-ink focus-visible:shadow-[inset_var(--focus-ring)] aria-[current=page]:text-ink"
      aria-current={active === item.key ? 'page' : undefined}
    >
      <Icon name={item.icon} class="size-[18px]" />
      <span class="truncate">{item.label}</span>
    </a>
  {/each}
  <a
    href={resolve('/settings' as '/')}
    class="inline-flex min-h-[var(--density-control-lg)] min-w-0 flex-col items-center justify-center gap-0 px-1 py-0.5 text-[0.6875rem] leading-none font-medium text-ink-muted outline-none hover:bg-surface-2 hover:text-ink focus-visible:shadow-[inset_var(--focus-ring)] aria-[current=page]:text-ink"
    aria-current={settingsActive ? 'page' : undefined}
  >
    <Icon name="gear-six" class="size-[18px]" />
    <span class="truncate">{m.settings()}</span>
  </a>
</nav>
