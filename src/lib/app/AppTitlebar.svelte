<script lang="ts">
  import { m } from '$lib/i18n';
  import type { NativeWindowControlMode } from './shell-types';

  type WindowAction = 'minimize' | 'toggle-maximize' | 'close';
  type Props = {
    contextLabel?: string;
    compact?: boolean;
    nativeControlMode?: NativeWindowControlMode;
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
  }: Props = $props();

  /** 仅测试：AppShell 未传入 shell.platform.windowControls 时的回退。 */
  function resolveNativeControlModeFallback(): NativeWindowControlMode {
    if (typeof window === 'undefined') return 'browser-preview';

    const platform = navigator.userAgent.toLowerCase();
    const tauri = '__TAURI_INTERNALS__' in window || platform.includes('tauri');

    if (!tauri) return 'browser-preview';
    if (platform.includes('mac')) return 'macos-overlay';
    return 'windows-overlay';
  }

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
  极简 titlebar：上下文 + 窗控。
  无搜索 / 设置 / 主题循环（设置页专责）。
  拖拽：仅 data-tauri-drag-region 区；控件簇 no-drag。
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
        <span class="truncate" data-tauri-drag-region>{m.app_name()} / {contextLabel}</span>
      </div>
      <div class="min-h-full min-w-4 flex-1 self-stretch" data-tauri-drag-region></div>
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
