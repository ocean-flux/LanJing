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
  };

  let {
    size = 64,
    width,
    height,
    animated = false,
    label = 'LanJing',
    class: className = '',
  }: Props = $props();

  const title = $derived(label ? m.brand_mark_label({ label }) : m.brand_mark_fallback());
  const imageWidth = $derived(`${width ?? size}px`);
  const imageHeight = $derived(`${height ?? size}px`);
</script>

<span
  class={['lanjing-mark', animated && 'is-animated', className]}
  style:--mark-width={imageWidth}
  style:--mark-height={imageHeight}
  role="img"
  aria-label={title}
>
  <img src={asset('/brand/lanjing-app-icon.png')} alt="" aria-hidden="true" />
</span>

<style>
  .lanjing-mark {
    display: inline-grid;
    width: var(--mark-width);
    height: var(--mark-height);
    place-items: center;
    overflow: hidden;
    border-radius: 24%;
  }

  .lanjing-mark img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .is-animated img {
    animation: mark-image-enter 980ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  @keyframes mark-image-enter {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.94);
      filter: blur(6px);
    }

    58% {
      opacity: 1;
    }

    to {
      opacity: 1;
      transform: translateY(0) scale(1);
      filter: blur(0);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .is-animated img {
      animation: none;
    }
  }
</style>
