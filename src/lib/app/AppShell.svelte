<script lang="ts">
  import { page } from '$app/state';
  import { AppLaunch } from '$lib/components/brand';
  import { Toaster } from '$lib/components/ui/sonner';
  import { m } from '$lib/i18n';
  import { getMode } from '$lib/stores/theme.svelte';
  import type { Snippet } from 'svelte';
  import AppBottomNav from './AppBottomNav.svelte';
  import AppRail from './AppRail.svelte';
  import AppSearchOverlay from './AppSearchOverlay.svelte';
  import AppTitlebar from './AppTitlebar.svelte';
  import MiniPlayerSlot from './MiniPlayerSlot.svelte';
  import {
    resolvePlatformCapabilities,
    resolveShellMode,
    usesBottomNav,
    usesDesktopRail,
    usesIconRail,
  } from './shell-mode';
  import type {
    AmbientAudioSession,
    HoverKind,
    ModeShellContract,
    PointerKind,
    ShellPresentationMode,
    ShellRoute,
  } from './shell-types';

  type Props = {
    children?: Snippet;
    presentation?: ShellPresentationMode;
    shell?: ModeShellContract;
  };

  let { children, presentation = 'normal', shell }: Props = $props();
  let showLaunch = $state(true);
  let searchOpen = $state(false);
  let viewportWidth = $state(typeof window === 'undefined' ? 1280 : window.innerWidth);
  let viewportHeight = $state(typeof window === 'undefined' ? 800 : window.innerHeight);

  const hoverKind: HoverKind =
    typeof window !== 'undefined' && window.matchMedia('(hover: hover)').matches ? 'hover' : 'none';
  const pointerKind: PointerKind =
    typeof window !== 'undefined' && window.matchMedia('(pointer: fine)').matches
      ? 'fine'
      : 'coarse';
  const fallbackPlatform = $derived(
    resolvePlatformCapabilities({
      width: viewportWidth,
      height: viewportHeight,
      hover: hoverKind,
      pointer: pointerKind,
    }),
  );
  const fallbackRoute = $derived.by<ShellRoute>(() => {
    const pathname = page.url.pathname;
    if (pathname.startsWith('/apps')) return 'apps';
    if (pathname.startsWith('/sources')) return 'sources';
    if (pathname.startsWith('/library')) return 'library';
    return 'realm';
  });
  const fallbackAudio: AmbientAudioSession = null;
  const shellState = $derived(
    shell ?? {
      productContext: fallbackRoute,
      mediaSpace: null,
      foregroundActivity: { kind: 'browse' as const, id: fallbackRoute },
      presentation,
      platform: fallbackPlatform,
      theme: {
        mode: getMode(),
        reducedMotion: false,
        reducedTransparency: false,
      },
      ambientAudio: fallbackAudio,
    },
  );
  const shellMode = $derived(
    resolveShellMode({
      width: shellState.platform.viewportWidth,
      hover: shellState.platform.hover,
      pointer: shellState.platform.pointer,
    }),
  );
  const activeRoute = $derived(shellState.productContext);
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
    shellState.presentation === 'reader' || shellState.foregroundActivity.kind === 'reader',
  );
  const shellPresentation = $derived(readerMode ? 'reader' : shellState.presentation);
  const showRail = $derived(!readerMode && (usesIconRail(shellMode) || usesDesktopRail(shellMode)));
  const showTitlebar = $derived(
    !readerMode && shellMode !== 'mobile' && shellMode !== 'tablet-portrait',
  );
  const showBottomNav = $derived(!readerMode && usesBottomNav(shellMode));
  const miniPlayerState = $derived(
    shellState.ambientAudio
      ? {
          reserved: true,
          visible: shellState.ambientAudio.state === 'playing',
          label: shellState.ambientAudio.label,
        }
      : { reserved: true, visible: false, label: m.mini_player_empty() },
  );
</script>

<svelte:window bind:innerWidth={viewportWidth} bind:innerHeight={viewportHeight} />

<div
  class="relative grid h-[100dvh] min-w-0 grid-rows-[1fr] overflow-hidden bg-canvas text-ink"
  data-testid="mode-shell"
  data-route={activeRoute}
  data-product-context={shellState.productContext}
  data-media-space={shellState.mediaSpace ?? 'none'}
  data-foreground-activity={`${shellState.foregroundActivity.kind}${shellState.foregroundActivity.id ? `:${shellState.foregroundActivity.id}` : ''}`}
  data-presentation={shellPresentation}
  data-shell-mode={shellMode}
  data-platform={shellState.platform.kind}
  data-orientation={shellState.platform.orientation}
  data-theme-mode={shellState.theme.mode}
  data-ambient-audio={shellState.ambientAudio?.state ?? 'none'}
>
  <div class="relative z-10 flex min-h-0">
    {#if showRail}
      <AppRail active={activeRoute} compact={usesIconRail(shellMode)} />
    {/if}

    <div class="grid min-h-0 min-w-0 flex-1 grid-rows-[auto_1fr_auto_auto]">
      {#if showTitlebar}
        <AppTitlebar
          {contextLabel}
          compact={shellMode === 'tablet-landscape' || shellMode === 'narrow-desktop'}
          nativeControlMode={shellState.platform.windowControls}
          themeMode={shellState.theme.mode}
          onsearch={() => (searchOpen = true)}
        />
      {/if}

      <main
        class={[
          'min-h-0 overflow-auto scroll-smooth motion-reduce:scroll-auto',
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
<AppLaunch visible={showLaunch} durationMs={1800} oncomplete={() => (showLaunch = false)} />
<Toaster />
