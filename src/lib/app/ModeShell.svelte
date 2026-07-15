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
  import type { ModeShellContract } from './shell-types';

  type Props = {
    children?: import('svelte').Snippet;
    shell?: ModeShellContract;
  };

  let { children, shell }: Props = $props();
  let viewportWidth = $state(typeof window === 'undefined' ? 1280 : window.innerWidth);
  let viewportHeight = $state(typeof window === 'undefined' ? 800 : window.innerHeight);

  const hover =
    typeof window !== 'undefined' && window.matchMedia('(hover: hover)').matches ? 'hover' : 'none';
  const pointer =
    typeof window !== 'undefined' && window.matchMedia('(pointer: fine)').matches
      ? 'fine'
      : 'coarse';
  const defaultPlatform = $derived(
    resolvePlatformCapabilities({ width: viewportWidth, height: viewportHeight, hover, pointer }),
  );
  const defaultShell = $derived.by<ModeShellContract>(() => {
    const pathname = page.url.pathname;
    return {
      productContext: resolveProductContext(pathname),
      mediaSpace: resolveMediaSpace(pathname),
      foregroundActivity: resolveForegroundActivity(pathname),
      presentation: resolvePresentation(pathname),
      platform: defaultPlatform,
      theme: {
        mode: getMode(),
        reducedMotion:
          typeof window !== 'undefined' &&
          window.matchMedia('(prefers-reduced-motion: reduce)').matches,
        reducedTransparency:
          typeof window !== 'undefined' &&
          window.matchMedia('(prefers-reduced-transparency: reduce)').matches,
      },
      ambientAudio: null,
    };
  });
  const activeShell = $derived(shell ?? defaultShell);
</script>

<svelte:window bind:innerWidth={viewportWidth} bind:innerHeight={viewportHeight} />

<AppShell shell={activeShell}>
  {#if children}
    {@render children()}
  {/if}
</AppShell>
