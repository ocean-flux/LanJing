<script lang="ts">
  import { browser } from '$app/environment';
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
    /** Required product shell contract from ModeShell (single source of truth). */
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

  // System reduce-transparency → solid material (dataset); restores stored pref when cleared.
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
  const contextLabel = $derived(
    activeRoute === 'apps'
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
  const showRail = $derived(!readerMode && (usesIconRail(shellMode) || usesDesktopRail(shellMode)));
  const showTitlebar = $derived(
    !readerMode && shellMode !== 'mobile' && shellMode !== 'tablet-portrait',
  );
  const showBottomNav = $derived(!readerMode && usesBottomNav(shellMode));
  const miniPlayerState = $derived(
    shell.ambientAudio
      ? {
          reserved: true,
          visible: shell.ambientAudio.state === 'playing',
          label: shell.ambientAudio.label,
        }
      : { reserved: true, visible: false, label: m.mini_player_empty() },
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

    <!-- flex-col so main keeps flex-1 whether titlebar/bottom slots mount -->
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
            : 'bg-canvas px-[var(--page-padding-mobile)] py-4 md:px-[var(--page-padding-tablet)] md:py-5 xl:px-[var(--page-padding-desktop)] xl:py-6',
        ]}
      >
        <div class={[!readerMode && 'mx-auto w-full max-w-[var(--content-max-width)]']}>
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

<AppSearchOverlay open={searchOpen} hasSources={false} onclose={() => (searchOpen = false)} />
<AppLaunch
  visible={showLaunch}
  durationMs={1800}
  oncomplete={() => {
    showLaunch = false;
  }}
/>
<Toaster />
