<script lang="ts">
  import type { MediaAppKey } from '$lib/brand';
  import { m } from '$lib/i18n';

  type Props = {
    name: MediaAppKey;
    label?: string;
    active?: boolean;
    size?: number;
    class?: string;
  };

  const labels: Record<MediaAppKey, string> = {
    novel: m.media_label_novel(),
    comic: m.media_label_comic(),
    music: m.media_label_music(),
    video: m.media_label_video(),
    images: m.media_label_images(),
    podcast: m.media_label_podcast(),
    article: m.media_label_article(),
    local: m.media_label_local(),
  };

  let { name, label, active = false, size = 40, class: className = '' }: Props = $props();
  const resolvedLabel = $derived(label ?? labels[name]);
</script>

<svg
  class={['media-app-icon', active && 'is-active', className]}
  width={size}
  height={size}
  viewBox="0 0 64 64"
  role="img"
  aria-label={resolvedLabel}
>
  <rect class="icon-plate" x="6.5" y="6.5" width="51" height="51" rx="10" />
  <rect class="icon-frame" x="9" y="9" width="46" height="46" rx="8" />
  <path class="icon-horizon" d="M16 33h32" />

  {#if name === 'novel'}
    <path class="icon-line" d="M18 23c4.5-2.6 9.2-2.4 14 0.6v20c-4.8-3-9.5-3.2-14-0.6z" />
    <path class="icon-line" d="M32 23.6c4.8-3 9.5-3.2 14-0.6v20c-4.5-2.6-9.2-2.4-14 0.6z" />
    <path class="icon-accent" d="M32 24v20" />
  {:else if name === 'comic'}
    <path class="icon-line" d="M18 22h28v18H31l-7.5 6v-6H18z" />
    <circle class="icon-dot" cx="26" cy="31" r="1.5" />
    <circle class="icon-dot" cx="32" cy="31" r="1.5" />
    <circle class="icon-dot" cx="38" cy="31" r="1.5" />
  {:else if name === 'music'}
    <path
      class="icon-line"
      d="M24 41a5.2 5.2 0 1 1-3.3-4.8V20l20.8-4.4V34a5.2 5.2 0 1 1-3.3-4.8v-9l-17.5 3.6"
    />
  {:else if name === 'video'}
    <rect class="icon-line" x="17.5" y="21.5" width="29" height="21" rx="4" />
    <path class="icon-accent is-filled" d="m29 27 11 6-11 6z" />
  {:else if name === 'images'}
    <rect class="icon-line" x="17.5" y="20" width="29" height="24" rx="4" />
    <path class="icon-accent" d="m21 39 8.2-8 6 6.2 4.2-4.4 5.4 6.2" />
    <circle class="icon-dot" cx="38" cy="27" r="1.6" />
  {:else if name === 'podcast'}
    <path class="icon-line" d="M32 36.5a7.5 7.5 0 1 0 0-15 7.5 7.5 0 0 0 0 15z" />
    <path class="icon-line" d="M21 42.5a16.5 16.5 0 1 1 22 0" />
    <path class="icon-accent" d="M32 36.5v10" />
  {:else if name === 'article'}
    <path class="icon-line" d="M22 18h16.5l5.5 5.5V46H22z" />
    <path class="icon-line" d="M38.5 18v6h5.5" />
    <path class="icon-accent" d="M27 30.5h11M27 36.5h11M27 42.5h7" />
  {:else}
    <path class="icon-line" d="M17.5 25h13.5l4 5h11.5v16h-29z" />
    <path class="icon-line" d="M17.5 25v-5h12.5l4 5" />
  {/if}
</svg>

<style>
  .media-app-icon {
    display: inline-block;
    overflow: visible;
    color: color-mix(in oklab, var(--foreground) 82%, var(--lantern));
  }

  .icon-plate,
  .icon-frame,
  .icon-line,
  .icon-accent,
  .icon-horizon {
    fill: none;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .icon-plate {
    fill: color-mix(in oklab, var(--surface-control) 88%, var(--background));
    stroke: var(--border);
    stroke-width: 1;
  }

  .icon-frame {
    stroke: color-mix(in oklab, currentColor 50%, var(--border));
    stroke-width: 1.25;
  }

  .icon-line {
    stroke: currentColor;
    stroke-width: 2.35;
  }

  .icon-accent,
  .icon-horizon {
    stroke: var(--lantern);
    stroke-width: 2.15;
  }

  .icon-dot {
    fill: var(--lantern);
    stroke: none;
  }

  .is-filled {
    fill: color-mix(in oklab, var(--lantern) 24%, transparent);
  }

  .icon-horizon {
    opacity: 0.22;
  }

  .is-active .icon-frame {
    stroke: color-mix(in oklab, var(--lantern) 64%, currentColor);
  }

  .is-active .icon-plate {
    fill: color-mix(in oklab, var(--lantern) 10%, var(--surface-2));
  }
</style>
