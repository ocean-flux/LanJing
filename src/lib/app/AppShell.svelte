<script lang="ts">
  import { browser } from '$app/environment';
  import { AppLaunch } from '$lib/components/brand';
  import { Toaster } from '$lib/components/ui/sonner';
  import { m } from '$lib/i18n';
  import { THEME_REGISTRY } from '$lib/stores/appearance-packs';
  import { syncMaterialTransparencyForA11y } from '$lib/stores/theme.svelte';
  import DeeplinkInstallHost from '$lib/views/sources/DeeplinkInstallHost.svelte';
  import type { Snippet } from 'svelte';
  import type { Attachment } from 'svelte/attachments';
  import AppBottomNav from './AppBottomNav.svelte';
  import AppTitlebar from './AppTitlebar.svelte';
  import {
    markColdLaunchSessionShown,
    readColdLaunchSessionShown,
    shouldShowColdLaunch,
  } from './cold-launch';
  import { resolvePrimaryChromeFamily, resolveShellMode } from './shell-mode';
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

  // 系统减少透明度只同步有效材质，不回写用户持久化偏好。
  const syncReducedTransparency: Attachment<HTMLDivElement> = () => {
    syncMaterialTransparencyForA11y(shell.theme.reducedTransparency);
  };

  const shellMode = $derived(
    resolveShellMode({
      width: shell.platform.viewportWidth,
      hover: shell.platform.hover,
      pointer: shell.platform.pointer,
    }),
  );
  const chromeFamily = $derived(resolvePrimaryChromeFamily(shellMode));
  const activeRoute = $derived<ShellRoute | undefined>(
    shell.settingsActive ? undefined : shell.productContext,
  );
  const contextLabel = $derived(
    shell.settingsActive
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
  const compactTitlebar = $derived(chromeFamily === 'bottom');
  const showBottomNav = $derived(!readerMode && chromeFamily === 'bottom');
  const toasterTheme = $derived(THEME_REGISTRY[shell.theme.appearancePack].face);

  function focusMainContent(event: MouseEvent) {
    const mainContent = document.getElementById('main-content');
    if (!(mainContent instanceof HTMLElement)) return;
    event.preventDefault();
    mainContent.focus();
  }
</script>

<div
  {@attach syncReducedTransparency}
  class={[
    'app-canvas grid h-dvh min-w-0 overflow-hidden text-ink',
    readerMode ? 'grid-rows-[minmax(0,1fr)]' : 'grid-rows-[auto_minmax(0,1fr)_auto]',
  ]}
  data-testid="mode-shell"
  data-route={shell.productContext}
  data-product-context={shell.productContext}
  data-settings-active={shell.settingsActive ? 'true' : 'false'}
  data-media-space={shell.mediaSpace ?? 'none'}
  data-foreground-activity={`${shell.foregroundActivity.kind}${shell.foregroundActivity.id ? `:${shell.foregroundActivity.id}` : ''}`}
  data-presentation={readerMode ? 'reader' : shell.presentation}
  data-shell-mode={shellMode}
  data-chrome-family={chromeFamily}
  data-platform={shell.platform.kind}
  data-orientation={shell.platform.orientation}
  data-theme-mode={shell.theme.mode}
  data-appearance-pack={shell.theme.appearancePack}
  data-reduced-motion={shell.theme.reducedMotion ? 'true' : 'false'}
  data-reduced-transparency={shell.theme.reducedTransparency ? 'true' : 'false'}
  data-ambient-audio={shell.ambientAudio?.state ?? 'none'}
>
  <a
    href="#main-content"
    class="fixed top-[calc(var(--safe-area-top)+0.5rem)] left-[calc(var(--safe-area-left)+0.5rem)] z-(--layer-overlay) -m-px h-px w-px overflow-hidden rounded-md border-0 border-hairline-strong bg-surface-1 p-0 whitespace-nowrap text-ink outline-none [clip-path:inset(50%)] focus-visible:m-0 focus-visible:inline-flex focus-visible:h-(--density-control-md) focus-visible:w-auto focus-visible:items-center focus-visible:overflow-visible focus-visible:border focus-visible:px-3 focus-visible:text-sm focus-visible:font-semibold focus-visible:shadow-(--focus-ring) focus-visible:[clip-path:none]"
    onclick={focusMainContent}
    data-skip-link
  >
    {m.shell_skip_to_main()}
  </a>
  {#if !readerMode}
    <AppTitlebar
      {contextLabel}
      compact={compactTitlebar}
      nativeControlMode={shell.platform.windowControls}
      active={activeRoute}
      settingsActive={shell.settingsActive}
    />
  {/if}

  <main
    id="main-content"
    tabindex="-1"
    class={[
      'app-scroll-region min-h-0 min-w-0 overflow-x-hidden overflow-y-auto scroll-smooth motion-reduce:scroll-auto',
      readerMode ? 'bg-transparent p-0' : 'bg-transparent px-(--page-gutter) py-(--section-gap)',
    ]}
    data-app-scroll-region
  >
    {#if children}
      {@render children()}
    {/if}
  </main>

  {#if showBottomNav}
    <AppBottomNav active={activeRoute} settingsActive={shell.settingsActive} />
  {/if}
</div>

<AppLaunch
  visible={showLaunch}
  durationMs={1800}
  oncomplete={() => {
    showLaunch = false;
  }}
/>
<!-- 全局深链导入 Sheet：不新开主导航 / 路由落地页 -->
<DeeplinkInstallHost />
<Toaster theme={toasterTheme} />
