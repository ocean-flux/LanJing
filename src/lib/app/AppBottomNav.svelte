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
  <!-- 底浮玻璃岛：safe-area 上浮，非贴边满宽粗条 -->
  <div
    class="pointer-events-none absolute inset-x-0 bottom-0 z-20 flex justify-center px-3 pb-[max(0.5rem,var(--shell-bottom-safe-padding))] pt-2"
    data-bottom-nav-slot
  >
    <nav
      class="glass-island-bottom motion-reader-recede pointer-events-auto flex min-h-(--shell-bottom-nav-height) w-full max-w-md items-stretch justify-around gap-0.5 p-1 sm:px-1.5"
      aria-label={m.nav_bottom()}
      data-bottom-nav="visible"
      data-shell-island="bottom"
    >
      {#each navItems as item (item.key)}
        <a
          href={resolve(item.href)}
          class="motion-nav-capsule inline-flex min-h-11 min-w-12 flex-1 flex-col items-center justify-center gap-1 rounded-2xl px-1 text-[0.72rem] font-medium text-ink-muted outline-none transition-colors hover:bg-lantern-soft hover:text-ink focus-visible:bg-lantern-soft focus-visible:text-ink motion-reduce:transform-none aria-[current=page]:bg-lantern-soft aria-[current=page]:text-ink sm:min-w-14 sm:px-2"
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
  </div>
{/if}

<style>
  .glass-island-bottom {
    border-radius: var(--radius-3xl);
    border: 1px solid var(--surface-material-border);
    background: var(--surface-material);
    box-shadow:
      var(--surface-panel-shadow),
      0 10px 32px color-mix(in oklab, var(--canvas) 40%, transparent),
      inset 0 1px 0 color-mix(in oklab, var(--ink) 6%, transparent);
  }

  @supports (backdrop-filter: blur(1px)) {
    .glass-island-bottom {
      backdrop-filter: blur(var(--material-blur)) saturate(var(--material-saturation));
      -webkit-backdrop-filter: blur(var(--material-blur)) saturate(var(--material-saturation));
    }
  }

  :global(:root[data-material-transparency='low']) .glass-island-bottom,
  :global(:root.low-transparency) .glass-island-bottom,
  :global([data-reduced-transparency='true']) .glass-island-bottom {
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }

  @media (prefers-reduced-transparency: reduce) {
    .glass-island-bottom {
      backdrop-filter: none;
      -webkit-backdrop-filter: none;
    }
  }
</style>
