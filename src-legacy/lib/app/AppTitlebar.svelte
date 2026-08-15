<script lang="ts">
  import { resolve } from '$app/paths';
  import { LanJingMark } from '$lib/components/brand';
  import Icon from '$lib/components/Icon.svelte';
  import { m } from '$lib/i18n';
  import { getPrimaryNavigationItems } from './shell-navigation';
  import type { NativeWindowControlMode, ShellRoute } from './shell-types';
  import WindowControls from './WindowControls.svelte';
  import { shouldRenderHtmlWindowControls } from './window-controls';

  type Props = {
    contextLabel?: string;
    compact?: boolean;
    nativeControlMode?: NativeWindowControlMode;
    active?: ShellRoute;
    settingsActive?: boolean;
  };

  let {
    contextLabel = m.nav_realm(),
    compact = false,
    nativeControlMode = 'browser-preview',
    active,
    settingsActive = false,
  }: Props = $props();

  const navItems = $derived(getPrimaryNavigationItems());
  const showHtmlWindowControls = $derived(
    !compact && shouldRenderHtmlWindowControls(nativeControlMode),
  );
  // macOS Overlay 交通灯叠在窗口左上；与 compact 无关，始终预留，避免挡品牌/导航/标题。
  const trafficLightInset = $derived(nativeControlMode === 'macos-overlay' ? '86px' : '0px');
  async function onDragDblClick() {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().toggleMaximize();
    } catch {
      // 浏览器预览没有原生窗口；双击空白拖拽区保持无副作用。
    }
  }
</script>

<header
  class="glass-chrome relative z-(--layer-chrome) flex h-(--density-control-lg) shrink-0 items-stretch border-b border-hairline text-ink"
  style:padding-left={trafficLightInset}
  aria-label={m.titlebar_label({ context: contextLabel })}
  data-native-window-controls={nativeControlMode}
  data-titlebar-chrome={compact ? 'app-bar' : 'desktop'}
  data-titlebar-controls={showHtmlWindowControls ? 'html' : 'native'}
  data-macos-traffic-light-safe={trafficLightInset !== '0px' ? 'true' : undefined}
>
  {#if compact}
    <div class="flex min-w-0 flex-1 items-center px-(--page-gutter)">
      <span class="truncate text-sm font-semibold">{contextLabel}</span>
    </div>
  {:else}
    <a
      href={resolve('/')}
      class="titlebar-no-drag inline-flex shrink-0 items-center gap-1.5 px-2.5 text-sm font-semibold outline-none hover:bg-surface-2 focus-visible:shadow-[inset_var(--focus-ring)]"
      aria-label={m.app_name()}
    >
      <LanJingMark size={20} label={m.app_name()} />
      <span>{m.app_name()}</span>
    </a>

    <nav class="titlebar-no-drag flex h-full items-stretch" aria-label={m.nav_main()}>
      {#each navItems as item (item.key)}
        <a
          href={resolve(item.href)}
          class={[
            'relative inline-flex h-full items-center gap-1.5 px-2.5 text-sm font-medium text-ink-muted outline-none hover:bg-surface-2 hover:text-ink focus-visible:shadow-[inset_var(--focus-ring)]',
            active === item.key && !settingsActive && 'text-ink',
          ]}
          aria-current={active === item.key && !settingsActive ? 'page' : undefined}
        >
          <Icon name={item.icon} class="size-4" />
          <span>{item.label}</span>
          {#if active === item.key && !settingsActive}
            <span
              class="absolute inset-x-2 bottom-0 h-0.5 bg-lantern"
              aria-hidden="true"
              data-titlebar-active-indicator
            ></span>
          {/if}
        </a>
      {/each}
      <a
        href={resolve('/settings' as '/')}
        class={[
          'relative inline-flex h-full shrink-0 items-center gap-1.5 px-2.5 text-sm font-medium text-ink-muted outline-none hover:bg-surface-2 hover:text-ink focus-visible:shadow-[inset_var(--focus-ring)]',
          settingsActive && 'text-ink',
        ]}
        aria-current={settingsActive ? 'page' : undefined}
      >
        <Icon name="gear-six" class="size-4" />
        <span>{m.settings()}</span>
        {#if settingsActive}
          <span
            class="absolute inset-x-2 bottom-0 h-0.5 bg-lantern"
            aria-hidden="true"
            data-titlebar-active-indicator
          ></span>
        {/if}
      </a>
    </nav>

    <div
      class="titlebar-drag min-w-6 flex-1"
      data-tauri-drag-region
      ondblclick={onDragDblClick}
      role="presentation"
    ></div>

    {#if showHtmlWindowControls}
      <WindowControls {nativeControlMode} />
    {/if}
  {/if}
</header>

<style>
  .titlebar-drag {
    -webkit-app-region: drag;
  }

  .titlebar-no-drag {
    -webkit-app-region: no-drag;
  }
</style>
