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
  let themeId = $state<AppearancePackId>(getAppearancePack().id);

  function chooseMode(next: ThemeMode) {
    mode = next;
    setMode(next);
  }

  function chooseTheme(next: AppearancePackId) {
    themeId = next;
    setAppearancePack({ id: next });
  }

  const modeOptions: { id: ThemeMode; label: () => string }[] = [
    { id: 'light', label: () => m.theme_mode_light() },
    { id: 'dark', label: () => m.theme_mode_dark() },
    { id: 'system', label: () => m.theme_mode_system() },
  ];

  /** 圆形饼图预览：canvas / surface / lantern 三瓣，一眼辨主题 */
  const themeOptions: {
    id: AppearancePackId;
    label: () => string;
    slices: [string, string, string];
  }[] = [
    {
      id: 'inkstone-precision',
      label: () => m.settings_theme_inkstone(),
      slices: ['#f4f5f5', '#eef0f0', '#2a6f7a'],
    },
    {
      id: 'cold-cinnabar',
      label: () => m.settings_theme_cinnabar(),
      slices: ['#f2f2f0', '#ecece8', '#c45a3c'],
    },
  ];

  function pieStyle(slices: [string, string, string]): string {
    const [a, b, c] = slices;
    return `background: conic-gradient(${a} 0deg 120deg, ${b} 120deg 240deg, ${c} 240deg 360deg)`;
  }
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
      <p class="text-xs font-medium text-ink-muted">{m.settings_mode_label()}</p>
      <div class="flex flex-wrap gap-2" role="group" aria-label={m.settings_mode_label()}>
        {#each modeOptions as opt (opt.id)}
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
      <p class="text-xs font-medium text-ink-muted">{m.settings_theme_label()}</p>
      <div class="flex flex-wrap gap-4" role="group" aria-label={m.settings_theme_label()}>
        {#each themeOptions as opt (opt.id)}
          <button
            type="button"
            class="motion-nav-capsule group flex flex-col items-center gap-2 rounded-xl p-1 outline-none focus-visible:shadow-[var(--focus-ring)]"
            aria-pressed={themeId === opt.id}
            aria-label={opt.label()}
            onclick={() => chooseTheme(opt.id)}
          >
            <span
              class={[
                'relative inline-flex size-14 shrink-0 rounded-full border-2 shadow-[inset_0_0_0_1px_rgb(0_0_0/0.06)] transition-[box-shadow,border-color]',
                themeId === opt.id
                  ? 'border-lantern shadow-[0_0_0_3px_var(--lantern-soft)]'
                  : 'border-hairline group-hover:border-ink-muted',
              ]}
              style={pieStyle(opt.slices)}
              aria-hidden="true"
            ></span>
            <span
              class={['text-xs font-medium', themeId === opt.id ? 'text-ink' : 'text-ink-muted']}
            >
              {opt.label()}
            </span>
          </button>
        {/each}
      </div>
    </div>
  </section>
</section>
