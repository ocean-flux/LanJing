<script lang="ts">
  import GripVertical from '@lucide/svelte/icons/grip-vertical';
  import Pause from '@lucide/svelte/icons/pause';
  import Play from '@lucide/svelte/icons/play';
  import X from '@lucide/svelte/icons/x';
  import { prefersReducedMotion } from 'svelte/motion';
  import { fade, fly } from 'svelte/transition';
  import { tracks, type PrototypeAudioState } from './prototype-fixtures';

  type Props = {
    open: boolean;
    audioState: PrototypeAudioState;
    onclose: () => void;
    onaudiochange: (state: PrototypeAudioState) => void;
  };

  let { open, audioState, onclose, onaudiochange }: Props = $props();

  function handleKeydown(event: KeyboardEvent): void {
    if (open && event.key === 'Escape') onclose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <button
    class="queue-backdrop"
    type="button"
    aria-label="关闭播放队列"
    onclick={onclose}
    transition:fade={{ duration: prefersReducedMotion.current ? 0 : 180 }}
  ></button>
  <div
    class="queue-panel"
    role="dialog"
    aria-modal="true"
    aria-labelledby="prototype-queue-title"
    transition:fly={{
      x: prefersReducedMotion.current ? 0 : 28,
      duration: prefersReducedMotion.current ? 0 : 360,
    }}
  >
    <header>
      <div>
        <span>环境音频会话</span>
        <h2 id="prototype-queue-title">接下来播放</h2>
      </div>
      <button type="button" onclick={onclose} aria-label="关闭队列">
        <X size={18} strokeWidth={1.5} aria-hidden="true" />
      </button>
    </header>

    <div class="queue-session">
      <span class:active={audioState === 'playing'} aria-hidden="true"></span>
      <p>
        <strong>{audioState === 'playing' ? '环境音频正在跨空间继续' : '环境音频已暂停'}</strong>
        <small>其他有声活动开始时暂停，结束后不自动恢复。</small>
      </p>
      <button
        type="button"
        onclick={() => onaudiochange(audioState === 'playing' ? 'paused' : 'playing')}
        aria-label={audioState === 'playing' ? '暂停环境音频' : '恢复环境音频'}
      >
        {#if audioState === 'playing'}
          <Pause size={16} fill="currentColor" strokeWidth={1.4} aria-hidden="true" />
        {:else}
          <Play size={16} fill="currentColor" strokeWidth={1.4} aria-hidden="true" />
        {/if}
      </button>
    </div>

    <ol>
      {#each tracks as track, index (track.id)}
        <li class:current={index === 0}>
          <GripVertical size={15} strokeWidth={1.4} aria-hidden="true" />
          <span>{String(index + 1).padStart(2, '0')}</span>
          <p>
            <strong>{track.title}</strong>
            <small>{track.artist}</small>
          </p>
          <time>{track.duration}</time>
        </li>
      {/each}
    </ol>

    <footer>PROTOTYPE · 原创本地 fixture · 不写入播放历史</footer>
  </div>
{/if}

<style>
  .queue-backdrop {
    position: fixed;
    inset: 0;
    z-index: 88;
    border: 0;
    background: rgb(7 8 10 / 0.48);
    cursor: default;
    backdrop-filter: blur(6px);
  }

  .queue-panel {
    position: fixed;
    top: 12px;
    right: 12px;
    bottom: 12px;
    z-index: 89;
    display: grid;
    grid-template-rows: auto auto 1fr auto;
    width: min(390px, calc(100vw - 24px));
    overflow: hidden;
    border: 1px solid color-mix(in oklab, var(--proto-ink) 12%, transparent);
    border-radius: 22px;
    background: var(--proto-surface-strong);
    color: var(--proto-ink);
    box-shadow:
      0 1px 0 rgb(255 255 255 / 0.08) inset,
      0 28px 88px rgb(0 0 0 / 0.34);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 22px 22px 16px;
  }

  header div {
    display: grid;
    gap: 2px;
  }

  header span {
    color: var(--proto-accent);
    font-family: var(--font-code, monospace);
    font-size: 0.6rem;
    letter-spacing: 0.1em;
  }

  h2 {
    margin: 0;
    font-size: 1.28rem;
    font-weight: 560;
    letter-spacing: -0.025em;
  }

  button {
    border: 0;
    font: inherit;
  }

  header button,
  .queue-session > button {
    display: grid;
    width: 38px;
    place-items: center;
    border-radius: 11px;
    background: color-mix(in oklab, var(--proto-ink) 6%, transparent);
    color: var(--proto-ink);
    cursor: pointer;
    transition:
      background 180ms cubic-bezier(0.32, 0.72, 0, 1),
      transform 180ms cubic-bezier(0.32, 0.72, 0, 1);
    aspect-ratio: 1;
  }

  header button:hover,
  header button:focus-visible,
  .queue-session > button:hover,
  .queue-session > button:focus-visible {
    background: color-mix(in oklab, var(--proto-ink) 11%, transparent);
  }

  header button:active,
  .queue-session > button:active {
    transform: scale(0.93);
  }

  .queue-session {
    display: grid;
    grid-template-columns: 10px minmax(0, 1fr) auto;
    gap: 12px;
    align-items: center;
    margin: 0 14px 10px;
    padding: 14px;
    border-radius: 16px;
    background: var(--proto-accent-soft);
  }

  .queue-session > span {
    width: 7px;
    border-radius: 50%;
    background: var(--proto-muted);
    aspect-ratio: 1;
  }

  .queue-session > span.active {
    background: var(--proto-accent);
    box-shadow: 0 0 0 5px color-mix(in oklab, var(--proto-accent) 12%, transparent);
  }

  p {
    display: grid;
    min-width: 0;
    gap: 2px;
    margin: 0;
  }

  p strong {
    font-size: 0.78rem;
    font-weight: 560;
  }

  p small {
    color: var(--proto-muted);
    font-size: 0.65rem;
    line-height: 1.35;
  }

  ol {
    min-height: 0;
    overflow: auto;
    margin: 0;
    padding: 8px 14px 20px;
    list-style: none;
  }

  li {
    display: grid;
    grid-template-columns: 16px 22px minmax(0, 1fr) auto;
    gap: 9px;
    align-items: center;
    min-height: 58px;
    padding: 6px 8px;
    border-radius: 12px;
    color: var(--proto-muted);
  }

  li.current {
    background: color-mix(in oklab, var(--proto-ink) 5%, transparent);
    color: var(--proto-accent);
  }

  li > span,
  time {
    font-family: var(--font-code, monospace);
    font-size: 0.58rem;
  }

  li p strong {
    color: var(--proto-ink);
  }

  time {
    color: var(--proto-muted);
  }

  footer {
    padding: 14px 22px 18px;
    border-top: 1px solid var(--proto-line);
    color: var(--proto-muted);
    font-family: var(--font-code, monospace);
    font-size: 0.56rem;
    letter-spacing: 0.05em;
  }

  @media (max-width: 600px) {
    .queue-panel {
      top: auto;
      right: 8px;
      bottom: 8px;
      left: 8px;
      width: auto;
      max-height: min(76dvh, 620px);
      border-radius: 20px;
    }
  }

  @media (prefers-reduced-transparency: reduce) {
    .queue-backdrop {
      backdrop-filter: none;
    }
  }
</style>
