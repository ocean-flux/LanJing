<script lang="ts">
  import { m } from '$lib/i18n';
  import type { NativeWindowControlMode } from './shell-types';
  import { shouldRenderHtmlWindowControls } from './window-controls';

  type WindowAction = 'minimize' | 'toggle-maximize' | 'close';

  type Props = {
    nativeControlMode: NativeWindowControlMode;
  };

  let { nativeControlMode }: Props = $props();
  const visible = $derived(shouldRenderHtmlWindowControls(nativeControlMode));

  /** 与 Win11 原生标题栏一致：最大化后切换为还原字形。 */
  let maximized = $state(false);
  let unlistenResized: (() => void) | undefined;

  $effect(() => {
    if (!visible) {
      unlistenResized?.();
      unlistenResized = undefined;
      return;
    }

    let cancelled = false;

    void (async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const appWindow = getCurrentWindow();
        if (cancelled) return;

        maximized = await appWindow.isMaximized();
        unlistenResized = await appWindow.onResized(async () => {
          maximized = await appWindow.isMaximized();
        });
      } catch {
        // 浏览器预览不挂原生监听；保持默认未最大化字形。
      }
    })();

    return () => {
      cancelled = true;
      unlistenResized?.();
      unlistenResized = undefined;
    };
  });

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
        maximized = await appWindow.isMaximized();
        return;
      }

      await appWindow.close();
    } catch (error) {
      // 浏览器预览不渲染本控件；这里仅记录真实宿主权限或调用失败。
      console.warn('[window-controls]', action, error);
    }
  }
</script>

{#if visible}
  <div
    class="titlebar-no-drag flex h-full shrink-0 items-stretch"
    data-window-controls-source="html"
    role="group"
    aria-label={m.window_controls_open()}
  >
    <button
      type="button"
      class="window-control"
      data-window-action="minimize"
      aria-label={m.window_minimize()}
      title={m.window_minimize()}
      onclick={() => runWindowAction('minimize')}
    >
      <!-- Win11 比例：约 10px 字形、细描边 -->
      <svg class="window-icon" viewBox="0 0 10 10" aria-hidden="true" focusable="false">
        <path d="M1 5h8" fill="none" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
    <button
      type="button"
      class="window-control"
      data-window-action="toggle-maximize"
      data-window-maximized={maximized ? 'true' : 'false'}
      aria-label={m.window_toggle_maximize()}
      title={m.window_toggle_maximize()}
      onclick={() => runWindowAction('toggle-maximize')}
    >
      {#if maximized}
        <!-- 还原：后框只露顶/右边，前框完整 -->
        <svg class="window-icon" viewBox="0 0 10 10" aria-hidden="true" focusable="false">
          <path d="M3 1.5h5.5v5.5" fill="none" stroke="currentColor" stroke-width="1" />
          <rect
            x="1.5"
            y="3"
            width="5.5"
            height="5.5"
            fill="none"
            stroke="currentColor"
            stroke-width="1"
          />
        </svg>
      {:else}
        <svg class="window-icon" viewBox="0 0 10 10" aria-hidden="true" focusable="false">
          <rect
            x="1.5"
            y="1.5"
            width="7"
            height="7"
            fill="none"
            stroke="currentColor"
            stroke-width="1"
          />
        </svg>
      {/if}
    </button>
    <button
      type="button"
      class="window-control window-control-close"
      data-window-action="close"
      aria-label={m.window_close()}
      title={m.window_close()}
      onclick={() => runWindowAction('close')}
    >
      <svg class="window-icon" viewBox="0 0 10 10" aria-hidden="true" focusable="false">
        <path
          d="M2.2 2.2l5.6 5.6M7.8 2.2l-5.6 5.6"
          fill="none"
          stroke="currentColor"
          stroke-width="1"
        />
      </svg>
    </button>
  </div>
{/if}

<style>
  .titlebar-no-drag {
    -webkit-app-region: no-drag;
  }

  .window-control {
    display: grid;
    width: 46px;
    height: 100%;
    place-items: center;
    border: 0;
    border-radius: 0;
    background: transparent;
    color: var(--ink-muted);
    outline: none;
    cursor: default;
  }

  .window-control:hover,
  .window-control:focus-visible {
    background: var(--surface-2);
    color: var(--ink);
  }

  .window-control:focus-visible {
    box-shadow: inset var(--focus-ring);
  }

  .window-control-close:hover,
  .window-control-close:focus-visible,
  .window-control-close:active {
    background: var(--danger);
    color: var(--destructive-foreground);
  }

  .window-icon {
    display: block;
    /* Win11 caption glyph 约 10px；靠细描边贴近原生，不靠放大 */
    width: 10px;
    height: 10px;
  }
</style>
