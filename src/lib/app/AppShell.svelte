<script lang="ts">
  import { browser } from '$app/environment';
  import { page } from '$app/state';
  import { resolve } from '$app/paths';
  import Settings from '@lucide/svelte/icons/settings';
  import { AppLaunch } from '$lib/components/brand';
  import { Toaster } from '$lib/components/ui/sonner';
  import { m } from '$lib/i18n';
  import { syncMaterialTransparencyForA11y } from '$lib/stores/theme.svelte';
  import { tick, type Snippet } from 'svelte';
  import AppBottomNav from './AppBottomNav.svelte';
  import AppRail from './AppRail.svelte';
  import AppTitlebar from './AppTitlebar.svelte';
  import IslandExpand from './IslandExpand.svelte';
  import {
    markColdLaunchSessionShown,
    readColdLaunchSessionShown,
    shouldShowColdLaunch,
  } from './cold-launch';
  import {
    isSettingsPathname,
    resolveActivePrimaryRoute,
    resolvePrimaryChromeFamily,
    resolveShellMode,
  } from './shell-mode';
  import {
    RAIL_HOVER_CLOSE_MS,
    RAIL_HOVER_OPEN_MS,
    readRailBehavior,
    readRailCollapsed,
    writeRailBehavior,
    writeRailCollapsed,
    type RailBehavior,
  } from './shell-rail-preference';
  import type { ModeShellContract, ShellRoute } from './shell-types';

  type Props = {
    children?: Snippet;
    /** ModeShell 下发的产品壳契约（唯一真相源）。 */
    shell: ModeShellContract;
  };

  let { children, shell }: Props = $props();

  const initialShowLaunch = shouldShowColdLaunch({
    now: typeof performance !== 'undefined' ? performance.now() : 0,
    sessionShown: browser ? readColdLaunchSessionShown(sessionStorage) : false,
  });
  if (initialShowLaunch && browser) {
    markColdLaunchSessionShown(sessionStorage);
  }

  let showLaunch = $state(initialShowLaunch);
  // 侧栏行为：固定 / 悬停（延迟展开）
  let railBehavior = $state<RailBehavior>(browser ? readRailBehavior(localStorage) : 'fixed');
  // fixed 模式折叠偏好；hover 模式用 pointer 状态
  let railCollapsed = $state(browser ? readRailCollapsed(localStorage) : false);
  let railHoverIntent = $state(false);
  let hoverOpenTimer: ReturnType<typeof setTimeout> | null = null;
  let hoverCloseTimer: ReturnType<typeof setTimeout> | null = null;

  // 系统减少透明度 → 实色材质（dataset）；标志清除后恢复已存用户偏好。
  $effect(() => {
    syncMaterialTransparencyForA11y(shell.theme.reducedTransparency);
  });

  const shellMode = $derived(
    resolveShellMode({
      width: shell.platform.viewportWidth,
      hover: shell.platform.hover,
      pointer: shell.platform.pointer,
    }),
  );
  const chromeFamily = $derived(resolvePrimaryChromeFamily(shellMode));
  // 设置非四境：四境 active 清空，脊上设置项单独 current
  const isSettingsRoute = $derived(isSettingsPathname(page.url.pathname));
  const activeRoute = $derived<ShellRoute | undefined>(
    resolveActivePrimaryRoute(page.url.pathname, shell.productContext),
  );
  const contextLabel = $derived(
    isSettingsRoute
      ? m.settings()
      : shell.productContext === 'apps'
        ? m.nav_apps()
        : shell.productContext === 'sources'
          ? m.nav_sources()
          : shell.productContext === 'library'
            ? m.nav_library()
            : m.nav_realm(),
  );
  const readerMode = $derived(
    shell.presentation === 'reader' || shell.foregroundActivity.kind === 'reader',
  );
  // 桌面脊：始终挂载以便宽度动画
  const showRailChrome = $derived(!readerMode && chromeFamily === 'rail');
  const railHoverOpen = $derived(showRailChrome && railHoverIntent);
  const railVisuallyCollapsed = $derived(railBehavior === 'hover' ? !railHoverOpen : railCollapsed);
  const showHoverRailEdge = $derived(
    showRailChrome && railBehavior === 'hover' && railVisuallyCollapsed,
  );
  const showFixedRailRestore = $derived(
    showRailChrome && railBehavior === 'fixed' && railVisuallyCollapsed,
  );
  const macosOverlay = $derived(shell.platform.windowControls === 'macos-overlay');
  // 移动顶栏：上下文 + 设置
  const showMobileToolbar = $derived(
    !readerMode && (shellMode === 'mobile' || shellMode === 'tablet-portrait'),
  );
  const showBottomNav = $derived(!readerMode && chromeFamily === 'bottom');
  // 底岛上浮时为 main 预留滚动内边距，避免内容被遮
  const mainBottomPad = $derived(
    showBottomNav
      ? 'pb-[calc(var(--shell-bottom-nav-height)+var(--shell-bottom-safe-padding)+1.25rem)]'
      : undefined,
  );

  function clearHoverTimers() {
    if (hoverOpenTimer) {
      clearTimeout(hoverOpenTimer);
      hoverOpenTimer = null;
    }
    if (hoverCloseTimer) {
      clearTimeout(hoverCloseTimer);
      hoverCloseTimer = null;
    }
  }

  $effect(() => {
    if (!showRailChrome) return;
    return clearHoverTimers;
  });

  async function collapseRail() {
    if (railBehavior !== 'fixed') return;
    railCollapsed = true;
    if (browser) writeRailCollapsed(localStorage, true);
    await tick();
    document.querySelector<HTMLButtonElement>('[data-shell-rail-restore]')?.focus();
  }

  function expandRail() {
    if (railBehavior === 'hover') {
      clearHoverTimers();
      railHoverIntent = true;
      return;
    }
    railCollapsed = false;
    if (browser) writeRailCollapsed(localStorage, false);
  }

  async function restoreFixedRail() {
    expandRail();
    await tick();
    document.querySelector<HTMLElement>('[data-shell-rail-behavior]')?.focus();
  }

  function setRailBehavior(next: RailBehavior) {
    clearHoverTimers();
    railBehavior = next;
    if (browser) writeRailBehavior(localStorage, next);
    if (next === 'hover') {
      railHoverIntent = false;
    } else {
      railCollapsed = browser ? readRailCollapsed(localStorage) : false;
      railHoverIntent = false;
    }
  }

  function onRailPointerEnter() {
    if (railBehavior !== 'hover') return;
    if (hoverCloseTimer) {
      clearTimeout(hoverCloseTimer);
      hoverCloseTimer = null;
    }
    if (shell.theme.reducedMotion) {
      railHoverIntent = true;
      return;
    }
    if (railHoverOpen || hoverOpenTimer) return;
    hoverOpenTimer = setTimeout(() => {
      railHoverIntent = true;
      hoverOpenTimer = null;
    }, RAIL_HOVER_OPEN_MS);
  }

  function onRailPointerLeave() {
    if (railBehavior !== 'hover') return;
    if (hoverOpenTimer) {
      clearTimeout(hoverOpenTimer);
      hoverOpenTimer = null;
    }
    if (!railHoverOpen) return;
    if (shell.theme.reducedMotion) {
      railHoverIntent = false;
      return;
    }
    hoverCloseTimer = setTimeout(() => {
      railHoverIntent = false;
      hoverCloseTimer = null;
    }, RAIL_HOVER_CLOSE_MS);
  }
</script>

<div
  class="relative grid h-[100dvh] min-w-0 grid-rows-[auto_1fr] overflow-hidden bg-canvas text-ink"
  data-testid="mode-shell"
  data-route={shell.productContext}
  data-product-context={shell.productContext}
  data-media-space={shell.mediaSpace ?? 'none'}
  data-foreground-activity={`${shell.foregroundActivity.kind}${shell.foregroundActivity.id ? `:${shell.foregroundActivity.id}` : ''}`}
  data-presentation={readerMode ? 'reader' : shell.presentation}
  data-shell-mode={shellMode}
  data-chrome-family={chromeFamily}
  data-rail-collapsed={railVisuallyCollapsed ? 'true' : 'false'}
  data-rail-behavior={railBehavior}
  data-platform={shell.platform.kind}
  data-orientation={shell.platform.orientation}
  data-theme-mode={shell.theme.mode}
  data-appearance-pack={shell.theme.appearancePack}
  data-reduced-motion={shell.theme.reducedMotion ? 'true' : 'false'}
  data-reduced-transparency={shell.theme.reducedTransparency ? 'true' : 'false'}
  data-ambient-audio={shell.ambientAudio?.state ?? 'none'}
>
  <!-- 可选 mesh 光晕：仅装饰，不拦截指针；色来自 token（冷青系） -->
  {#if !readerMode}
    <div
      class="shell-mesh pointer-events-none absolute inset-0 z-0"
      aria-hidden="true"
      data-shell-mesh
    ></div>
  {/if}

  {#if !readerMode && !showMobileToolbar}
    <AppTitlebar
      {contextLabel}
      nativeControlMode={shell.platform.windowControls}
      compact={showMobileToolbar}
    />
  {/if}

  <div
    class={['relative z-10 flex min-h-0', macosOverlay && 'shell-macos-workspace-safe']}
    data-macos-traffic-light-safe={macosOverlay ? 'true' : undefined}
  >
    {#if showRailChrome}
      <!-- 固定/悬停共用左缘+侧槽；悬停时 enter/leave 作用在整带上 -->
      <div
        class="relative flex shrink-0 self-stretch"
        data-shell-rail-zone
        data-rail-behavior={railBehavior}
        role="presentation"
        onpointerenter={onRailPointerEnter}
        onpointerleave={onRailPointerLeave}
      >
        {#if showHoverRailEdge}
          <div
            class="rail-edge-hit w-2 shrink-0 cursor-e-resize self-stretch"
            data-shell-rail-edge
            role="presentation"
            title={m.rail_expand_edge()}
          ></div>
        {/if}

        {#if showFixedRailRestore}
          <button
            type="button"
            class="rail-edge-restore titlebar-no-drag m-1.5 inline-flex min-h-11 min-w-11 shrink-0 items-center justify-center self-start rounded-2xl text-sm font-semibold text-ink outline-none focus-visible:shadow-[var(--focus-ring)]"
            aria-label={m.rail_expand_edge()}
            title={m.rail_expand_edge()}
            data-shell-rail-restore
            onclick={restoreFixedRail}
          >
            <span aria-hidden="true">›</span>
          </button>
        {/if}

        <div
          class={[
            'shell-rail-slot relative shrink-0 self-stretch overflow-hidden',
            railVisuallyCollapsed ? 'shell-rail-slot-collapsed' : 'shell-rail-slot-expanded',
            shell.theme.reducedMotion && 'shell-rail-slot-instant',
          ]}
          data-shell-rail-slot={railVisuallyCollapsed ? 'collapsed' : 'expanded'}
          aria-hidden={railVisuallyCollapsed ? 'true' : undefined}
        >
          <AppRail
            active={activeRoute}
            settingsActive={isSettingsRoute}
            {railBehavior}
            oncollapse={collapseRail}
            onbehaviorchange={setRailBehavior}
            nativeControlMode={shell.platform.windowControls}
          />
        </div>
      </div>
    {/if}

    <div class="relative flex min-h-0 min-w-0 flex-1 flex-col">
      {#if showMobileToolbar}
        <div
          class="mobile-toolbar-material motion-reader-recede flex h-11 shrink-0 items-center gap-2 border-b px-3"
          data-mobile-toolbar
          data-tauri-drag-region
        >
          <span class="min-w-0 flex-1 truncate text-sm font-semibold text-ink">{contextLabel}</span>
          <IslandExpand
            active={activeRoute}
            settingsActive={isSettingsRoute}
            nativeControlMode={shell.platform.windowControls}
            triggerVariant="mobile"
          />
          <a
            href={resolve('/settings' as '/')}
            class="titlebar-no-drag inline-flex h-9 w-9 items-center justify-center rounded-lg text-ink-muted outline-none transition-colors hover:bg-lantern-soft hover:text-ink focus-visible:bg-lantern-soft focus-visible:shadow-[var(--focus-ring)]"
            aria-label={m.settings_open()}
            aria-current={isSettingsRoute ? 'page' : undefined}
            title={m.settings()}
            data-mobile-toolbar-settings
          >
            <Settings size={18} strokeWidth={1.75} aria-hidden="true" />
          </a>
        </div>
      {/if}

      <main
        class={[
          'min-h-0 flex-1 overflow-auto scroll-smooth motion-reduce:scroll-auto',
          mainBottomPad,
          readerMode
            ? 'bg-transparent px-0 py-0'
            : isSettingsRoute
              ? 'bg-transparent px-3 py-2 md:px-4 md:py-3'
              : 'bg-transparent px-[var(--page-padding-mobile)] py-3 md:px-[var(--page-padding-tablet)] md:py-4 xl:px-[var(--page-padding-desktop)]',
        ]}
      >
        <!-- 四境首页/设置全宽；阅读 measure 由阅读器自身控制 -->
        <div class={!readerMode ? 'w-full max-w-none' : undefined}>
          {#if children}
            {@render children()}
          {/if}
        </div>
      </main>

      <!-- 本版工作台不挂载 mini-player 消费条 -->
      <AppBottomNav active={activeRoute} hidden={!showBottomNav} />
    </div>
  </div>
</div>

<AppLaunch
  visible={showLaunch}
  durationMs={1800}
  oncomplete={() => {
    showLaunch = false;
  }}
/>
<Toaster />

<style>
  /* 冷青 mesh：抬高不透明度，避免纯黑空洞 */
  .shell-mesh {
    background:
      radial-gradient(
        90% 70% at 8% 12%,
        color-mix(in oklab, var(--mesh-a, var(--lantern)) 55%, transparent),
        transparent 58%
      ),
      radial-gradient(
        70% 55% at 92% 88%,
        color-mix(in oklab, var(--mesh-b, var(--lantern)) 40%, transparent),
        transparent 52%
      ),
      radial-gradient(
        50% 40% at 70% 20%,
        color-mix(in oklab, var(--lantern-soft) 35%, transparent),
        transparent 60%
      );
  }

  /* 侧栏槽：宽度过渡；折叠后槽不占位，恢复钮仍留在槽外。 */
  .shell-rail-slot {
    width: calc(var(--shell-rail-width) + 0.75rem);
    max-width: calc(var(--shell-rail-width) + 0.75rem);
    opacity: 1;
    transition:
      width var(--motion-duration-standard) var(--motion-standard),
      max-width var(--motion-duration-standard) var(--motion-standard),
      opacity var(--motion-duration-standard) var(--motion-standard);
  }

  .shell-rail-slot-collapsed {
    width: 0;
    max-width: 0;
    opacity: 0;
    pointer-events: none;
  }

  .shell-rail-slot-instant {
    transition-duration: 0.01ms;
  }

  .shell-rail-slot :global([data-shell-rail='spine']) {
    min-width: var(--shell-rail-width);
  }
  .shell-macos-workspace-safe {
    padding-left: 86px;
  }

  .titlebar-no-drag {
    -webkit-app-region: no-drag;
  }

  .rail-edge-restore,
  .mobile-toolbar-material {
    border-color: var(--surface-material-border);
    background: var(--surface-material);
  }

  @supports (backdrop-filter: blur(1px)) {
    .rail-edge-restore,
    .mobile-toolbar-material {
      backdrop-filter: blur(var(--material-blur)) saturate(var(--material-saturation));
      -webkit-backdrop-filter: blur(var(--material-blur)) saturate(var(--material-saturation));
    }
  }

  :global(:root[data-material-transparency='low']) .rail-edge-restore,
  :global(:root[data-material-transparency='low']) .mobile-toolbar-material,
  :global(:root.low-transparency) .rail-edge-restore,
  :global(:root.low-transparency) .mobile-toolbar-material,
  [data-reduced-transparency='true'] .rail-edge-restore,
  [data-reduced-transparency='true'] .mobile-toolbar-material {
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }

  @media (prefers-reduced-transparency: reduce) {
    .rail-edge-restore,
    .mobile-toolbar-material {
      backdrop-filter: none;
      -webkit-backdrop-filter: none;
    }
  }
</style>
