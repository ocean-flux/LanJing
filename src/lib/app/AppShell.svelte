<script lang="ts">
  import { browser } from '$app/environment';
  import { page } from '$app/state';
  import { AppLaunch } from '$lib/components/brand';
  import { Toaster } from '$lib/components/ui/sonner';
  import { m } from '$lib/i18n';
  import { syncMaterialTransparencyForA11y } from '$lib/stores/theme.svelte';
  import type { Snippet } from 'svelte';
  import AppBottomNav from './AppBottomNav.svelte';
  import AppRail from './AppRail.svelte';
  import AppSearchOverlay from './AppSearchOverlay.svelte';
  import AppTitlebar from './AppTitlebar.svelte';
  import {
    markColdLaunchSessionShown,
    readColdLaunchSessionShown,
    shouldShowColdLaunch,
  } from './cold-launch';
  import MiniPlayerSlot from './MiniPlayerSlot.svelte';
  import { resolveShellMode, usesBottomNav, usesDesktopRail, usesIconRail } from './shell-mode';
  import type { ModeShellContract } from './shell-types';

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
  let searchOpen = $state(false);

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
  const activeRoute = $derived(shell.productContext);
  // 设置非四境：titlebar 文案单独覆盖，不扩 ShellRoute
  const isSettingsRoute = $derived(page.url.pathname.startsWith('/settings'));
  const contextLabel = $derived(
    isSettingsRoute
      ? m.settings()
      : activeRoute === 'apps'
        ? m.nav_apps()
        : activeRoute === 'sources'
          ? m.nav_sources()
          : activeRoute === 'library'
            ? m.nav_library()
            : m.nav_realm(),
  );
  const readerMode = $derived(
    shell.presentation === 'reader' || shell.foregroundActivity.kind === 'reader',
  );
  const shellPresentation = $derived(readerMode ? 'reader' : shell.presentation);
  // 设置页：隐藏侧栏，拉宽偏好列表；titlebar 仍显示「设置」
  const showRail = $derived(
    !readerMode && !isSettingsRoute && (usesIconRail(shellMode) || usesDesktopRail(shellMode)),
  );
  const showTitlebar = $derived(
    !readerMode && shellMode !== 'mobile' && shellMode !== 'tablet-portrait',
  );
  const showBottomNav = $derived(!readerMode && usesBottomNav(shellMode));
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
</script>

<div
  class="relative grid h-[100dvh] min-w-0 grid-rows-[1fr] overflow-hidden bg-canvas text-ink"
  data-testid="mode-shell"
  data-route={activeRoute}
  data-product-context={shell.productContext}
  data-media-space={shell.mediaSpace ?? 'none'}
  data-foreground-activity={`${shell.foregroundActivity.kind}${shell.foregroundActivity.id ? `:${shell.foregroundActivity.id}` : ''}`}
  data-presentation={shellPresentation}
  data-shell-mode={shellMode}
  data-platform={shell.platform.kind}
  data-orientation={shell.platform.orientation}
  data-theme-mode={shell.theme.mode}
  data-appearance-pack={shell.theme.appearancePack}
  data-reduced-motion={shell.theme.reducedMotion ? 'true' : 'false'}
  data-reduced-transparency={shell.theme.reducedTransparency ? 'true' : 'false'}
  data-ambient-audio={shell.ambientAudio?.state ?? 'none'}
>
  <div class="relative z-10 flex min-h-0">
    {#if showRail}
      <AppRail active={activeRoute} compact={usesIconRail(shellMode)} />
    {/if}

    <!-- flex-col：无论 titlebar/bottom 槽是否挂载，main 都保持 flex-1 -->
    <div class="flex min-h-0 min-w-0 flex-1 flex-col">
      {#if showTitlebar}
        <AppTitlebar
          {contextLabel}
          compact={shellMode === 'tablet-landscape' || shellMode === 'narrow-desktop'}
          nativeControlMode={shell.platform.windowControls}
          themeMode={shell.theme.mode}
          onsearch={() => (searchOpen = true)}
        />
      {/if}

      <main
        class={[
          'min-h-0 flex-1 overflow-auto scroll-smooth motion-reduce:scroll-auto',
          readerMode
            ? 'bg-transparent px-0 py-0'
            : isSettingsRoute
              ? 'bg-canvas px-3 py-2 md:px-4 md:py-3'
              : 'bg-canvas px-[var(--page-padding-mobile)] py-4 md:px-[var(--page-padding-tablet)] md:py-5 xl:px-[var(--page-padding-desktop)] xl:py-6',
        ]}
      >
        <div
          class={[
            !readerMode &&
              (isSettingsRoute
                ? 'mx-auto w-full max-w-xl'
                : 'mx-auto w-full max-w-[var(--content-max-width)]'),
          ]}
        >
          {#if children}
            {@render children()}
          {/if}
        </div>
      </main>

      <MiniPlayerSlot state={miniPlayerState} presentation={shellPresentation} />
      <AppBottomNav
        active={activeRoute}
        hidden={!showBottomNav}
        onsearch={() => (searchOpen = true)}
      />
    </div>
  </div>
</div>

<AppSearchOverlay open={searchOpen} hasSources={false} onclose={() => (searchOpen = false)} />
<AppLaunch
  visible={showLaunch}
  durationMs={1800}
  oncomplete={() => {
    showLaunch = false;
  }}
/>
<Toaster />
