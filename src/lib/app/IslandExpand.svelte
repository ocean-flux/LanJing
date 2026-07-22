<script lang="ts">
  import { resolve } from '$app/paths';
  import Boxes from '@lucide/svelte/icons/boxes';
  import Compass from '@lucide/svelte/icons/compass';
  import Database from '@lucide/svelte/icons/database';
  import Maximize2 from '@lucide/svelte/icons/maximize-2';
  import Radio from '@lucide/svelte/icons/radio';
  import Settings from '@lucide/svelte/icons/settings';
  import X from '@lucide/svelte/icons/x';
  import { m } from '$lib/i18n';
  import { tick } from 'svelte';
  import { getPrimaryNavigationItems } from './shell-navigation';
  import type { NativeWindowControlMode, ShellRoute } from './shell-types';
  import { portal } from './portal';
  import { shouldRenderHtmlWindowControls } from './window-controls';

  type Props = {
    active?: ShellRoute | undefined;
    settingsActive?: boolean;
    nativeControlMode: NativeWindowControlMode;
    triggerVariant?: 'rail' | 'mobile';
  };

  let {
    active,
    settingsActive = false,
    nativeControlMode,
    triggerVariant = 'rail',
  }: Props = $props();

  const navItems = $derived(getPrimaryNavigationItems());
  const hasHtmlWindowControls = $derived(shouldRenderHtmlWindowControls(nativeControlMode));
  let open = $state(false);
  let previousFocus: HTMLElement | null = null;

  function focusableElements(): HTMLElement[] {
    const panelEl = document.querySelector<HTMLElement>('[data-island-expand]');
    if (!panelEl) return [];
    return Array.from(
      panelEl.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
    );
  }

  async function openExpand(event: MouseEvent) {
    previousFocus = event.currentTarget as HTMLButtonElement;
    open = true;
    await tick();
    focusableElements()[0]?.focus();
  }

  async function closeExpand({ restoreFocus = true } = {}) {
    if (!open) return;
    open = false;
    await tick();
    if (restoreFocus) previousFocus?.focus();
  }

  function onKeydown(event: KeyboardEvent) {
    if (!open) return;

    if (event.key === 'Escape') {
      event.preventDefault();
      void closeExpand();
      return;
    }

    if (event.key !== 'Tab') return;
    const focusable = focusableElements();
    if (focusable.length === 0) {
      event.preventDefault();
      document.querySelector<HTMLElement>('[data-island-expand]')?.focus();
      return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }
</script>

<svelte:document onkeydown={onKeydown} />

<button
  type="button"
  class={[
    'titlebar-no-drag inline-flex items-center justify-center text-ink-muted outline-none transition-colors hover:bg-lantern-soft hover:text-ink focus-visible:bg-lantern-soft focus-visible:text-ink focus-visible:shadow-[var(--focus-ring)]',
    triggerVariant === 'rail' ? 'h-10 w-10 rounded-xl' : 'h-9 w-9 rounded-lg',
  ]}
  aria-label={m.island_expand_open()}
  aria-expanded={open}
  aria-haspopup="dialog"
  title={m.island_expand_title()}
  data-island-expand-trigger={triggerVariant}
  onclick={openExpand}
>
  <Maximize2 size={triggerVariant === 'rail' ? 18 : 17} strokeWidth={1.75} aria-hidden="true" />
</button>

{#if open}
  <div class="island-expand-layer" data-island-expand-layer use:portal>
    <button
      type="button"
      class="island-expand-backdrop"
      tabindex="-1"
      aria-hidden="true"
      onclick={() => closeExpand()}
    ></button>
    <div
      class="island-expand-panel titlebar-no-drag"
      role="dialog"
      aria-modal="true"
      aria-labelledby="island-expand-title"
      aria-describedby="island-expand-description"
      data-island-expand
      data-window-control-mode={nativeControlMode}
      tabindex="-1"
    >
      <header class="island-expand-heading">
        <div>
          <p class="text-[0.68rem] font-semibold uppercase tracking-[0.18em] text-ink-subtle">
            {m.app_name()}
          </p>
          <h2 id="island-expand-title" class="mt-1 text-xl font-semibold tracking-tight text-ink">
            {m.island_expand_title()}
          </h2>
          <p id="island-expand-description" class="mt-1 text-sm text-ink-muted">
            {m.island_expand_description()}
          </p>
        </div>
        <button
          type="button"
          class="inline-flex h-11 w-11 items-center justify-center rounded-2xl text-ink-muted outline-none transition-colors hover:bg-surface-3 hover:text-ink focus-visible:shadow-[var(--focus-ring)]"
          aria-label={m.island_expand_close()}
          data-island-first
          onclick={() => closeExpand()}
        >
          <X size={19} strokeWidth={1.75} aria-hidden="true" />
        </button>
      </header>

      <nav class="island-expand-nav" aria-label={m.island_expand_nav()}>
        {#each navItems as item (item.key)}
          <a
            href={resolve(item.href)}
            class="island-expand-link"
            aria-current={!settingsActive && active === item.key ? 'page' : undefined}
            onclick={() => closeExpand({ restoreFocus: false })}
          >
            <span class="island-expand-icon" aria-hidden="true">
              {#if item.key === 'realm'}
                <Compass size={19} />
              {:else if item.key === 'apps'}
                <Boxes size={19} />
              {:else if item.key === 'sources'}
                <Radio size={19} />
              {:else}
                <Database size={19} />
              {/if}
            </span>
            <span>{item.label}</span>
          </a>
        {/each}
        <a
          href={resolve('/settings' as '/')}
          class="island-expand-link"
          aria-current={settingsActive ? 'page' : undefined}
          onclick={() => closeExpand({ restoreFocus: false })}
        >
          <span class="island-expand-icon" aria-hidden="true"><Settings size={19} /></span>
          <span>{m.settings()}</span>
        </a>
        <a
          href={resolve('/settings#appearance' as '/')}
          class="island-expand-link"
          onclick={() => closeExpand({ restoreFocus: false })}
        >
          <span class="island-expand-icon" aria-hidden="true"><Maximize2 size={19} /></span>
          <span>{m.island_expand_appearance()}</span>
        </a>
      </nav>

      <section class="island-expand-window" aria-labelledby="island-expand-window-title">
        <h3 id="island-expand-window-title" class="text-sm font-semibold text-ink">
          {m.island_expand_window()}
        </h3>
        <p class="mt-1 text-sm leading-relaxed text-ink-muted">
          {hasHtmlWindowControls
            ? m.island_expand_window_html()
            : nativeControlMode === 'macos-overlay'
              ? m.island_expand_window_macos()
              : m.island_expand_window_system()}
        </p>
        <p class="mt-1 text-xs leading-relaxed text-ink-subtle">
          {m.island_expand_window_hint()}
        </p>
      </section>
    </div>
  </div>
{/if}

<style>
  .titlebar-no-drag {
    -webkit-app-region: no-drag;
  }

  .island-expand-layer {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: grid;
    place-items: center;
    padding: clamp(0.75rem, 3vw, 2rem);
  }

  .island-expand-backdrop {
    position: absolute;
    inset: 0;
    border: 0;
    background: color-mix(in oklab, var(--canvas) 72%, transparent);
    cursor: default;
  }

  .island-expand-panel {
    position: relative;
    display: flex;
    width: min(58rem, 96vw);
    max-height: min(44rem, 92dvh);
    flex-direction: column;
    gap: 1.25rem;
    overflow: auto;
    border: 1px solid var(--surface-material-border);
    border-radius: var(--radius-3xl);
    background: var(--surface-material);
    box-shadow: var(--surface-dialog-shadow);
    padding: clamp(1rem, 3vw, 2rem);
    outline: none;
    animation: island-expand-in var(--motion-duration-standard) var(--motion-emphasis) both;
  }

  .island-expand-heading {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  .island-expand-nav {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(9rem, 1fr));
    gap: 0.5rem;
  }

  .island-expand-link {
    display: flex;
    min-height: 3.5rem;
    align-items: center;
    gap: 0.75rem;
    border-radius: var(--radius-xl);
    padding: 0.65rem 0.75rem;
    color: var(--ink-muted);
    outline: none;
    transition:
      color var(--motion-duration-fast) var(--motion-standard),
      background-color var(--motion-duration-fast) var(--motion-standard);
  }

  .island-expand-link:hover,
  .island-expand-link:focus-visible,
  .island-expand-link[aria-current='page'] {
    color: var(--ink);
    background: var(--lantern-soft);
  }

  .island-expand-link:focus-visible {
    box-shadow: var(--focus-ring);
  }

  .island-expand-icon {
    display: grid;
    width: 2rem;
    height: 2rem;
    flex: none;
    place-items: center;
    border: 1px solid var(--surface-material-border);
    border-radius: 0.75rem;
    background: var(--surface-material);
  }

  .island-expand-window {
    border: 1px solid var(--surface-material-border);
    border-radius: var(--radius-xl);
    background: var(--surface-material);
    padding: 0.9rem 1rem;
  }

  @supports (backdrop-filter: blur(1px)) {
    .island-expand-panel,
    .island-expand-icon,
    .island-expand-window {
      backdrop-filter: blur(var(--material-blur)) saturate(var(--material-saturation));
      -webkit-backdrop-filter: blur(var(--material-blur)) saturate(var(--material-saturation));
    }
  }

  :global(:root[data-material-transparency='low']) .island-expand-panel,
  :global(:root[data-material-transparency='low']) .island-expand-icon,
  :global(:root[data-material-transparency='low']) .island-expand-window,
  :global(:root.low-transparency) .island-expand-panel,
  :global(:root.low-transparency) .island-expand-icon,
  :global(:root.low-transparency) .island-expand-window,
  :global([data-reduced-transparency='true']) .island-expand-panel,
  :global([data-reduced-transparency='true']) .island-expand-icon,
  :global([data-reduced-transparency='true']) .island-expand-window {
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }

  @keyframes island-expand-in {
    from {
      opacity: 0;
      transform: translateY(0.75rem);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .island-expand-panel {
      animation: none;
    }
  }

  @media (prefers-reduced-transparency: reduce) {
    .island-expand-panel,
    .island-expand-icon,
    .island-expand-window {
      backdrop-filter: none;
      -webkit-backdrop-filter: none;
    }
  }
</style>
