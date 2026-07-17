<script lang="ts">
  import { browser } from '$app/environment';
  import { page } from '$app/state';
  import { resolve } from '$app/paths';
  import Settings from '@lucide/svelte/icons/settings';
  import { AppLaunch } from '$lib/components/brand';
  import { Toaster } from '$lib/components/ui/sonner';
  import { m } from '$lib/i18n';
  import { syncMaterialTransparencyForA11y } from '$lib/stores/theme.svelte';
  import type { Snippet } from 'svelte';
  import AppBottomNav from './AppBottomNav.svelte';
  import AppRail from './AppRail.svelte';
  import AppTitlebar from './AppTitlebar.svelte';
  import {
    markColdLaunchSessionShown,
    readColdLaunchSessionShown,
    shouldShowColdLaunch,
  } from './cold-launch';
  import MiniPlayerSlot from './MiniPlayerSlot.svelte';
  import {
    isSettingsPathname,
    resolveActivePrimaryRoute,
    resolvePrimaryChromeFamily,
    resolveShellMode,
  } from './shell-mode';
  import { readRailCollapsed, writeRailCollapsed } from './shell-rail-preference';
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
  // 脊折叠偏好：仅 rail 族有意义；左缘热区负责唤回
  let railCollapsed = $state(browser ? readRailCollapsed(localStorage) : false);

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
  const shellPresentation = $derived(readerMode ? 'reader' : shell.presentation);
  // 设置页保留脊（dim）；折叠时脊宽 0 + 左缘热区
  const showRail = $derived(!readerMode && chromeFamily === 'rail' && !railCollapsed);
  const showRailEdgeHit = $derived(!readerMode && chromeFamily === 'rail' && railCollapsed);
  const showTitlebar = $derived(
    !readerMode && shellMode !== 'mobile' && shellMode !== 'tablet-portrait',
  );
  // 移动顶栏：无桌面 titlebar 时提供上下文 + 设置
  const showMobileToolbar = $derived(
    !readerMode && (shellMode === 'mobile' || shellMode === 'tablet-portrait'),
  );
  const showBottomNav = $derived(!readerMode && chromeFamily === 'bottom');
  // 无环境音频：仅高度座位；有会话（播/停）才露出可交互条。
  const miniPlayerState = $derived(
    shell.ambientAudio
      ? {
          reserved: true,
          visible: true,
          label: shell.ambientAudio.label,
        }
      : { reserved: true, visible: false, label: m.mini_player_reserved() },
  );

  function collapseRail() {
    railCollapsed = true;
    if (browser) writeRailCollapsed(localStorage, true);
  }

  function expandRail() {
    railCollapsed = false;
    if (browser) writeRailCollapsed(localStorage, false);
  }
</script>

<div
  class="relative grid h-[100dvh] min-w-0 grid-rows-[1fr] overflow-hidden bg-canvas text-ink"
  data-testid="mode-shell"
  data-route={shell.productContext}
  data-product-context={shell.productContext}
  data-media-space={shell.mediaSpace ?? 'none'}
  data-foreground-activity={`${shell.foregroundActivity.kind}${shell.foregroundActivity.id ? `:${shell.foregroundActivity.id}` : ''}`}
  data-presentation={shellPresentation}
  data-shell-mode={shellMode}
  data-chrome-family={chromeFamily}
  data-rail-collapsed={railCollapsed ? 'true' : 'false'}
  data-platform={shell.platform.kind}
  data-orientation={shell.platform.orientation}
  data-theme-mode={shell.theme.mode}
  data-appearance-pack={shell.theme.appearancePack}
  data-reduced-motion={shell.theme.reducedMotion ? 'true' : 'false'}
  data-reduced-transparency={shell.theme.reducedTransparency ? 'true' : 'false'}
  data-ambient-audio={shell.ambientAudio?.state ?? 'none'}
>
  <div class="relative z-10 flex min-h-0">
    {#if showRailEdgeHit}
      <!-- 折叠后左缘热区：指针进入即展开脊 -->
      <div
        class="w-2 shrink-0 cursor-e-resize self-stretch"
        data-shell-rail-edge
        role="presentation"
        title={m.rail_expand_edge()}
        onpointerenter={expandRail}
      ></div>
    {/if}

    {#if showRail}
      <AppRail active={activeRoute} settingsActive={isSettingsRoute} oncollapse={collapseRail} />
    {/if}

    <!-- flex-col：无论 titlebar/bottom 槽是否挂载，main 都保持 flex-1 -->
    <div class="flex min-h-0 min-w-0 flex-1 flex-col">
      {#if showTitlebar}
        <AppTitlebar
          {contextLabel}
          compact={shellMode === 'tablet-landscape' || shellMode === 'narrow-desktop'}
          nativeControlMode={shell.platform.windowControls}
        />
      {/if}

      {#if showMobileToolbar}
        <div
          class="motion-reader-recede flex h-11 shrink-0 items-center gap-2 border-b border-hairline bg-surface-1 px-3"
          data-mobile-toolbar
        >
          <span class="min-w-0 flex-1 truncate text-sm font-semibold text-ink">{contextLabel}</span>
          <a
            href={resolve('/settings' as '/')}
            class="inline-flex h-9 w-9 items-center justify-center rounded-lg text-ink-muted outline-none transition-colors hover:bg-lantern-soft hover:text-ink focus-visible:bg-lantern-soft focus-visible:shadow-[var(--focus-ring)]"
            aria-label={m.settings_open()}
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
          readerMode
            ? 'bg-transparent px-0 py-0'
            : isSettingsRoute
              ? 'bg-canvas px-3 py-2 md:px-4 md:py-3'
              : 'bg-canvas px-[var(--page-padding-mobile)] py-3 md:px-[var(--page-padding-tablet)] md:py-4 xl:px-[var(--page-padding-desktop)]',
        ]}
      >
        <!-- 四境首页/设置全宽；阅读 measure 由阅读器自身控制 -->
        <div class={!readerMode ? 'w-full max-w-none' : undefined}>
          {#if children}
            {@render children()}
          {/if}
        </div>
      </main>

      <MiniPlayerSlot state={miniPlayerState} presentation={shellPresentation} />
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
