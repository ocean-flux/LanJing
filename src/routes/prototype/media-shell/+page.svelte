<!-- PROTOTYPE ONLY — 三种 LanJing 媒体壳层变体，单一路由，通过 ?variant=A|B|C 切换。 -->
<script lang="ts">
  import { dev } from '$app/environment';
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { page } from '$app/state';
  import { prefersReducedMotion } from 'svelte/motion';
  import { fade } from 'svelte/transition';
  import PrototypeQueue from './PrototypeQueue.svelte';
  import PrototypeSwitcher from './PrototypeSwitcher.svelte';
  import VariantA from './VariantA.svelte';
  import VariantB from './VariantB.svelte';
  import VariantC from './VariantC.svelte';
  import {
    paletteOptions,
    spaceOptions,
    type PrototypeAudioState,
    type PrototypePalette,
    type PrototypeSpace,
    type PrototypeTheme,
    type PrototypeVariant,
  } from './prototype-fixtures';

  let space = $state<PrototypeSpace>('realm');
  let theme = $state<PrototypeTheme>('dark');
  let palette = $state<PrototypePalette>('tide');
  let audioState = $state<PrototypeAudioState>('playing');
  let queueOpen = $state(false);

  const variant = $derived(normalizeVariant(page.url.searchParams.get('variant')));
  const spaceLabel = $derived(spaceOptions.find((item) => item.id === space)?.label ?? '境场');
  const paletteLabel = $derived(
    paletteOptions.find((item) => item.id === palette)?.label ?? '墨青冰蓝',
  );
  const stateSummary = $derived(
    `${spaceLabel} · ${theme === 'dark' ? '深色' : '浅色'} · ${paletteLabel} · ${
      audioState === 'playing' ? '音频播放' : audioState === 'paused' ? '音频暂停' : '无音频'
    }`,
  );

  function normalizeVariant(value: string | null): PrototypeVariant {
    return value === 'B' || value === 'C' ? value : 'A';
  }

  function setVariant(next: PrototypeVariant): void {
    const url = new URL(page.url);
    url.searchParams.set('variant', next);
    void goto(resolve(`/prototype/media-shell?${url.searchParams.toString()}`), {
      replaceState: true,
      noScroll: true,
      keepFocus: true,
    });
    queueOpen = false;
  }

  function setSpace(next: PrototypeSpace): void {
    space = next;
    queueOpen = false;
  }

  function setAudioState(next: PrototypeAudioState): void {
    audioState = next;
    if (next === 'none') queueOpen = false;
  }
</script>

<svelte:head>
  <title>LanJing 媒体壳层原型</title>
  <meta name="description" content="可丢弃的 LanJing Adaptive Frame 媒体壳层 UI 原型" />
</svelte:head>

{#if dev}
  <div
    class={[
      'prototype-root',
      theme === 'dark' ? 'prototype-dark' : 'prototype-light',
      `prototype-palette-${palette}`,
    ]}
    data-space={space}
    data-variant={variant}
  >
    <div class="prototype-question" aria-hidden="true">
      <span>THROWAWAY UI</span>
      <p>Adaptive Frame · 原创本地 fixture · 无持久化 / 无后端 mutation</p>
    </div>

    {#key variant}
      <div class="variant-stage" in:fade={{ duration: prefersReducedMotion.current ? 0 : 260 }}>
        {#if variant === 'A'}
          <VariantA
            {space}
            {theme}
            {audioState}
            onspacechange={setSpace}
            onthemechange={(next) => (theme = next)}
            onaudiochange={setAudioState}
            onqueueopen={() => (queueOpen = true)}
          />
        {:else if variant === 'B'}
          <VariantB
            {space}
            {theme}
            {audioState}
            onspacechange={setSpace}
            onthemechange={(next) => (theme = next)}
            onaudiochange={setAudioState}
            onqueueopen={() => (queueOpen = true)}
          />
        {:else}
          <VariantC
            {space}
            {theme}
            {audioState}
            onspacechange={setSpace}
            onthemechange={(next) => (theme = next)}
            onaudiochange={setAudioState}
            onqueueopen={() => (queueOpen = true)}
          />
        {/if}
      </div>
    {/key}

    <PrototypeSwitcher
      current={variant}
      {palette}
      {stateSummary}
      onchange={setVariant}
      onpalettechange={(next) => (palette = next)}
    />
    <PrototypeQueue
      open={queueOpen}
      {audioState}
      onclose={() => (queueOpen = false)}
      onaudiochange={setAudioState}
    />
  </div>
{:else}
  <section class="prototype-unavailable">
    <h1>Prototype unavailable</h1>
    <p>此 throwaway 路由只在开发服务器中启用。</p>
  </section>
{/if}

<style>
  .prototype-root {
    --proto-accent: #cf5a3b;
    --proto-accent-soft: rgb(207 90 59 / 0.14);
    --proto-on-accent: #fff8f2;
    --prototype-switcher-bottom: 88px;
    position: fixed;
    inset: 0;
    z-index: 40;
    height: 100dvh;
    overflow: hidden;
    background: var(--proto-canvas);
    color: var(--proto-ink);
    color-scheme: light dark;
    font-family: var(--font-ui, 'Outfit', sans-serif);
  }

  .prototype-dark {
    --proto-canvas: #111316;
    --proto-surface: #17191d;
    --proto-surface-strong: #1e2025;
    --proto-ink: #eceae4;
    --proto-muted: #9b9993;
    --proto-line: rgb(236 234 228 / 0.1);
    --reader-surface: #191613;
    --reader-ink: #ded7cb;
  }

  .prototype-light {
    --proto-canvas: #f1f0ec;
    --proto-surface: #f8f7f3;
    --proto-surface-strong: #ffffff;
    --proto-ink: #202124;
    --proto-muted: #6c6b66;
    --proto-line: rgb(32 33 36 / 0.1);
    --reader-surface: #f3ede1;
    --reader-ink: #26221d;
  }

  .prototype-dark.prototype-palette-tide {
    --proto-canvas: #0d1416;
    --proto-surface: #131c1f;
    --proto-surface-strong: #1a2529;
    --proto-ink: #e7eeec;
    --proto-muted: #91a3a3;
    --proto-line: rgb(231 238 236 / 0.1);
    --proto-accent: #63adba;
    --proto-accent-soft: rgb(99 173 186 / 0.15);
    --proto-on-accent: #071416;
    --reader-surface: #111b1c;
    --reader-ink: #cfddda;
  }

  .prototype-light.prototype-palette-tide {
    --proto-canvas: #edf2f1;
    --proto-surface: #f5f8f7;
    --proto-surface-strong: #ffffff;
    --proto-ink: #172326;
    --proto-muted: #657578;
    --proto-line: rgb(23 35 38 / 0.1);
    --proto-accent: #267d8b;
    --proto-accent-soft: rgb(38 125 139 / 0.13);
    --proto-on-accent: #f4ffff;
    --reader-surface: #e8efed;
    --reader-ink: #1e2b2c;
  }

  .prototype-dark.prototype-palette-volt {
    --proto-canvas: #10110f;
    --proto-surface: #171815;
    --proto-surface-strong: #1e201b;
    --proto-ink: #f0f1e9;
    --proto-muted: #a1a394;
    --proto-line: rgb(240 241 233 / 0.1);
    --proto-accent: #d5e548;
    --proto-accent-soft: rgb(213 229 72 / 0.14);
    --proto-on-accent: #16180b;
    --reader-surface: #171812;
    --reader-ink: #e0e2d5;
  }

  .prototype-light.prototype-palette-volt {
    --proto-canvas: #f2f4ed;
    --proto-surface: #f8faf3;
    --proto-surface-strong: #ffffff;
    --proto-ink: #20231e;
    --proto-muted: #6c7165;
    --proto-line: rgb(32 35 30 / 0.1);
    --proto-accent: #748100;
    --proto-accent-soft: rgb(116 129 0 / 0.13);
    --proto-on-accent: #fbffe8;
    --reader-surface: #eef0e6;
    --reader-ink: #252820;
  }

  .variant-stage {
    width: 100%;
    height: 100%;
  }

  .prototype-question {
    position: fixed;
    top: 7px;
    left: 50%;
    z-index: 70;
    display: flex;
    align-items: center;
    gap: 8px;
    pointer-events: none;
    color: color-mix(in oklab, var(--proto-ink) 48%, transparent);
    font-family: var(--font-code, monospace);
    font-size: 0.5rem;
    transform: translateX(-50%);
    white-space: nowrap;
  }

  .prototype-question span {
    color: var(--proto-accent);
    letter-spacing: 0.12em;
  }

  .prototype-question p {
    margin: 0;
  }

  .prototype-unavailable {
    display: grid;
    min-height: 100dvh;
    place-content: center;
    padding: 24px;
    background: #111316;
    color: #eceae4;
    text-align: center;
  }

  .prototype-unavailable h1,
  .prototype-unavailable p {
    margin: 0;
  }

  .prototype-unavailable p {
    margin-top: 8px;
    color: #9b9993;
  }

  @media (max-width: 900px) {
    .prototype-root {
      --prototype-switcher-bottom: 152px;
    }

    .prototype-question {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .variant-stage {
      transition: none;
    }
  }
</style>
