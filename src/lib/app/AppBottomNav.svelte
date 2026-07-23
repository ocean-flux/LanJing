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
  class="glass-chrome grid min-h-11 shrink-0 grid-cols-5 border-t border-hairline supports-backdrop-filter:backdrop-blur-[var(--material-blur)]"
  style:padding-bottom="max(env(safe-area-inset-bottom), var(--shell-bottom-safe-padding))"
  aria-label={m.nav_bottom()}
  data-bottom-nav="visible"
>
  {#each navItems as item (item.key)}
    <a
      href={resolve(item.href)}
      class="inline-flex min-h-11 min-w-0 flex-col items-center justify-center gap-0.5 px-1 py-1 text-[0.7rem] font-medium text-ink-muted outline-none hover:bg-surface-2 hover:text-ink focus-visible:shadow-[inset_var(--focus-ring)] aria-[current=page]:text-ink"
      aria-current={active === item.key ? 'page' : undefined}
    >
      <Icon name={item.icon} class="size-[18px]" />
      <span class="truncate">{item.label}</span>
    </a>
  {/each}
  <a
    href={resolve('/settings' as '/')}
    class="inline-flex min-h-11 min-w-0 flex-col items-center justify-center gap-0.5 px-1 py-1 text-[0.7rem] font-medium text-ink-muted outline-none hover:bg-surface-2 hover:text-ink focus-visible:shadow-[inset_var(--focus-ring)] aria-[current=page]:text-ink"
    aria-current={settingsActive ? 'page' : undefined}
  >
    <Icon name="gear-six" class="size-[18px]" />
    <span class="truncate">{m.settings()}</span>
  </a>
</nav>
