<script lang="ts">
  import { Monitor, Moon, Search, Sun } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';
  import { m } from '$lib/i18n';
  import { getMode, setMode, type ThemeMode } from '$lib/stores/theme.svelte';
  import type { NativeWindowControlMode } from './shell-types';

  type WindowAction = 'minimize' | 'toggle-maximize' | 'close';
  type Props = {
    contextLabel?: string;
    compact?: boolean;
    nativeControlMode?: NativeWindowControlMode;
    themeMode?: ThemeMode;
    onsearch?: () => void;
  };

  const titlebarMessages = m as typeof m & {
    window_controls_preview: () => string;
    window_minimize: () => string;
    window_toggle_maximize: () => string;
    window_close: () => string;
  };

  let {
    contextLabel = m.nav_realm(),
    compact = false,
    nativeControlMode: controlledNativeControlMode,
    themeMode: controlledThemeMode,
    onsearch,
  }: Props = $props();

  const nextMode: Record<ThemeMode, ThemeMode> = {
    system: 'light',
    light: 'dark',
    dark: 'system',
  };

  /** Test-only fallback when AppShell does not pass shell.platform.windowControls. */
  function resolveNativeControlModeFallback(): NativeWindowControlMode {
    if (typeof window === 'undefined') return 'browser-preview';

    const platform = navigator.userAgent.toLowerCase();
    const tauri = '__TAURI_INTERNALS__' in window || platform.includes('tauri');

    if (!tauri) return 'browser-preview';
    if (platform.includes('mac')) return 'macos-overlay';
    if (platform.includes('win')) return 'system-decorated';
    return 'system-decorated';
  }

  const currentMode = $derived(controlledThemeMode ?? getMode());
  const modeLabel = $derived.by(() => {
    if (currentMode === 'light') return m.theme_mode_light();
    if (currentMode === 'dark') return m.theme_mode_dark();
    return m.theme_mode_system();
  });
  // Production path: AppShell always supplies shell.platform.windowControls.
  const nativeControlMode = $derived(
    controlledNativeControlMode ?? resolveNativeControlModeFallback(),
  );
  const showPreviewControls = $derived(nativeControlMode === 'browser-preview');
  const titlebarControlLeft = $derived(nativeControlMode === 'macos-overlay' ? '86px' : '0px');
  const titlebarControlRight = $derived(nativeControlMode === 'windows-overlay' ? '138px' : '0px');

  function cycleThemeMode() {
    setMode(nextMode[currentMode]);
  }

  async function runWindowAction(action: WindowAction) {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const appWindow = getCurrentWindow();

      if (action === 'minimize') {
        await appWindow.minimize();
        return;
      }

      if (action === 'toggle-maximize') {
        await appWindow.toggleMaximize();
        return;
      }

      await appWindow.close();
    } catch {
      // 浏览器预览保留视觉位置；真实窗口控制由 Tauri / 系统接管。
    }
  }
</script>

<header
  class={[
    'titlebar-surface motion-reader-recede relative flex shrink-0 items-center gap-3 overflow-hidden border-b border-hairline bg-surface-1 px-3 text-sm',
    'pl-[calc(var(--titlebar-control-left,0px)+0.75rem)] pr-[calc(var(--titlebar-control-right,0px)+0.75rem)]',
    compact ? 'h-(--shell-titlebar-compact-height)' : 'h-(--shell-titlebar-height)',
  ]}
  style:--titlebar-control-left={titlebarControlLeft}
  style:--titlebar-control-right={titlebarControlRight}
  aria-label={m.titlebar_label({ context: contextLabel })}
  aria-describedby="titlebar-native-controls"
  data-native-window-controls={nativeControlMode}
  data-tauri-drag-region
>
  <span id="titlebar-native-controls" class="sr-only">{m.titlebar_native_controls()}</span>
  <div class="relative z-10 min-w-0 font-medium text-ink-muted" data-tauri-drag-region>
    <span class="truncate" data-tauri-drag-region>览境 / {contextLabel}</span>
  </div>

  <div class="relative z-10 min-w-4 flex-1" data-tauri-drag-region></div>

  <div class="relative z-10 flex items-center gap-1.5">
    <Button
      type="button"
      variant="outline"
      size="sm"
      class="motion-command-lens h-8 rounded-sm border-hairline bg-surface-1 px-3 text-ink-muted hover:bg-lantern-soft hover:text-ink"
      aria-label={m.search_open()}
      onclick={onsearch}
    >
      <Search size={14} strokeWidth={1.9} aria-hidden="true" />
      <span class="hidden sm:inline">{m.search()}</span>
    </Button>
    <Button
      type="button"
      variant="ghost"
      size="sm"
      class="motion-nav-capsule grid h-8 w-8 place-items-center rounded-sm p-0 text-ink-muted hover:bg-lantern-soft hover:text-ink"
      aria-label={m.theme_mode_toggle({ mode: modeLabel })}
      title={m.theme_mode_toggle({ mode: modeLabel })}
      onclick={cycleThemeMode}
    >
      {#if currentMode === 'light'}
        <Sun size={15} strokeWidth={1.9} aria-hidden="true" />
      {:else if currentMode === 'dark'}
        <Moon size={15} strokeWidth={1.9} aria-hidden="true" />
      {:else}
        <Monitor size={15} strokeWidth={1.9} aria-hidden="true" />
      {/if}
      <span class="sr-only">{modeLabel}</span>
    </Button>
  </div>

  {#if showPreviewControls}
    <div
      class="window-control-group relative z-10 ml-1"
      data-preview-window-controls="visible"
      role="group"
      aria-label={titlebarMessages.window_controls_preview()}
    >
      <button
        type="button"
        class="window-control"
        data-window-action="minimize"
        aria-label={titlebarMessages.window_minimize()}
        title={titlebarMessages.window_minimize()}
        onclick={() => runWindowAction('minimize')}
      >
        <span class="window-glyph window-glyph-minimize" aria-hidden="true"></span>
      </button>
      <button
        type="button"
        class="window-control"
        data-window-action="toggle-maximize"
        aria-label={titlebarMessages.window_toggle_maximize()}
        title={titlebarMessages.window_toggle_maximize()}
        onclick={() => runWindowAction('toggle-maximize')}
      >
        <span class="window-glyph window-glyph-maximize" aria-hidden="true"></span>
      </button>
      <button
        type="button"
        class="window-control"
        data-window-action="close"
        aria-label={titlebarMessages.window_close()}
        title={titlebarMessages.window_close()}
        onclick={() => runWindowAction('close')}
      >
        <span class="window-glyph window-glyph-close" aria-hidden="true"></span>
      </button>
    </div>
  {/if}
</header>

<style>
  .titlebar-surface::before {
    position: absolute;
    inset: 0;
    pointer-events: none;
    content: '';
    opacity: 0.026;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='80' height='80' viewBox='0 0 80 80'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='.85' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='80' height='80' filter='url(%23n)' opacity='.55'/%3E%3C/svg%3E");
    mix-blend-mode: overlay;
  }

  .window-control-group {
    display: flex;
    height: 2rem;
    overflow: hidden;
    color: var(--ink-muted);
    background: color-mix(in oklab, var(--surface-2) 88%, var(--canvas));
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
  }

  .window-control {
    position: relative;
    display: grid;
    width: 2.35rem;
    height: 100%;
    place-items: center;
    color: inherit;
    outline: none;
    transition:
      color var(--motion-duration-fast) var(--motion-standard),
      background-color var(--motion-duration-fast) var(--motion-standard),
      transform var(--motion-duration-fast) var(--motion-standard);
  }

  .window-control + .window-control {
    border-left: 1px solid var(--hairline);
  }

  .window-control:hover,
  .window-control:focus-visible {
    color: var(--ink);
    background: var(--lantern-soft);
  }

  .window-control:active {
    transform: scale(0.97);
  }

  .window-control[data-window-action='close']:hover,
  .window-control[data-window-action='close']:focus-visible {
    color: var(--danger);
    background: color-mix(in oklab, var(--danger) 16%, transparent);
  }

  .window-glyph {
    position: relative;
    display: block;
    width: 0.86rem;
    height: 0.86rem;
  }

  .window-glyph::before,
  .window-glyph::after {
    position: absolute;
    inset: 0;
    margin: auto;
    content: '';
    background: currentColor;
  }

  .window-glyph-minimize::before {
    width: 0.7rem;
    height: 1.5px;
  }

  .window-glyph-minimize::after,
  .window-glyph-maximize::after {
    display: none;
  }

  .window-glyph-maximize::before {
    width: 0.62rem;
    height: 0.62rem;
    background: transparent;
    border: 1.5px solid currentColor;
    border-radius: 2px;
  }

  .window-glyph-close::before,
  .window-glyph-close::after {
    width: 0.72rem;
    height: 1.5px;
  }

  .window-glyph-close::before {
    transform: rotate(45deg);
  }

  .window-glyph-close::after {
    transform: rotate(-45deg);
  }
</style>
