<script lang="ts">
  import { AppLaunch } from '$lib/components/brand';
  import { Toaster } from '$lib/components/ui/sonner';
  import { m } from '$lib/i18n';
  import type { Snippet } from 'svelte';
  import AppBottomNav from './AppBottomNav.svelte';
  import AppRail from './AppRail.svelte';
  import AppSearchOverlay from './AppSearchOverlay.svelte';
  import AppTitlebar from './AppTitlebar.svelte';
  import MiniPlayerSlot from './MiniPlayerSlot.svelte';
  import { resolveShellMode, usesBottomNav, usesDesktopRail, usesIconRail } from './shell-mode';
  import type { ModeShellContract } from './shell-types';

  type Props = {
    children?: Snippet;
    /** Required product shell contract from ModeShell (single source of truth). */
    shell: ModeShellContract;
  };

  let { children, shell }: Props = $props();
  let showLaunch = $state(true);
  let searchOpen = $state(false);

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
  data-ambient-audio={shell.ambientAudio?.state ?? 'none'}
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
          nativeControlMode={shell.platform.windowControls}
          themeMode={shell.theme.mode}
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
