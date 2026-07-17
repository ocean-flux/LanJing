<script lang="ts">
  import Boxes from '@lucide/svelte/icons/boxes';
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import Compass from '@lucide/svelte/icons/compass';
  import Database from '@lucide/svelte/icons/database';
  import Radio from '@lucide/svelte/icons/radio';
  import Settings from '@lucide/svelte/icons/settings';
  import { asset, resolve } from '$app/paths';
  import { m } from '$lib/i18n';
  import { getPrimaryNavigationItems } from './shell-navigation';
  import type { ShellRoute } from './shell-types';

  type Props = {
    /** 四境当前项；设置路由时为 undefined，四境 dim 无 current */
    active?: ShellRoute | undefined;
    /** 当前是否在设置页（脊上设置链可 aria-current） */
    settingsActive?: boolean;
    oncollapse?: () => void;
  };

  let { active, settingsActive = false, oncollapse }: Props = $props();

  const navItems = $derived(getPrimaryNavigationItems());
  // 设置中四境无 active 时降低对比，避免「第五境」误读
  const realmsDimmed = $derived(settingsActive || active === undefined);
</script>

<nav
  class="motion-reader-recede flex h-full w-(--shell-rail-width) shrink-0 flex-col items-center border-r border-hairline bg-canvas px-1 py-2"
  aria-label={m.nav_main()}
  data-shell-rail="spine"
>
  <a
    href={resolve('/')}
    class="motion-dock-wake mb-3 inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-lg text-ink outline-none transition-colors hover:bg-surface-3 focus-visible:bg-surface-3"
    aria-label={m.realm_brand()}
    title={m.realm_brand()}
  >
    <span
      class="brand-monogram inline-flex h-6 w-6 shrink-0 items-center justify-center"
      style:--mark-url={`url(${asset('/brand/icon.png')})`}
      aria-hidden="true"
    >
      <span class="brand-monogram-glyph"></span>
    </span>
  </a>

  <div class="flex flex-1 flex-col items-center gap-1">
    {#each navItems as item (item.key)}
      {@const isCurrent = !settingsActive && active === item.key}
      <a
        href={resolve(item.href)}
        class={[
          'motion-nav-capsule group inline-flex h-10 w-10 items-center justify-center rounded-xl text-ink-muted outline-none transition-colors hover:bg-lantern-soft hover:text-ink focus-visible:bg-lantern-soft focus-visible:text-ink',
          isCurrent && 'bg-lantern-soft text-ink',
          realmsDimmed && !isCurrent && 'opacity-40',
        ]}
        aria-label={item.label}
        aria-current={isCurrent ? 'page' : undefined}
        title={item.label}
      >
        <span class="inline-flex h-5 w-5 items-center justify-center text-inherit [&_svg]:block">
          {#if item.key === 'realm'}
            <Compass size={18} aria-hidden="true" />
          {:else if item.key === 'apps'}
            <Boxes size={18} aria-hidden="true" />
          {:else if item.key === 'sources'}
            <Radio size={18} aria-hidden="true" />
          {:else}
            <Database size={18} aria-hidden="true" />
          {/if}
        </span>
      </a>
    {/each}
  </div>

  <div class="mt-auto flex flex-col items-center gap-1 pb-1">
    <a
      href={resolve('/settings' as '/')}
      class={[
        'motion-nav-capsule inline-flex h-10 w-10 items-center justify-center rounded-xl text-ink-muted outline-none transition-colors hover:bg-lantern-soft hover:text-ink focus-visible:bg-lantern-soft focus-visible:text-ink',
        settingsActive && 'bg-lantern-soft text-ink',
      ]}
      aria-label={m.settings_open()}
      aria-current={settingsActive ? 'page' : undefined}
      title={m.settings()}
      data-shell-rail-settings
    >
      <Settings size={18} strokeWidth={1.75} aria-hidden="true" />
    </a>
    <button
      type="button"
      class="inline-flex h-9 w-9 items-center justify-center rounded-lg text-ink-subtle outline-none transition-colors hover:bg-surface-3 hover:text-ink focus-visible:bg-surface-3 focus-visible:shadow-[var(--focus-ring)]"
      aria-label={m.rail_collapse()}
      title={m.rail_collapse()}
      data-shell-rail-collapse
      onclick={() => oncollapse?.()}
    >
      <ChevronLeft size={16} strokeWidth={1.75} aria-hidden="true" />
    </button>
  </div>
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
