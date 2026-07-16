<script lang="ts">
  import { resolve } from '$app/paths';
  import { Boxes, Compass, Database, Radio, Search, Settings } from '@lucide/svelte';
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
    /** 与 titlebar 共用：打开全局搜索（移动无 titlebar 时的唯一入口）。 */
    onsearch?: () => void;
  };

  const navItems: NavItem[] = [
    { key: 'realm', label: m.nav_realm(), href: '/' },
    { key: 'apps', label: m.nav_apps(), href: '/apps' },
    { key: 'sources', label: m.nav_sources(), href: '/sources' },
    { key: 'library', label: m.nav_library(), href: '/library' },
  ];

  let { active = 'realm', hidden = false, onsearch }: Props = $props();
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
    <button
      type="button"
      class="motion-command-lens inline-flex min-h-11 min-w-12 flex-1 flex-col items-center justify-center gap-1 rounded-xl px-1 text-[0.72rem] font-medium text-ink-muted outline-none transition-colors hover:bg-lantern-soft hover:text-ink focus-visible:bg-lantern-soft focus-visible:text-ink motion-reduce:transform-none sm:min-w-14 sm:px-2"
      aria-label={m.search_open()}
      data-bottom-nav-search
      onclick={() => onsearch?.()}
    >
      <Search size={18} aria-hidden="true" />
      <span>{m.search()}</span>
    </button>
    <a
      href={resolve('/settings' as '/')}
      class="motion-nav-capsule inline-flex min-h-11 min-w-12 flex-1 flex-col items-center justify-center gap-1 rounded-xl px-1 text-[0.72rem] font-medium text-ink-muted outline-none transition-colors hover:bg-lantern-soft hover:text-ink focus-visible:bg-lantern-soft focus-visible:text-ink motion-reduce:transform-none sm:min-w-14 sm:px-2"
      aria-label={m.settings_open()}
      data-bottom-nav-settings
    >
      <Settings size={18} aria-hidden="true" />
      <span>{m.settings()}</span>
    </a>
  </nav>
{/if}
