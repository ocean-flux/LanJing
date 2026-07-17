<script lang="ts">
  import { resolve } from '$app/paths';
  import Boxes from '@lucide/svelte/icons/boxes';
  import Compass from '@lucide/svelte/icons/compass';
  import Database from '@lucide/svelte/icons/database';
  import Radio from '@lucide/svelte/icons/radio';
  import { m } from '$lib/i18n';
  import { getPrimaryNavigationItems } from './shell-navigation';
  import type { ShellRoute } from './shell-types';

  type Props = {
    /** 四境当前项；设置路由时 undefined */
    active?: ShellRoute | undefined;
    hidden?: boolean;
  };

  let { active, hidden = false }: Props = $props();
  const navItems = $derived(getPrimaryNavigationItems());
</script>

{#if !hidden}
  <nav
    class="motion-reader-recede flex min-h-[calc(var(--shell-bottom-nav-height)+var(--shell-bottom-safe-padding))] shrink-0 items-start justify-around border-t border-hairline bg-canvas px-1 pb-(--shell-bottom-safe-padding) pt-2 sm:px-2"
    aria-label={m.nav_bottom()}
    data-bottom-nav="visible"
  >
    {#each navItems as item (item.key)}
      <a
        href={resolve(item.href)}
        class="motion-nav-capsule inline-flex min-h-11 min-w-12 flex-1 flex-col items-center justify-center gap-1 rounded-xl px-1 text-[0.72rem] font-medium text-ink-muted outline-none transition-colors hover:bg-lantern-soft hover:text-ink focus-visible:bg-lantern-soft focus-visible:text-ink motion-reduce:transform-none aria-[current=page]:bg-lantern-soft aria-[current=page]:text-ink sm:min-w-14 sm:px-2"
        aria-label={item.label}
        aria-current={active === item.key ? 'page' : undefined}
      >
        {#if item.key === 'realm'}
          <Compass size={18} aria-hidden="true" />
        {:else if item.key === 'apps'}
          <Boxes size={18} aria-hidden="true" />
        {:else if item.key === 'sources'}
          <Radio size={18} aria-hidden="true" />
        {:else}
          <Database size={18} aria-hidden="true" />
        {/if}
        <span>{item.label}</span>
      </a>
    {/each}
  </nav>
{/if}
