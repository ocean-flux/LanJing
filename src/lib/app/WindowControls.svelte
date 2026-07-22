<script lang="ts">
  import { m } from '$lib/i18n';
  import type { NativeWindowControlMode } from './shell-types';
  import { shouldRenderHtmlWindowControls } from './window-controls';

  type WindowAction = 'minimize' | 'toggle-maximize' | 'close';

  type Props = {
    /** 宿主传入壳契约中的窗控模式。 */
    nativeControlMode: NativeWindowControlMode;
  };

  let { nativeControlMode }: Props = $props();
  const visible = $derived(shouldRenderHtmlWindowControls(nativeControlMode));

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
    } catch (error) {
      // 浏览器预览无窗口 API；Tauri 缺权限时也会落到这里。
      console.warn('[window-controls]', action, error);
    }
  }
</script>

{#if visible}
  <div
    class="window-inline titlebar-no-drag flex items-center gap-1"
    data-preview-window-controls="visible"
    data-window-controls-source="html"
    data-window-controls-layout="inline"
    role="group"
    aria-label={m.window_controls_preview()}
  >
    <button
      type="button"
      class="window-control"
      data-window-action="minimize"
      aria-label={m.window_minimize()}
      title={m.window_minimize()}
      onclick={() => runWindowAction('minimize')}
    >
      <svg class="window-icon" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
        <path
          d="M2.5 6h7"
          fill="none"
          stroke="currentColor"
          stroke-width="1.2"
          stroke-linecap="round"
        />
      </svg>
    </button>
    <button
      type="button"
      class="window-control"
      data-window-action="toggle-maximize"
      aria-label={m.window_toggle_maximize()}
      title={m.window_toggle_maximize()}
      onclick={() => runWindowAction('toggle-maximize')}
    >
      <svg class="window-icon" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
        <rect
          x="2.75"
          y="2.75"
          width="6.5"
          height="6.5"
          fill="none"
          stroke="currentColor"
          stroke-width="1.2"
          rx="1.1"
        />
      </svg>
    </button>
    <button
      type="button"
      class="window-control window-control-close"
      data-window-action="close"
      aria-label={m.window_close()}
      title={m.window_close()}
      onclick={() => runWindowAction('close')}
    >
      <svg class="window-icon" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
        <path
          d="M3.25 3.25l5.5 5.5M8.75 3.25l-5.5 5.5"
          fill="none"
          stroke="currentColor"
          stroke-width="1.2"
          stroke-linecap="round"
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
    width: 1.75rem;
    height: 1.75rem;
    place-items: center;
    color: var(--ink-muted);
    background: var(--surface-material);
    border: 1px solid var(--surface-material-border);
    border-radius: 999px;
    box-shadow: var(--surface-control-shadow);
    outline: none;
    cursor: default;
    transition:
      color var(--motion-duration-fast) var(--motion-standard),
      background-color var(--motion-duration-fast) var(--motion-standard),
      border-color var(--motion-duration-fast) var(--motion-standard),
      transform var(--motion-duration-fast) var(--motion-standard);
  }

  .window-control:hover,
  .window-control:focus-visible {
    color: var(--ink);
    border-color: var(--hairline-strong);
    transform: scale(1.04);
  }

  .window-control:focus-visible {
    box-shadow: var(--focus-ring);
  }

  .window-control-close:hover,
  .window-control-close:focus-visible,
  .window-control-close:active {
    color: var(--destructive-foreground);
    background: var(--danger);
    border-color: var(--danger);
  }

  .window-icon {
    display: block;
    width: 10px;
    height: 10px;
    overflow: visible;
  }

  @supports (backdrop-filter: blur(1px)) {
    .window-control {
      backdrop-filter: blur(var(--material-blur)) saturate(var(--material-saturation));
      -webkit-backdrop-filter: blur(var(--material-blur)) saturate(var(--material-saturation));
    }
  }

  :global(:root[data-material-transparency='low']) .window-control,
  :global(:root.low-transparency) .window-control,
  :global([data-reduced-transparency='true']) .window-control {
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }

  @media (prefers-reduced-transparency: reduce) {
    .window-control {
      backdrop-filter: none;
      -webkit-backdrop-filter: none;
    }
  }
</style>
