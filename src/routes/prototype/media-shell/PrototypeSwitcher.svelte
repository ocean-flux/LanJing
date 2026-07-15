<script lang="ts">
  import { dev } from '$app/environment';
  import { ArrowLeft, ArrowRight } from '@lucide/svelte';
  import type { PrototypePalette, PrototypeVariant } from './prototype-fixtures';
  import { paletteOptions, variantNames } from './prototype-fixtures';

  type Props = {
    current: PrototypeVariant;
    palette: PrototypePalette;
    stateSummary: string;
    onchange: (variant: PrototypeVariant) => void;
    onpalettechange: (palette: PrototypePalette) => void;
  };

  const variants: PrototypeVariant[] = ['A', 'B', 'C'];
  let { current, palette, stateSummary, onchange, onpalettechange }: Props = $props();

  function cycle(direction: -1 | 1): void {
    const currentIndex = variants.indexOf(current);
    const nextIndex = (currentIndex + direction + variants.length) % variants.length;
    onchange(variants[nextIndex]);
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (!dev || (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight')) return;

    const target = event.target;
    if (
      target instanceof HTMLElement &&
      (target.matches('input, textarea, select, [contenteditable="true"]') ||
        target.closest('[contenteditable="true"]'))
    ) {
      return;
    }

    event.preventDefault();
    cycle(event.key === 'ArrowLeft' ? -1 : 1);
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if dev}
  <aside class="prototype-switcher" aria-label="原型变体切换器">
    <button type="button" onclick={() => cycle(-1)} aria-label="上一个原型变体">
      <ArrowLeft size={16} strokeWidth={1.6} aria-hidden="true" />
    </button>
    <div class="prototype-switcher__label" aria-live="polite">
      <span>DEV PROTOTYPE</span>
      <strong>{current} · {variantNames[current]}</strong>
      <small>{stateSummary}</small>
      <div class="palette-swatches" aria-label="原型色系">
        {#each paletteOptions as option (option.id)}
          <button
            type="button"
            class:active={palette === option.id}
            style:--swatch={option.swatch}
            aria-label={`切换到${option.label}色系`}
            title={option.label}
            onclick={() => onpalettechange(option.id)}
          ></button>
        {/each}
      </div>
    </div>
    <button type="button" onclick={() => cycle(1)} aria-label="下一个原型变体">
      <ArrowRight size={16} strokeWidth={1.6} aria-hidden="true" />
    </button>
  </aside>
{/if}

<style>
  .prototype-switcher {
    position: fixed;
    right: 50%;
    bottom: var(--prototype-switcher-bottom, 16px);
    z-index: 80;
    display: grid;
    grid-template-columns: 42px minmax(150px, auto) 42px;
    align-items: stretch;
    overflow: hidden;
    border: 1px solid rgb(255 255 255 / 0.16);
    border-radius: 16px;
    background: rgb(14 15 18 / 0.94);
    color: #f4f1eb;
    box-shadow:
      0 1px 0 rgb(255 255 255 / 0.1) inset,
      0 18px 52px rgb(0 0 0 / 0.34);
    transform: translateX(50%);
    backdrop-filter: blur(18px) saturate(1.1);
  }

  button {
    display: grid;
    min-width: 42px;
    min-height: 52px;
    place-items: center;
    border: 0;
    background: transparent;
    color: inherit;
    cursor: pointer;
    transition:
      background 180ms cubic-bezier(0.32, 0.72, 0, 1),
      transform 180ms cubic-bezier(0.32, 0.72, 0, 1);
  }

  button:hover,
  button:focus-visible {
    background: rgb(255 255 255 / 0.1);
  }

  button:active {
    transform: scale(0.94);
  }

  .prototype-switcher__label {
    display: grid;
    align-content: center;
    min-width: 0;
    padding: 7px 12px;
    border-right: 1px solid rgb(255 255 255 / 0.1);
    border-left: 1px solid rgb(255 255 255 / 0.1);
    text-align: center;
  }

  span {
    color: #d2734a;
    font-family: var(--font-code, monospace);
    font-size: 0.54rem;
    letter-spacing: 0.16em;
  }

  strong {
    margin-top: 1px;
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.01em;
    white-space: nowrap;
  }

  small {
    overflow: hidden;
    margin-top: 1px;
    color: rgb(244 241 235 / 0.62);
    font-size: 0.58rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .palette-swatches {
    display: flex;
    justify-content: center;
    gap: 7px;
    margin-top: 5px;
  }

  .palette-swatches button {
    min-width: 13px;
    min-height: 13px;
    border: 1px solid rgb(255 255 255 / 0.24);
    border-radius: 50%;
    background: var(--swatch);
    box-shadow: none;
    aspect-ratio: 1;
  }

  .palette-swatches button.active {
    box-shadow:
      0 0 0 2px #111216,
      0 0 0 3px rgb(255 255 255 / 0.74);
  }

  .palette-swatches button:hover,
  .palette-swatches button:focus-visible {
    background: var(--swatch);
    transform: scale(1.14);
  }

  @media (max-width: 520px) {
    .prototype-switcher {
      grid-template-columns: 38px minmax(128px, 1fr) 38px;
      width: min(286px, calc(100vw - 28px));
      border-radius: 14px;
    }

    button {
      min-width: 38px;
      min-height: 48px;
    }

    .palette-swatches button {
      min-width: 13px;
      min-height: 13px;
    }

    .prototype-switcher__label {
      padding-inline: 8px;
    }

    small {
      font-size: 0.54rem;
    }
  }

  @media (prefers-reduced-transparency: reduce) {
    .prototype-switcher {
      background: #111216;
      backdrop-filter: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    button {
      transition-duration: 0.01ms;
    }
  }
</style>
