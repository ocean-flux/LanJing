<script lang="ts">
  import { resolve } from '$app/paths';
  import { Boxes, Compass, Database, Radio } from '@lucide/svelte';
  import { m } from '$lib/i18n';
  import type { ShellRoute } from './shell-types';

  type ShellHref = '/' | '/apps' | '/sources' | '/library';
  type NavItem = {
    key: ShellRoute;
    label: string;
    href: ShellHref;
  };

  type Props = {
    active?: ShellRoute;
    hidden?: boolean;
  };

  const navItems: NavItem[] = [
    { key: 'realm', label: m.nav_realm(), href: '/' },
    { key: 'apps', label: m.nav_apps(), href: '/apps' },
    { key: 'sources', label: m.nav_sources(), href: '/sources' },
    { key: 'library', label: m.nav_library(), href: '/library' },
  ];

  let { active = 'realm', hidden = false }: Props = $props();
</script>

{#if !hidden}
  <nav
    class="motion-reader-recede flex min-h-[calc(var(--shell-bottom-nav-height)+var(--shell-bottom-safe-padding))] shrink-0 items-start justify-around border-t border-hairline bg-canvas px-2 pb-(--shell-bottom-safe-padding) pt-2 md:hidden"
    aria-label={m.nav_bottom()}
    data-bottom-nav="visible"
  >
    {#each navItems as item (item.key)}
      <a
        href={resolve(item.href)}
        class="motion-nav-capsule inline-flex min-h-11 min-w-16 flex-col items-center justify-center gap-1 rounded-xl px-3 text-[0.72rem] font-medium text-ink-muted outline-none transition-colors hover:bg-lantern-soft hover:text-ink focus-visible:bg-lantern-soft focus-visible:text-ink motion-reduce:transform-none aria-[current=page]:bg-lantern-soft aria-[current=page]:text-ink"
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
