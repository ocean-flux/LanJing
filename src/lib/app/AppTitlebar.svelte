<script lang="ts">
  import { m } from '$lib/i18n';
  import type { NativeWindowControlMode } from './shell-types';
  import WindowControls from './WindowControls.svelte';
  import { shouldRenderHtmlWindowControls } from './window-controls';

  type Props = {
    /** 仅 a11y；不渲染可见标题文案 */
    contextLabel?: string;
    compact?: boolean;
    nativeControlMode?: NativeWindowControlMode;
  };

  // paraglide HMR 可能短暂缺键：避免 m.xxx is not a function 打断壳渲染
  const messages = m as typeof m & {
    titlebar_dblclick_maximize?: () => string;
  };

  let {
    contextLabel = m.nav_realm(),
    compact = false,
    nativeControlMode: controlledNativeControlMode,
  }: Props = $props();

  const dblclickHint = $derived(
    typeof messages.titlebar_dblclick_maximize === 'function'
      ? messages.titlebar_dblclick_maximize()
      : '',
  );

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
  const titlebarControlLeft = $derived(nativeControlMode === 'macos-overlay' ? '86px' : '0px');
  const showHtmlWindowControls = $derived(shouldRenderHtmlWindowControls(nativeControlMode));

  /** 双击拖拽带 = 最大化/还原（创意替代常驻窗控） */
  async function onDragDblClick() {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().toggleMaximize();
    } catch {
      // 浏览器预览无窗口 API
    }
  }
</script>

<!--
  持久标题带：
  - Windows/Linux/browser 显示 HTML 窗控；系统装饰与 macOS 使用原生控件
  - 双击拖拽区切换最大化
  - macOS 左侧预留系统交通灯间距
-->
<header
  class={[
    'titlebar-strip motion-reader-recede relative z-30 flex shrink-0 items-center',
    compact ? 'h-7' : 'h-8',
  ]}
  style:padding-left="var(--titlebar-control-left, 0px)"
  style:--titlebar-control-left={titlebarControlLeft}
  aria-label={m.titlebar_label({ context: contextLabel })}
  aria-describedby="titlebar-native-controls"
  data-native-window-controls={nativeControlMode}
  data-titlebar-chrome="persistent"
  data-titlebar-controls={showHtmlWindowControls ? 'html' : 'native'}
>
  <span id="titlebar-native-controls" class="sr-only">{m.titlebar_native_controls()}</span>

  <div
    class="titlebar-drag min-h-full min-w-0 flex-1 self-stretch"
    data-tauri-drag-region
    ondblclick={onDragDblClick}
    role="presentation"
  >
    <span class="sr-only" data-tauri-drag-region>
      {m.app_name()} / {contextLabel}{dblclickHint ? `. ${dblclickHint}` : ''}
    </span>
  </div>
  {#if showHtmlWindowControls}
    <div class="titlebar-controls mr-1.5 shrink-0">
      <WindowControls {nativeControlMode} />
    </div>
  {/if}
</header>

<style>
  .titlebar-strip {
    background: transparent;
  }

  :global(:root[data-material-transparency='low']) .titlebar-strip,
  :global(:root.low-transparency) .titlebar-strip,
  :global([data-reduced-transparency='true']) .titlebar-strip {
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }

  .titlebar-drag {
    -webkit-app-region: drag;
  }
</style>
