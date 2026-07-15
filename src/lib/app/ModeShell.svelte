<script lang="ts">
  import { page } from '$app/state';
  import { getMode } from '$lib/stores/theme.svelte';
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
        reducedMotion:
          typeof window !== 'undefined' &&
          window.matchMedia('(prefers-reduced-motion: reduce)').matches,
        reducedTransparency:
          typeof window !== 'undefined' &&
          window.matchMedia('(prefers-reduced-transparency: reduce)').matches,
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
