<script lang="ts">
  import { m } from '$lib/i18n';
  import type { MiniPlayerSlotState, ShellPresentationMode } from './shell-types';

  type Props = {
    state: MiniPlayerSlotState;
    presentation?: ShellPresentationMode;
  };

  let { state, presentation = 'normal' }: Props = $props();
  const hiddenForReader = $derived(presentation === 'reader');
</script>

{#if state.reserved && !hiddenForReader}
  {#if state.visible}
    <!-- 有环境音频会话：可交互条，非幽灵空控件 -->
    <button
      type="button"
      class="motion-dock-wake mx-3 mb-2 min-h-[var(--shell-mini-player-height)] rounded-xl border border-hairline bg-surface-1 px-4 py-3 text-left text-sm text-ink outline-none transition-colors hover:bg-surface-2 focus-visible:bg-surface-2 md:mx-4"
      aria-label={m.mini_player_current()}
      data-mini-player="active"
    >
      <span class="block font-medium">{state.label}</span>
    </button>
  {:else}
    <!-- 无会话：纯高度占位，无按钮外观，避免「坏掉的播放器」暗示 -->
    <div
      class="min-h-[var(--shell-mini-player-height)] shrink-0"
      data-mini-player="seat"
      aria-hidden="true"
    ></div>
  {/if}
{/if}
