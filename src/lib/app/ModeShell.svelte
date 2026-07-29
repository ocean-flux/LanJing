<script lang="ts">
  import { beforeNavigate } from '$app/navigation';
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { getAppearancePack, getMode } from '$lib/stores/theme.svelte';
  import { resolveRuntimePlatform, type RuntimePlatform } from './platform-runtime';
  import { setPlatformContext } from './platform-context.svelte';
  import AppShell from './AppShell.svelte';
  import {
    resolveForegroundActivity,
    resolvePlatformCapabilities,
    resolveProductContext,
    isSettingsPathname,
  } from './shell-mode';
  import {
    getActivityOverride,
    getAmbientAudio,
    notifyPathnameChanged,
  } from './shell-session.svelte';
  import type { ModeShellContract } from './shell-types';

  type Props = {
    children?: import('svelte').Snippet;
    /** 最高优先完整契约覆盖（测试 / 显式注入）。 */
    shell?: ModeShellContract;
  };

  let { children, shell }: Props = $props();
  let viewportWidth = $state(typeof window === 'undefined' ? 1280 : window.innerWidth);
  let viewportHeight = $state(typeof window === 'undefined' ? 800 : window.innerHeight);
  let runtimePlatform = $state<RuntimePlatform>('unknown');
  // 系统 a11y 偏好需随 media change 重绑，保证壳层 data-* / 材质与系统一致。
  let reducedMotion = $state(
    typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches,
  );
  let reducedTransparency = $state(
    typeof window !== 'undefined' &&
      window.matchMedia('(prefers-reduced-transparency: reduce)').matches,
  );

  const hover =
    typeof window !== 'undefined' && window.matchMedia('(hover: hover)').matches ? 'hover' : 'none';
  const pointer =
    typeof window !== 'undefined' && window.matchMedia('(pointer: fine)').matches
      ? 'fine'
      : 'coarse';

  const providedRuntimePlatform = $derived<RuntimePlatform>(
    shell ? (shell.platform.kind === 'browser' ? 'unknown' : shell.platform.kind) : runtimePlatform,
  );
  setPlatformContext({
    get platform() {
      return providedRuntimePlatform;
    },
  });

  beforeNavigate((navigation) => {
    if (navigation.from?.url.pathname !== navigation.to?.url.pathname) {
      notifyPathnameChanged();
    }
  });

  onMount(() => {
    let cancelled = false;
    const motionMq = window.matchMedia('(prefers-reduced-motion: reduce)');
    const transparencyMq = window.matchMedia('(prefers-reduced-transparency: reduce)');

    const syncMotion = () => {
      reducedMotion = motionMq.matches;
    };
    const syncTransparency = () => {
      reducedTransparency = transparencyMq.matches;
    };

    syncMotion();
    syncTransparency();
    motionMq.addEventListener('change', syncMotion);
    transparencyMq.addEventListener('change', syncTransparency);

    if (!shell) {
      void resolveRuntimePlatform().then(
        (resolvedPlatform) => {
          if (cancelled || shell) return;
          runtimePlatform = resolvedPlatform;
        },
        () => {
          if (!cancelled && !shell) runtimePlatform = 'unknown';
        },
      );
    }

    return () => {
      cancelled = true;
      motionMq.removeEventListener('change', syncMotion);
      transparencyMq.removeEventListener('change', syncTransparency);
    };
  });

  const platform = $derived(
    resolvePlatformCapabilities({
      width: viewportWidth,
      height: viewportHeight,
      hover,
      pointer,
      kind: runtimePlatform === 'unknown' ? 'browser' : runtimePlatform,
      tauri: runtimePlatform !== 'unknown',
    }),
  );

  // beforeNavigate 由 SvelteKit 随组件释放；runtime 与 media listener 在 onMount teardown 清理。

  const orchestratedShell = $derived.by<ModeShellContract>(() => {
    const pathname = page.url.pathname;
    const override = getActivityOverride();
    const derivedActivity = resolveForegroundActivity(pathname);

    return {
      productContext: resolveProductContext(pathname),
      settingsActive: isSettingsPathname(pathname),
      mediaSpace: null,
      foregroundActivity: override ?? derivedActivity,
      presentation: 'normal',
      platform,
      theme: {
        mode: getMode(),
        appearancePack: getAppearancePack().id,
        reducedMotion,
        reducedTransparency,
      },
      ambientAudio: getAmbientAudio(),
    };
  });

  const activeShell = $derived(shell ?? orchestratedShell);
</script>

<svelte:window bind:innerWidth={viewportWidth} bind:innerHeight={viewportHeight} />

<AppShell shell={activeShell}>
  {#if children}
    {@render children()}
  {/if}
</AppShell>
