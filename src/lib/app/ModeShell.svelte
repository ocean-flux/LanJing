<script lang="ts">
  import { page } from '$app/state';
  import { getAppearancePack, getMode } from '$lib/stores/theme.svelte';
  import AppShell from './AppShell.svelte';
  import {
    resolveForegroundActivity,
    resolveMediaSpace,
    resolvePlatformCapabilities,
    resolvePresentation,
    resolveProductContext,
  } from './shell-mode';
  import {
    getActivityOverride,
    getAmbientAudio,
    notifyPathnameChanged,
  } from './shell-session.svelte';
  import type { ModeShellContract } from './shell-types';

  type Props = {
    children?: import('svelte').Snippet;
    /** Highest-priority full contract override (tests / explicit injection). */
    shell?: ModeShellContract;
  };

  let { children, shell }: Props = $props();
  let viewportWidth = $state(typeof window === 'undefined' ? 1280 : window.innerWidth);
  let viewportHeight = $state(typeof window === 'undefined' ? 800 : window.innerHeight);
  let previousPathname = $state<string | undefined>(undefined);
  // Live system a11y prefs — must rebind on media change so shell data-* / material stay honest.
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

  const platform = $derived(
    resolvePlatformCapabilities({ width: viewportWidth, height: viewportHeight, hover, pointer }),
  );

  // Pathname change clears activity override before render; ambientAudio stays owned by session.
  $effect.pre(() => {
    const pathname = page.url.pathname;
    if (previousPathname !== undefined && previousPathname !== pathname) {
      notifyPathnameChanged();
    }
    previousPathname = pathname;
  });

  $effect(() => {
    if (typeof window === 'undefined') return;

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

    return () => {
      motionMq.removeEventListener('change', syncMotion);
      transparencyMq.removeEventListener('change', syncTransparency);
    };
  });

  const orchestratedShell = $derived.by<ModeShellContract>(() => {
    const pathname = page.url.pathname;
    const override = getActivityOverride();
    const derivedActivity = resolveForegroundActivity(pathname);

    return {
      productContext: resolveProductContext(pathname),
      mediaSpace: resolveMediaSpace(pathname),
      foregroundActivity: override ?? derivedActivity,
      presentation: resolvePresentation(pathname),
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
