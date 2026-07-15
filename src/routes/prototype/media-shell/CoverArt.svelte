<script lang="ts">
  import type { CoverMotif } from './prototype-fixtures';

  type Props = {
    title: string;
    creator?: string;
    kicker?: string;
    art: string;
    motif: CoverMotif;
    foreground?: 'light' | 'dark';
    ratio?: 'portrait' | 'square' | 'landscape';
    quiet?: boolean;
    class?: string;
  };

  let {
    title,
    creator = '',
    kicker = '',
    art,
    motif,
    foreground = 'light',
    ratio = 'portrait',
    quiet = false,
    class: className = '',
  }: Props = $props();
</script>

<div
  class={[
    'cover-art',
    `ratio-${ratio}`,
    `motif-${motif}`,
    `ink-${foreground}`,
    quiet && 'is-quiet',
    className,
  ]}
  style:--cover-art={art}
  role="img"
  aria-label={[title, creator].filter(Boolean).join('，')}
>
  <span class="cover-art__field" aria-hidden="true"></span>
  <span class="cover-art__trace" aria-hidden="true"></span>
  {#if !quiet}
    <span class="cover-art__copy" aria-hidden="true">
      {#if kicker}<span class="cover-art__kicker">{kicker}</span>{/if}
      <strong>{title}</strong>
      {#if creator}<span class="cover-art__creator">{creator}</span>{/if}
    </span>
  {/if}
</div>

<style>
  .cover-art {
    --cover-radius: 18px;
    position: relative;
    isolation: isolate;
    min-width: 0;
    overflow: hidden;
    border-radius: var(--cover-radius);
    background: var(--cover-art);
    box-shadow:
      0 1px 0 rgb(255 255 255 / 0.2) inset,
      0 20px 60px rgb(12 14 18 / 0.22);
  }

  .ratio-portrait {
    aspect-ratio: 0.69;
  }

  .ratio-square {
    aspect-ratio: 1;
  }

  .ratio-landscape {
    aspect-ratio: 1.62;
  }

  .cover-art__field {
    position: absolute;
    inset: 0;
    z-index: -1;
    background:
      linear-gradient(180deg, rgb(255 255 255 / 0.08), transparent 24%),
      linear-gradient(0deg, rgb(4 7 10 / 0.46), transparent 48%);
  }

  .cover-art__field::after {
    position: absolute;
    inset: 0;
    background-image: radial-gradient(rgb(255 255 255 / 0.16) 0.6px, transparent 0.7px);
    background-size: 5px 5px;
    content: '';
    mix-blend-mode: soft-light;
    opacity: 0.18;
  }

  .cover-art__trace {
    position: absolute;
    z-index: 0;
    pointer-events: none;
  }

  .motif-horizon .cover-art__trace {
    right: 10%;
    bottom: 17%;
    width: 46%;
    height: 1px;
    background: currentColor;
    box-shadow: 0 -7px 0 rgb(255 255 255 / 0.18);
    opacity: 0.68;
  }

  .motif-orbit .cover-art__trace,
  .motif-signal .cover-art__trace {
    top: 16%;
    right: 12%;
    width: 42%;
    border: 1px solid currentColor;
    border-radius: 50%;
    aspect-ratio: 1;
    opacity: 0.48;
  }

  .motif-signal .cover-art__trace::after {
    position: absolute;
    top: 50%;
    right: -24%;
    left: -74%;
    height: 1px;
    background: currentColor;
    content: '';
  }

  .motif-type .cover-art__trace {
    top: 13%;
    right: 12%;
    width: 20%;
    height: 30%;
    border-top: 1px solid currentColor;
    border-right: 1px solid currentColor;
    opacity: 0.46;
  }

  .motif-petal .cover-art__trace {
    top: 13%;
    right: 10%;
    width: 26%;
    border: 1px solid currentColor;
    border-radius: 80% 15% 75% 20%;
    aspect-ratio: 0.72;
    transform: rotate(22deg);
    opacity: 0.44;
  }

  .motif-paper .cover-art__trace {
    top: 14%;
    right: 13%;
    width: 32%;
    height: 26%;
    border-top: 1px solid currentColor;
    border-bottom: 1px solid currentColor;
    opacity: 0.42;
  }

  .ink-light {
    color: #f5f0e7;
  }

  .ink-dark {
    color: #20242a;
  }

  .cover-art__copy {
    position: absolute;
    right: clamp(14px, 7%, 24px);
    bottom: clamp(14px, 7%, 24px);
    left: clamp(14px, 7%, 24px);
    display: grid;
    gap: 5px;
    text-shadow: 0 1px 14px rgb(0 0 0 / 0.24);
  }

  .cover-art__kicker,
  .cover-art__creator {
    font-size: clamp(0.56rem, 1.2vw, 0.7rem);
    line-height: 1.2;
    letter-spacing: 0.08em;
    opacity: 0.74;
  }

  .cover-art__kicker {
    text-transform: uppercase;
  }

  strong {
    max-width: 9ch;
    font-size: clamp(0.88rem, 2.2vw, 1.34rem);
    font-weight: 550;
    line-height: 1.08;
    letter-spacing: -0.025em;
  }

  .cover-art__creator {
    letter-spacing: 0.02em;
  }

  .is-quiet .cover-art__field {
    background: linear-gradient(180deg, rgb(255 255 255 / 0.08), rgb(5 8 12 / 0.12));
  }

  @media (max-width: 600px) {
    .cover-art {
      --cover-radius: 14px;
    }
  }
</style>
