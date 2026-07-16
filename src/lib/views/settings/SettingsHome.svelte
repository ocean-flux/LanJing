<script lang="ts">
  import { m } from '$lib/i18n';
  import {
    getAppearancePack,
    getMode,
    setAppearancePack,
    setMode,
    type AppearancePackId,
    type ThemeMode,
  } from '$lib/stores/theme.svelte';

  let mode = $state<ThemeMode>(getMode());
  let packId = $state<AppearancePackId>(getAppearancePack().id);

  function chooseMode(next: ThemeMode) {
    mode = next;
    setMode(next);
  }

  function choosePack(next: AppearancePackId) {
    packId = next;
    setAppearancePack({ id: next });
  }

  const themeOptions: { id: ThemeMode; label: () => string }[] = [
    { id: 'light', label: () => m.theme_mode_light() },
    { id: 'dark', label: () => m.theme_mode_dark() },
    { id: 'system', label: () => m.theme_mode_system() },
  ];

  const packOptions: { id: AppearancePackId; label: () => string }[] = [
    { id: 'inkstone-precision', label: () => m.settings_pack_inkstone() },
    { id: 'cold-cinnabar', label: () => m.settings_pack_cinnabar() },
  ];
</script>

<section class="mx-auto flex max-w-xl flex-col gap-4 py-4" data-testid="settings-home">
  <h1 class="text-xl font-semibold tracking-tight text-ink">{m.settings_title()}</h1>

  <section
    class="surface-panel flex flex-col gap-5 p-4 md:p-5"
    aria-labelledby="settings-appearance-title"
  >
    <h2 id="settings-appearance-title" class="text-sm font-semibold text-ink">
      {m.settings_appearance()}
    </h2>

    <div class="flex flex-col gap-2">
      <p class="text-xs font-medium text-ink-muted">{m.settings_theme_label()}</p>
      <div class="flex flex-wrap gap-2" role="group" aria-label={m.settings_theme_label()}>
        {#each themeOptions as opt (opt.id)}
          <button
            type="button"
            class={[
              'motion-nav-capsule inline-flex min-h-9 items-center rounded-lg border border-hairline px-3 text-sm font-medium outline-none transition-colors focus-visible:shadow-[var(--focus-ring)]',
              mode === opt.id
                ? 'border-transparent bg-lantern-soft text-ink'
                : 'bg-surface-1 text-ink-muted hover:bg-surface-3 hover:text-ink',
            ]}
            aria-pressed={mode === opt.id}
            onclick={() => chooseMode(opt.id)}
          >
            {opt.label()}
          </button>
        {/each}
      </div>
    </div>

    <div class="flex flex-col gap-2">
      <p class="text-xs font-medium text-ink-muted">{m.settings_pack_label()}</p>
      <div class="flex flex-wrap gap-2" role="group" aria-label={m.settings_pack_label()}>
        {#each packOptions as opt (opt.id)}
          <button
            type="button"
            class={[
              'motion-nav-capsule inline-flex min-h-9 items-center rounded-lg border border-hairline px-3 text-sm font-medium outline-none transition-colors focus-visible:shadow-[var(--focus-ring)]',
              packId === opt.id
                ? 'border-transparent bg-lantern-soft text-ink'
                : 'bg-surface-1 text-ink-muted hover:bg-surface-3 hover:text-ink',
            ]}
            aria-pressed={packId === opt.id}
            onclick={() => choosePack(opt.id)}
          >
            {opt.label()}
          </button>
        {/each}
      </div>
    </div>
  </section>
</section>
