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
  <button
    type="button"
    class={[
      'motion-dock-wake mx-3 mb-2 min-h-[var(--shell-mini-player-height)] rounded-xl border border-hairline bg-surface-1 px-4 py-3 text-left text-sm text-ink-muted outline-none transition-colors hover:bg-surface-2 hover:text-ink focus-visible:text-ink md:mx-4',
      !state.visible && 'opacity-72',
    ]}
    aria-label={state.visible ? m.mini_player_current() : m.mini_player_empty()}
    aria-disabled={!state.visible}
    data-mini-player={state.visible ? 'active' : 'reserved'}
  >
    <span class="block font-medium text-ink">{state.label}</span>
    {#if !state.visible}
      <span class="mt-1 block text-xs">{m.mini_player_empty()}</span>
    {/if}
  </button>
{/if}
