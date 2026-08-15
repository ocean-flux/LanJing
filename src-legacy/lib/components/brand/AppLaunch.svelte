<script lang="ts">
  import { m } from '$lib/i18n';
  import LanJingMark from './LanJingMark.svelte';

  type Props = {
    visible?: boolean;
    durationMs?: number;
    oncomplete?: () => void;
    class?: string;
  };

  let { visible = true, durationMs = 1800, oncomplete, class: className = '' }: Props = $props();

  const launchDuration = $derived(`${durationMs}ms`);

  function handleAnimationEnd(event: AnimationEvent) {
    if (event.animationName !== 'launch-exit') return;
    oncomplete?.();
  }
</script>

{#if visible}
  <section
    class={['app-launch', className]}
    style:--launch-duration={launchDuration}
    aria-label={m.launch_label()}
    onanimationend={handleAnimationEnd}
  >
    <div class="launch-frame" aria-hidden="true">
      <span></span>
      <span></span>
      <span></span>
    </div>

    <div class="launch-mark">
      <LanJingMark width={96} height={72} animated variant="photo" />
    </div>

    <div class="launch-copy">
      <span>{m.launch_product_name()}</span>
      <strong>{m.launch_slogan()}</strong>
    </div>
  </section>
{/if}

<style>
  .app-launch {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    overflow: hidden;
    background: var(--background);
    color: var(--foreground);
    animation: launch-exit 420ms ease calc(var(--launch-duration) - 420ms) forwards;
  }

  .launch-frame {
    position: absolute;
    width: min(68vw, 42rem);
    height: min(42dvh, 18rem);
    border: 1px solid color-mix(in oklab, var(--foreground) 10%, transparent);
    background: color-mix(in oklab, var(--surface-panel) 74%, transparent);
    box-shadow:
      0 1px 0 color-mix(in oklab, white 8%, transparent) inset,
      0 24px 70px rgb(0 0 0 / 0.14);
    animation: frame-enter 820ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  .launch-frame span {
    position: absolute;
    left: 12%;
    right: 12%;
    height: 1px;
    background: color-mix(in oklab, var(--lantern) 18%, transparent);
  }

  .launch-frame span:nth-child(1) {
    top: 32%;
  }

  .launch-frame span:nth-child(2) {
    top: 50%;
    opacity: 0.62;
  }

  .launch-frame span:nth-child(3) {
    top: 68%;
    background: color-mix(in oklab, var(--foreground) 10%, transparent);
  }

  .launch-mark {
    position: relative;
    z-index: 1;
    display: grid;
    place-items: center;
    width: 8.5rem;
    height: 8.5rem;
    border: 1px solid color-mix(in oklab, var(--foreground) 8%, transparent);
    background: color-mix(in oklab, var(--surface-panel) 72%, transparent);
    box-shadow: 0 18px 55px rgb(0 0 0 / 0.16);
    animation: mark-rise 760ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  .launch-copy {
    position: absolute;
    z-index: 1;
    bottom: clamp(2rem, 12dvh, 6rem);
    display: grid;
    gap: 0.55rem;
    text-align: center;
    animation: copy-enter 760ms cubic-bezier(0.16, 1, 0.3, 1) 120ms both;
  }

  .launch-copy span {
    color: var(--muted-foreground);
    font-size: 0.78rem;
    letter-spacing: 0.18em;
    text-transform: uppercase;
  }

  .launch-copy strong {
    font-size: clamp(1.35rem, 3vw, 2.7rem);
    font-weight: 600;
    letter-spacing: -0.04em;
  }

  @keyframes frame-enter {
    from {
      opacity: 0;
      transform: scaleX(0.72) scaleY(0.9);
      filter: blur(12px);
    }
    to {
      opacity: 1;
      transform: scale(1);
      filter: blur(0);
    }
  }

  @keyframes mark-rise {
    from {
      opacity: 0;
      transform: translateY(14px) scale(0.94);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  @keyframes copy-enter {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @keyframes launch-exit {
    to {
      opacity: 0;
      transform: scale(1.015);
      filter: blur(8px);
      visibility: hidden;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .app-launch,
    .launch-frame,
    .launch-mark,
    .launch-copy {
      animation-duration: 1ms;
      animation-delay: 1ms;
    }
  }
</style>
