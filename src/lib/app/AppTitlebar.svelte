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

  /** 仅测试：AppShell 未传入 shell.platform.windowControls 时的回退。 */
  function resolveNativeControlModeFallback(): NativeWindowControlMode {
    if (typeof window === 'undefined') return 'browser-preview';

    const platform = navigator.userAgent.toLowerCase();
    const tauri = '__TAURI_INTERNALS__' in window || platform.includes('tauri');

    if (!tauri) return 'browser-preview';
    if (platform.includes('mac')) return 'macos-overlay';
    return 'windows-overlay';
  }

  const currentMode = $derived(controlledThemeMode ?? getMode());
  const modeLabel = $derived.by(() => {
    if (currentMode === 'light') return m.theme_mode_light();
    if (currentMode === 'dark') return m.theme_mode_dark();
    return m.theme_mode_system();
  });
  const nativeControlMode = $derived(
    controlledNativeControlMode ?? resolveNativeControlModeFallback(),
  );
  // 官方无边框标题：Windows/Linux/browser 用 HTML 控件；macOS 用系统交通灯。
  const showWindowControls = $derived(
    nativeControlMode === 'browser-preview' ||
      nativeControlMode === 'windows-overlay' ||
      nativeControlMode === 'system-decorated',
  );
  const titlebarControlLeft = $derived(nativeControlMode === 'macos-overlay' ? '86px' : '0px');

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
      // 浏览器预览只保留可视 affordance；真实窗口动作由 Tauri 接管。
    }
  }
</script>

<!--
  无边框拖拽模型：
  - 仅显式拖拽区带 data-tauri-drag-region（标题 + flex 间隔）。
  - header 本身不是拖拽区（避免吞掉标题控件点击）。
  - 交互簇用 titlebar-no-drag，按钮永不启动拖拽。
-->
<header
  class={[
    'titlebar-surface motion-reader-recede relative flex shrink-0 border-b border-hairline bg-surface-1 text-sm',
    showWindowControls ? 'items-stretch' : 'items-center',
    'pl-[calc(var(--titlebar-control-left,0px)+0.75rem)]',
    showWindowControls ? 'pr-0' : 'pr-3',
    compact ? 'h-(--shell-titlebar-compact-height)' : 'h-(--shell-titlebar-height)',
  ]}
  style:--titlebar-control-left={titlebarControlLeft}
  aria-label={m.titlebar_label({ context: contextLabel })}
  aria-describedby="titlebar-native-controls"
  data-native-window-controls={nativeControlMode}
>
  <span id="titlebar-native-controls" class="sr-only">{m.titlebar_native_controls()}</span>

  <div class="relative z-10 flex min-h-0 min-w-0 flex-1 items-center">
    <div
      class="flex min-h-full min-w-0 flex-1 items-center gap-2 self-stretch py-0 pr-2"
      data-tauri-drag-region
    >
      <div class="min-w-0 font-medium text-ink-muted" data-tauri-drag-region>
        <span class="truncate" data-tauri-drag-region>览境 / {contextLabel}</span>
      </div>
      <div class="min-h-full min-w-4 flex-1 self-stretch" data-tauri-drag-region></div>
    </div>

    <div class="titlebar-no-drag relative z-10 flex shrink-0 items-center gap-1 pr-1">
      <Button
        type="button"
        variant="outline"
        size="sm"
        class="motion-command-lens h-8 rounded-md border-hairline bg-surface-1 px-2.5 text-ink-muted hover:bg-surface-3 hover:text-ink"
        aria-label={m.search_open()}
        onclick={onsearch}
      >
        <Search size={14} strokeWidth={1.75} aria-hidden="true" />
        <span class="hidden sm:inline">{m.search()}</span>
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        class="motion-nav-capsule grid h-8 w-8 place-items-center rounded-md p-0 text-ink-muted hover:bg-surface-3 hover:text-ink"
        aria-label={m.theme_mode_toggle({ mode: modeLabel })}
        title={m.theme_mode_toggle({ mode: modeLabel })}
        onclick={cycleThemeMode}
      >
        {#if currentMode === 'light'}
          <Sun size={15} strokeWidth={1.75} aria-hidden="true" />
        {:else if currentMode === 'dark'}
          <Moon size={15} strokeWidth={1.75} aria-hidden="true" />
        {:else}
          <Monitor size={15} strokeWidth={1.75} aria-hidden="true" />
        {/if}
        <span class="sr-only">{modeLabel}</span>
      </Button>
    </div>
  </div>

  {#if showWindowControls}
    <div
      class="window-control-group titlebar-no-drag relative z-20"
      data-preview-window-controls="visible"
      data-window-controls-source="html"
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
        <svg class="window-icon" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
          <path
            d="M2.5 6h7"
            fill="none"
            stroke="currentColor"
            stroke-width="1"
            stroke-linecap="square"
          />
        </svg>
      </button>
      <button
        type="button"
        class="window-control"
        data-window-action="toggle-maximize"
        aria-label={titlebarMessages.window_toggle_maximize()}
        title={titlebarMessages.window_toggle_maximize()}
        onclick={() => runWindowAction('toggle-maximize')}
      >
        <svg class="window-icon" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
          <rect
            x="2.5"
            y="2.5"
            width="7"
            height="7"
            fill="none"
            stroke="currentColor"
            stroke-width="1"
            rx="0.2"
          />
        </svg>
      </button>
      <button
        type="button"
        class="window-control window-control-close"
        data-window-action="close"
        aria-label={titlebarMessages.window_close()}
        title={titlebarMessages.window_close()}
        onclick={() => runWindowAction('close')}
      >
        <svg class="window-icon" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
          <path
            d="M3 3l6 6M9 3l-6 6"
            fill="none"
            stroke="currentColor"
            stroke-width="1"
            stroke-linecap="square"
          />
        </svg>
      </button>
    </div>
  {/if}
</header>

<style>
  .titlebar-no-drag {
    -webkit-app-region: no-drag;
    app-region: no-drag;
  }

  .window-control-group {
    display: flex;
    flex-shrink: 0;
    align-self: stretch;
    height: auto;
    margin: 0;
    color: var(--ink);
    -webkit-app-region: no-drag;
    app-region: no-drag;
  }

  .window-control {
    position: relative;
    display: grid;
    width: 46px;
    height: 100%;
    min-height: 100%;
    place-items: center;
    color: inherit;
    background: transparent;
    border: 0;
    border-radius: 0;
    outline: none;
    cursor: default;
    -webkit-app-region: no-drag;
    app-region: no-drag;
    transition:
      color var(--motion-duration-fast) var(--motion-standard),
      background-color var(--motion-duration-fast) var(--motion-standard);
  }

  .window-control:hover,
  .window-control:focus-visible {
    background: color-mix(in oklab, var(--ink) 7%, transparent);
  }

  .window-control:focus-visible {
    z-index: 1;
    box-shadow: inset 0 0 0 2px color-mix(in oklab, var(--lantern-strong) 45%, transparent);
  }

  .window-control:active {
    background: color-mix(in oklab, var(--ink) 12%, transparent);
  }

  .window-control-close:hover,
  .window-control-close:focus-visible {
    color: #ffffff;
    background: #c42b1c;
  }

  .window-control-close:active {
    color: #ffffff;
    background: color-mix(in srgb, #c42b1c 86%, black);
  }

  .window-icon {
    display: block;
    width: 12px;
    height: 12px;
    overflow: visible;
  }

  :global(.dark) .window-control:hover,
  :global(.dark) .window-control:focus-visible {
    background: color-mix(in oklab, var(--ink) 10%, transparent);
  }

  :global(.dark) .window-control:active {
    background: color-mix(in oklab, var(--ink) 16%, transparent);
  }
</style>
