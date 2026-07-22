<script lang="ts">
  import Boxes from '@lucide/svelte/icons/boxes';
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import Compass from '@lucide/svelte/icons/compass';
  import Database from '@lucide/svelte/icons/database';
  import MousePointer2 from '@lucide/svelte/icons/mouse-pointer-2';
  import PanelLeft from '@lucide/svelte/icons/panel-left';
  import Radio from '@lucide/svelte/icons/radio';
  import Settings from '@lucide/svelte/icons/settings';
  import { asset, resolve } from '$app/paths';
  import { m } from '$lib/i18n';
  import { tick } from 'svelte';
  import IslandExpand from './IslandExpand.svelte';
  import { portal } from './portal';
  import { getPrimaryNavigationItems } from './shell-navigation';
  import type { RailBehavior } from './shell-rail-preference';
  import type { NativeWindowControlMode, ShellRoute } from './shell-types';

  type Props = {
    active?: ShellRoute | undefined;
    settingsActive?: boolean;
    /** 当前侧栏行为：固定 / 悬停。 */
    railBehavior?: RailBehavior;
    oncollapse?: () => void;
    /** 切换 fixed ↔ hover。 */
    onbehaviorchange?: (next: RailBehavior) => void;
    nativeControlMode?: NativeWindowControlMode;
  };

  let {
    active,
    settingsActive = false,
    railBehavior = 'fixed',
    oncollapse,
    onbehaviorchange,
    nativeControlMode = 'browser-preview',
  }: Props = $props();

  const navItems = $derived(getPrimaryNavigationItems());
  const realmsDimmed = $derived(settingsActive || active === undefined);
  const isHoverMode = $derived(railBehavior === 'hover');

  let railMenuOpen = $state(false);
  let menuTop = $state(0);
  let menuLeft = $state(0);
  let optionsTriggerEl: HTMLButtonElement | null = null;

  async function closeRailMenu(restoreFocus = true) {
    if (!railMenuOpen) return;
    railMenuOpen = false;
    await tick();
    if (restoreFocus) optionsTriggerEl?.focus();
  }

  function placeRailMenu() {
    if (!optionsTriggerEl) return;
    const rect = optionsTriggerEl.getBoundingClientRect();
    menuLeft = Math.round(rect.right + 10);
    menuTop = Math.round(rect.top + rect.height / 2);
  }

  async function toggleRailMenu(event: MouseEvent) {
    optionsTriggerEl = event.currentTarget as HTMLButtonElement;
    if (railMenuOpen) {
      await closeRailMenu();
      return;
    }
    placeRailMenu();
    railMenuOpen = true;
    await tick();
    document.querySelector<HTMLElement>('[data-shell-rail-options-menu] button')?.focus();
  }

  async function pickBehavior(next: RailBehavior) {
    onbehaviorchange?.(next);
    await closeRailMenu();
  }

  function collapseFromMenu() {
    void closeRailMenu(false);
    oncollapse?.();
  }

  function onMenuPointerDown(event: PointerEvent) {
    event.stopPropagation();
  }

  function onDocPointerDown(event: PointerEvent) {
    if (!railMenuOpen) return;
    const target = event.target;
    if (!(target instanceof Node)) return;
    const optionsMenuEl = document.querySelector('[data-shell-rail-options-menu]');
    if (optionsTriggerEl?.contains(target) || optionsMenuEl?.contains(target)) return;
    queueMicrotask(() => void closeRailMenu());
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && railMenuOpen) {
      event.preventDefault();
      void closeRailMenu();
    }
  }

  function onResize() {
    if (railMenuOpen) placeRailMenu();
  }
</script>

<svelte:window onresize={onResize} />
<svelte:document onpointerdown={onDocPointerDown} onkeydown={onKeydown} />

<!-- 侧浮玻璃岛：四境 + 设置 + 单一侧栏菜单 + 窗控弹出 -->
<nav
  class="glass-island-rail motion-reader-recede titlebar-no-drag relative z-20 my-3 ml-3 flex h-[calc(100%-1.5rem)] w-(--shell-rail-width) shrink-0 flex-col items-center self-stretch p-1"
  aria-label={m.nav_main()}
  data-shell-rail="spine"
  data-shell-island="rail"
  data-rail-behavior={railBehavior}
>
  <div class="glass-island-rail-core flex h-full w-full flex-col items-center py-2">
    <a
      href={resolve('/')}
      class="motion-dock-wake mb-3 inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-xl text-ink outline-none transition-colors hover:bg-surface-3 focus-visible:bg-surface-3"
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

      <IslandExpand {active} {settingsActive} {nativeControlMode} triggerVariant="rail" />

      <!-- 单一侧栏控制：向右弹出半透明菜单 -->
      <div class="relative" data-shell-rail-options>
        {#if railMenuOpen}
          <div
            class="rail-options-menu"
            role="dialog"
            aria-modal="false"
            tabindex="-1"
            aria-label={m.rail_options()}
            data-shell-rail-options-menu
            style:top="{menuTop}px"
            style:left="{menuLeft}px"
            use:portal
            onpointerdown={onMenuPointerDown}
          >
            <button
              type="button"
              class={[
                'rail-option flex w-full items-center gap-2 rounded-xl px-2.5 py-2 text-left text-[0.78rem] outline-none transition-colors',
                !isHoverMode
                  ? 'bg-lantern-soft/40 text-ink'
                  : 'text-ink-muted hover:bg-surface-3/45 hover:text-ink',
              ]}
              data-rail-option="fixed"
              onclick={() => pickBehavior('fixed')}
            >
              <PanelLeft size={14} strokeWidth={1.75} aria-hidden="true" />
              <span class="leading-tight">{m.rail_behavior_fixed_label()}</span>
            </button>
            <button
              type="button"
              class={[
                'rail-option flex w-full items-center gap-2 rounded-xl px-2.5 py-2 text-left text-[0.78rem] outline-none transition-colors',
                isHoverMode
                  ? 'bg-lantern-soft/40 text-ink'
                  : 'text-ink-muted hover:bg-surface-3/45 hover:text-ink',
              ]}
              data-rail-option="hover"
              onclick={() => pickBehavior('hover')}
            >
              <MousePointer2 size={14} strokeWidth={1.75} aria-hidden="true" />
              <span class="leading-tight">{m.rail_behavior_hover_label()}</span>
            </button>
            {#if !isHoverMode}
              <div class="my-1 h-px bg-[color-mix(in_oklab,var(--ink)_8%,transparent)]"></div>
              <button
                type="button"
                class="rail-option flex w-full items-center gap-2 rounded-xl px-2.5 py-2 text-left text-[0.78rem] text-ink-muted outline-none transition-colors hover:bg-surface-3/45 hover:text-ink"
                data-rail-option="collapse"
                data-shell-rail-collapse
                onclick={collapseFromMenu}
              >
                <ChevronLeft size={14} strokeWidth={1.75} aria-hidden="true" />
                <span class="leading-tight">{m.rail_collapse()}</span>
              </button>
            {/if}
          </div>
        {/if}

        <button
          type="button"
          class={[
            'inline-flex h-9 w-9 items-center justify-center rounded-xl outline-none transition-colors focus-visible:shadow-[var(--focus-ring)]',
            railMenuOpen || isHoverMode
              ? 'bg-lantern-soft/35 text-ink hover:bg-lantern-soft/50'
              : 'text-ink-subtle hover:bg-surface-3/50 hover:text-ink',
          ]}
          aria-label={m.rail_options()}
          aria-expanded={railMenuOpen}
          aria-haspopup="dialog"
          title={isHoverMode ? m.rail_behavior_hover() : m.rail_behavior_fixed()}
          data-shell-rail-behavior
          data-rail-behavior={railBehavior}
          onclick={toggleRailMenu}
        >
          {#if isHoverMode}
            <MousePointer2 size={16} strokeWidth={1.75} aria-hidden="true" />
          {:else}
            <PanelLeft size={16} strokeWidth={1.75} aria-hidden="true" />
          {/if}
        </button>
      </div>
    </div>
  </div>
</nav>

<style>
  .titlebar-no-drag {
    -webkit-app-region: no-drag;
  }

  .glass-island-rail {
    border-radius: var(--radius-3xl);
    border: 1px solid var(--surface-material-border);
    background: var(--surface-material);
    box-shadow:
      inset 0 1px 0 color-mix(in oklab, var(--ink) 10%, transparent),
      0 0 0 1px color-mix(in oklab, var(--lantern) 12%, transparent),
      0 16px 48px color-mix(in oklab, #000 42%, transparent);
  }

  .glass-island-rail-core {
    border-radius: calc(var(--radius-3xl) - 4px);
    box-shadow: inset 0 1px 0 color-mix(in oklab, var(--ink) 8%, transparent);
  }

  @supports (backdrop-filter: blur(1px)) {
    .glass-island-rail {
      backdrop-filter: blur(var(--material-blur)) saturate(var(--material-saturation));
      -webkit-backdrop-filter: blur(var(--material-blur)) saturate(var(--material-saturation));
    }
  }

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
  .rail-options-menu {
    position: fixed;
    z-index: 80;
    min-width: 9.5rem;
    padding: 0.35rem;
    border-radius: 1rem;
    border: 1px solid var(--surface-material-border);
    background: var(--surface-material);
    box-shadow: var(--surface-dialog-shadow);
    transform: translateY(-50%);
    backdrop-filter: blur(var(--material-blur)) saturate(var(--material-saturation));
    -webkit-backdrop-filter: blur(var(--material-blur)) saturate(var(--material-saturation));
    animation: rail-menu-in var(--motion-duration-standard) var(--motion-standard) both;
  }

  :global(:root[data-material-transparency='low']) .glass-island-rail,
  :global(:root[data-material-transparency='low']) .rail-options-menu,
  :global(:root.low-transparency) .glass-island-rail,
  :global(:root.low-transparency) .rail-options-menu,
  :global([data-reduced-transparency='true']) .glass-island-rail,
  :global([data-reduced-transparency='true']) .rail-options-menu {
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }

  @keyframes rail-menu-in {
    from {
      opacity: 0;
      transform: translateY(-50%) translateX(-6px);
    }
    to {
      opacity: 1;
      transform: translateY(-50%) translateX(0);
    }
  }

  @media (prefers-reduced-transparency: reduce) {
    .glass-island-rail,
    .rail-options-menu {
      backdrop-filter: none;
      -webkit-backdrop-filter: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .rail-options-menu {
      animation: none;
    }
  }
</style>
