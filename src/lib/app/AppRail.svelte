<script lang="ts">
  import { BookOpen, Boxes, Compass, Database, Radio } from '@lucide/svelte';
  import { asset, resolve } from '$app/paths';
  import { m } from '$lib/i18n';
  import type { ShellRoute } from './shell-types';

  type ShellHref = '/' | '/apps' | '/sources' | '/library';
  type NavItem = {
    key: ShellRoute;
    label: string;
    href: ShellHref;
    supporting: string;
  };

  type Props = {
    active?: ShellRoute;
    compact?: boolean;
  };

  const navItems: NavItem[] = [
    { key: 'realm', label: m.nav_realm(), href: '/', supporting: m.nav_realm_supporting() },
    { key: 'apps', label: m.nav_apps(), href: '/apps', supporting: m.nav_apps_supporting() },
    {
      key: 'sources',
      label: m.nav_sources(),
      href: '/sources',
      supporting: m.nav_sources_supporting(),
    },
    {
      key: 'library',
      label: m.nav_library(),
      href: '/library',
      supporting: m.nav_library_supporting(),
    },
  ];

  let { active = 'realm', compact = false }: Props = $props();
</script>

<nav
  class={[
    'motion-reader-recede flex min-h-0 shrink-0 flex-col border-r border-hairline bg-canvas px-3 py-3',
    compact ? 'w-(--shell-rail-tablet-width) items-center' : 'w-(--shell-rail-expanded-width)',
  ]}
  aria-label={m.nav_main()}
  data-shell-rail={compact ? 'icon' : 'expanded'}
>
  <a
    href={resolve('/')}
    class={[
      'motion-dock-wake inline-flex h-10 shrink-0 items-center rounded-xl text-ink outline-none transition-colors hover:bg-surface-3 focus-visible:bg-surface-3',
      compact ? 'w-10 justify-center' : 'w-full justify-start gap-3 px-2',
    ]}
    aria-label={m.realm_brand()}
  >
    <span
      class="brand-monogram inline-flex h-7 w-7 shrink-0 items-center justify-center"
      style:--mark-url={`url(${asset('/brand/icon.png')})`}
      aria-hidden="true"
    >
      <span class="brand-monogram-glyph"></span>
    </span>
    {#if !compact}
      <span class="min-w-0">
        <span class="block truncate text-sm font-semibold tracking-tight">LanJing</span>
        <span class="block truncate text-[0.68rem] text-ink-muted">{m.nav_main()}</span>
      </span>
    {/if}
  </a>

  <div class={['mt-8 flex flex-1 flex-col gap-2', compact && 'items-center']}>
    {#each navItems as item (item.key)}
      <a
        href={resolve(item.href)}
        class={[
          'motion-nav-capsule group inline-flex min-h-11 items-center rounded-xl text-ink-muted outline-none transition-colors hover:bg-lantern-soft hover:text-ink focus-visible:bg-lantern-soft focus-visible:text-ink aria-[current=page]:bg-lantern-soft aria-[current=page]:text-ink',
          compact ? 'w-10 justify-center' : 'w-full justify-start gap-3 px-3',
        ]}
        aria-label={item.label}
        aria-current={active === item.key ? 'page' : undefined}
        title={compact ? item.label : undefined}
      >
        <span
          class="inline-flex h-6 w-6 items-center justify-center text-inherit [&_svg]:block [&_svg]:transition-transform [&_svg]:duration-200 [&_svg]:ease-[var(--motion-standard)] group-aria-[current=page]:[&_svg]:scale-110"
        >
          {#if item.key === 'realm'}
            <Compass size={19} aria-hidden="true" />
          {:else if item.key === 'apps'}
            <Boxes size={19} aria-hidden="true" />
          {:else if item.key === 'sources'}
            <Radio size={19} aria-hidden="true" />
          {:else}
            <Database size={19} aria-hidden="true" />
          {/if}
        </span>
        {#if !compact}
          <span class="min-w-0">
            <span class="block truncate text-sm font-medium">{item.label}</span>
            <span class="block truncate text-[0.68rem] text-ink-subtle">{item.supporting}</span>
          </span>
        {/if}
      </a>
    {/each}
  </div>

  <a
    href={resolve('/apps/novel' as '/')}
    class={[
      'motion-nav-capsule inline-flex min-h-10 items-center rounded-xl text-ink-muted outline-none transition-colors hover:bg-lantern-soft hover:text-ink focus-visible:bg-lantern-soft focus-visible:text-ink',
      compact ? 'w-10 justify-center' : 'w-full justify-start gap-3 px-3',
    ]}
    aria-label={m.nav_novel()}
    title={compact ? m.nav_novel() : undefined}
  >
    <BookOpen size={18} aria-hidden="true" />
    {#if !compact}
      <span class="text-sm font-medium">{m.nav_novel()}</span>
    {/if}
  </a>
</nav>

<style>
  .brand-monogram-glyph {
    display: block;
    width: 100%;
    height: 100%;
    background-color: var(--ink);
    opacity: 0.88;
    -webkit-mask-image: var(--mark-url);
    mask-image: var(--mark-url);
    -webkit-mask-repeat: no-repeat;
    mask-repeat: no-repeat;
    -webkit-mask-position: center;
    mask-position: center;
    -webkit-mask-size: contain;
    mask-size: contain;
  }
</style>
