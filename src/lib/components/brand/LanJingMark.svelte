<script lang="ts">
  import { asset } from '$app/paths';
  import { m } from '$lib/i18n';

  type Props = {
    size?: number;
    width?: number;
    height?: number;
    animated?: boolean;
    label?: string;
    class?: string;
    /** `ink` = shell mark (theme ink mask). `photo` = raw monogram PNG (launch/hero). */
    variant?: 'ink' | 'photo';
  };

  let {
    size = 64,
    width,
    height,
    animated = false,
    label = 'LanJing',
    class: className = '',
    variant = 'ink',
  }: Props = $props();

  const title = $derived(label ? m.brand_mark_label({ label }) : m.brand_mark_fallback());
  const imageWidth = $derived(`${width ?? size}px`);
  const imageHeight = $derived(`${height ?? size}px`);
  const monogramUrl = asset('/brand/icon.png');
</script>

<span
  class={[
    'lanjing-mark',
    variant === 'ink' ? 'is-ink' : 'is-photo',
    animated && 'is-animated',
    className,
  ]}
  style:--mark-width={imageWidth}
  style:--mark-height={imageHeight}
  style:--mark-url={`url(${monogramUrl})`}
  role="img"
  aria-label={title}
>
  {#if variant === 'photo'}
    <img src={monogramUrl} alt="" aria-hidden="true" />
  {:else}
    <span class="lanjing-mark-glyph" aria-hidden="true"></span>
  {/if}
</span>

<style>
  .lanjing-mark {
    display: inline-grid;
    width: var(--mark-width);
    height: var(--mark-height);
    place-items: center;
    overflow: visible;
    border-radius: 0;
    background: transparent;
  }

  /* Quiet precision: monogram as ink silhouette — matches canvas/ink, no metal mud. */
  .lanjing-mark-glyph {
    display: block;
    width: 100%;
    height: 100%;
    background-color: var(--ink);
    opacity: 0.88;
    -webkit-mask-image: var(--mark-url);
    mask-image: var(--mark-url);
    -webkit-mask-repeat: no-repeat;
    mask-repeat: no-repeat;
    -webkit-mask-position: center;
    mask-position: center;
    -webkit-mask-size: contain;
    mask-size: contain;
  }

  .is-photo img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .is-animated .lanjing-mark-glyph,
  .is-animated img {
    animation: mark-enter 980ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  @keyframes mark-enter {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.96);
    }

    to {
      opacity: 0.88;
      transform: translateY(0) scale(1);
    }
  }

  .is-photo.is-animated img {
    animation-name: mark-photo-enter;
  }

  @keyframes mark-photo-enter {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.96);
    }

    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .is-animated .lanjing-mark-glyph,
    .is-animated img {
      animation: none;
    }
  }
</style>
