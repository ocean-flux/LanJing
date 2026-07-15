<script lang="ts">
  import { ListMusic, Music2, Pause, Play, Volume2, X } from '@lucide/svelte';
  import {
    ambientTrack,
    type PrototypeAudioState,
    type PrototypeVariant,
  } from './prototype-fixtures';

  type Props = {
    state: PrototypeAudioState;
    tone: PrototypeVariant;
    reader?: boolean;
    onstatechange: (state: PrototypeAudioState) => void;
    onqueueopen: () => void;
  };

  let { state, tone, reader = false, onstatechange, onqueueopen }: Props = $props();
  const playing = $derived(state === 'playing');

  function togglePlayback(): void {
    onstatechange(playing ? 'paused' : 'playing');
  }
</script>

{#if state !== 'none'}
  {#if reader}
    <div class="reader-audio" data-tone={tone}>
      <span class="reader-audio__signal" class:is-playing={playing} aria-hidden="true">
        <Music2 size={15} strokeWidth={1.6} />
      </span>
      <button class="reader-audio__track" type="button" onclick={togglePlayback}>
        <span>{playing ? '环境音频播放中' : '环境音频已暂停'}</span>
        <strong>{ambientTrack.title}</strong>
      </button>
      <button
        class="reader-audio__action"
        type="button"
        onclick={togglePlayback}
        aria-label={playing ? '暂停环境音频' : '继续环境音频'}
      >
        {#if playing}
          <Pause size={15} fill="currentColor" strokeWidth={1.4} aria-hidden="true" />
        {:else}
          <Play size={15} fill="currentColor" strokeWidth={1.4} aria-hidden="true" />
        {/if}
      </button>
      <button
        class="reader-audio__action"
        type="button"
        onclick={() => onstatechange('none')}
        aria-label="结束环境音频会话"
      >
        <X size={15} strokeWidth={1.6} aria-hidden="true" />
      </button>
    </div>
  {:else}
    <section class="media-bar" data-tone={tone} aria-label="连续媒体带">
      <div class="media-bar__art" style:--ambient-art={ambientTrack.art} aria-hidden="true">
        <span class:is-playing={playing}></span>
      </div>

      <button class="media-bar__identity" type="button" onclick={togglePlayback}>
        <span>{playing ? '环境音频 · 正在播放' : '环境音频 · 已暂停'}</span>
        <strong>{ambientTrack.title}</strong>
        <small>{ambientTrack.artist}</small>
      </button>

      <div class="media-bar__timeline" aria-label={`播放进度 ${ambientTrack.progress}%`}>
        <span>{ambientTrack.elapsed}</span>
        <div><i style:--progress={`${ambientTrack.progress / 100}`}></i></div>
        <span>{ambientTrack.duration}</span>
      </div>

      <div class="media-bar__actions">
        <button type="button" onclick={togglePlayback} aria-label={playing ? '暂停' : '播放'}>
          {#if playing}
            <Pause size={17} fill="currentColor" strokeWidth={1.4} aria-hidden="true" />
          {:else}
            <Play size={17} fill="currentColor" strokeWidth={1.4} aria-hidden="true" />
          {/if}
        </button>
        <button type="button" aria-label="音量">
          <Volume2 size={17} strokeWidth={1.5} aria-hidden="true" />
        </button>
        <button type="button" onclick={onqueueopen} aria-label="打开队列">
          <ListMusic size={18} strokeWidth={1.5} aria-hidden="true" />
        </button>
        <button type="button" onclick={() => onstatechange('none')} aria-label="结束环境音频会话">
          <X size={17} strokeWidth={1.5} aria-hidden="true" />
        </button>
      </div>
    </section>
  {/if}
{/if}

<style>
  button {
    border: 0;
    font: inherit;
  }

  .media-bar {
    position: relative;
    z-index: 12;
    display: grid;
    grid-template-columns: 48px minmax(170px, 0.9fr) minmax(180px, 1.35fr) auto;
    gap: 14px;
    align-items: center;
    min-height: 68px;
    padding: 9px 14px;
    border-top: 1px solid var(--proto-line);
    background: color-mix(in oklab, var(--proto-surface) 92%, transparent);
    color: var(--proto-ink);
    box-shadow: 0 -12px 36px rgb(7 9 12 / 0.05);
  }

  .media-bar[data-tone='B'] {
    margin: 0 16px 12px;
    border: 1px solid color-mix(in oklab, var(--proto-ink) 11%, transparent);
    border-radius: 18px;
    background: color-mix(in oklab, var(--proto-surface-strong) 84%, transparent);
    box-shadow:
      0 1px 0 rgb(255 255 255 / 0.1) inset,
      0 18px 54px rgb(7 9 12 / 0.16);
    backdrop-filter: blur(18px) saturate(1.08);
  }

  .media-bar[data-tone='C'] {
    min-height: 58px;
    padding-block: 7px;
    border-top-color: color-mix(in oklab, var(--proto-ink) 8%, transparent);
    background: var(--proto-canvas);
    box-shadow: none;
  }

  .media-bar__art {
    position: relative;
    width: 48px;
    overflow: hidden;
    border-radius: 11px;
    background: var(--ambient-art);
    box-shadow: 0 8px 24px rgb(8 10 12 / 0.2);
    aspect-ratio: 1;
  }

  .media-bar__art::before {
    position: absolute;
    inset: 18%;
    border: 1px solid rgb(247 240 226 / 0.66);
    border-radius: 50%;
    content: '';
  }

  .media-bar__art span {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 4px;
    border-radius: 50%;
    background: #eee7dc;
    transform: translate(-50%, -50%);
    aspect-ratio: 1;
  }

  @media (prefers-reduced-motion: no-preference) {
    .media-bar__art span.is-playing {
      animation: ambient-pulse 1.8s cubic-bezier(0.32, 0.72, 0, 1) infinite alternate;
    }
  }

  .media-bar__identity {
    display: grid;
    min-width: 0;
    padding: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }

  .media-bar__identity > span {
    color: var(--proto-accent);
    font-family: var(--font-code, monospace);
    font-size: 0.58rem;
    letter-spacing: 0.08em;
  }

  .media-bar__identity strong,
  .reader-audio__track strong {
    overflow: hidden;
    margin-top: 1px;
    font-size: 0.82rem;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .media-bar__identity small {
    overflow: hidden;
    margin-top: 1px;
    color: var(--proto-muted);
    font-size: 0.68rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .media-bar__timeline {
    display: grid;
    grid-template-columns: auto minmax(80px, 1fr) auto;
    gap: 8px;
    align-items: center;
    color: var(--proto-muted);
    font-family: var(--font-code, monospace);
    font-size: 0.58rem;
  }

  .media-bar__timeline div {
    position: relative;
    height: 2px;
    overflow: hidden;
    border-radius: 999px;
    background: color-mix(in oklab, var(--proto-ink) 13%, transparent);
  }

  .media-bar__timeline i {
    position: absolute;
    inset: 0;
    background: var(--proto-accent);
    transform: scaleX(var(--progress));
    transform-origin: left center;
  }

  .media-bar__actions {
    display: flex;
    gap: 2px;
  }

  .media-bar__actions button,
  .reader-audio__action {
    display: grid;
    width: 36px;
    place-items: center;
    border-radius: 10px;
    background: transparent;
    color: var(--proto-muted);
    cursor: pointer;
    transition:
      color 180ms cubic-bezier(0.32, 0.72, 0, 1),
      background 180ms cubic-bezier(0.32, 0.72, 0, 1),
      transform 180ms cubic-bezier(0.32, 0.72, 0, 1);
    aspect-ratio: 1;
  }

  .media-bar__actions button:hover,
  .media-bar__actions button:focus-visible,
  .reader-audio__action:hover,
  .reader-audio__action:focus-visible {
    background: color-mix(in oklab, var(--proto-ink) 8%, transparent);
    color: var(--proto-ink);
  }

  .media-bar__actions button:active,
  .reader-audio__action:active {
    transform: scale(0.92);
  }

  .reader-audio {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) 34px 34px;
    gap: 5px;
    align-items: center;
    width: min(360px, calc(100vw - 28px));
    min-height: 48px;
    padding: 6px;
    border: 1px solid color-mix(in oklab, var(--reader-ink, var(--proto-ink)) 12%, transparent);
    border-radius: 15px;
    background: color-mix(in oklab, var(--reader-surface, var(--proto-surface)) 88%, transparent);
    color: var(--reader-ink, var(--proto-ink));
    box-shadow: 0 12px 36px rgb(7 9 12 / 0.14);
    backdrop-filter: blur(14px);
  }

  .reader-audio[data-tone='C'] {
    width: auto;
    min-width: 220px;
    border-radius: 999px;
  }

  .reader-audio__signal {
    display: grid;
    width: 34px;
    place-items: center;
    border-radius: 10px;
    background: var(--proto-accent-soft);
    color: var(--proto-accent);
    aspect-ratio: 1;
  }

  .reader-audio__signal.is-playing {
    box-shadow: 0 0 0 4px color-mix(in oklab, var(--proto-accent) 10%, transparent);
  }

  .reader-audio__track {
    display: grid;
    min-width: 0;
    padding: 0 4px;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }

  .reader-audio__track span {
    color: color-mix(in oklab, currentColor 64%, transparent);
    font-size: 0.58rem;
  }

  @keyframes ambient-pulse {
    from {
      opacity: 0.55;
      transform: translate(-50%, -50%) scale(0.82);
    }
    to {
      opacity: 1;
      transform: translate(-50%, -50%) scale(1.18);
    }
  }

  @media (max-width: 820px) {
    .media-bar {
      grid-template-columns: 44px minmax(0, 1fr) auto;
      min-height: 62px;
      padding: 7px 10px;
    }

    .media-bar[data-tone='B'] {
      margin: 0 10px 8px;
      border-radius: 15px;
    }

    .media-bar__art {
      width: 44px;
      border-radius: 9px;
    }

    .media-bar__timeline {
      position: absolute;
      right: 10px;
      bottom: 3px;
      left: 64px;
      display: block;
    }

    .media-bar__timeline > span {
      display: none;
    }

    .media-bar__timeline div {
      height: 2px;
    }

    .media-bar__actions button:nth-child(2),
    .media-bar__actions button:nth-child(4) {
      display: none;
    }
  }

  @media (max-width: 430px) {
    .media-bar__identity > span,
    .media-bar__identity small {
      display: none;
    }

    .media-bar__identity strong {
      font-size: 0.76rem;
    }

    .reader-audio {
      grid-template-columns: 32px minmax(0, 1fr) 32px 32px;
      min-height: 44px;
      padding: 5px;
      border-radius: 14px;
    }

    .reader-audio__signal,
    .reader-audio__action {
      width: 32px;
    }
  }

  @media (prefers-reduced-transparency: reduce) {
    .media-bar[data-tone='B'],
    .reader-audio {
      background: var(--proto-surface-strong);
      backdrop-filter: none;
    }
  }
</style>
