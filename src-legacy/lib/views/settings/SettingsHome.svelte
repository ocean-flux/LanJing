<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
  import PageFrame from '$lib/components/PageFrame.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { getLocale, locales, m, setLocale, type Locale } from '$lib/i18n';
  import {
    getDarkThemeId,
    getLightThemeId,
    getMode,
    listThemesForFace,
    setDarkThemeId,
    setLightThemeId,
    setMode,
    type AppearancePackId,
    type ThemeMode,
  } from '$lib/stores/theme.svelte';

  let mode = $state<ThemeMode>(getMode());
  let lightThemeId = $state<AppearancePackId>(getLightThemeId());
  let darkThemeId = $state<AppearancePackId>(getDarkThemeId());
  let locale = $state<Locale>(
    (locales as readonly string[]).includes(getLocale()) ? (getLocale() as Locale) : 'zh-CN',
  );

  function chooseMode(next: ThemeMode): void {
    mode = next;
    setMode(next);
  }

  function chooseLightTheme(next: AppearancePackId): void {
    lightThemeId = next;
    setLightThemeId(next);
  }

  function chooseDarkTheme(next: AppearancePackId): void {
    darkThemeId = next;
    setDarkThemeId(next);
  }

  function chooseLocale(next: Locale): void {
    if (next === locale) return;
    locale = next;
    // Paraglide 默认重载以切换文案包。
    setLocale(next);
  }

  function handleRadioKeydown(event: KeyboardEvent & { currentTarget: HTMLButtonElement }): void {
    const group = event.currentTarget.closest('[role="radiogroup"]');
    if (!group) return;

    const radios = Array.from(group.querySelectorAll<HTMLButtonElement>('[role="radio"]'));
    const currentIndex = radios.indexOf(event.currentTarget);
    if (currentIndex < 0) return;

    let nextIndex: number | null = null;
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
      nextIndex = (currentIndex + 1) % radios.length;
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
      nextIndex = (currentIndex - 1 + radios.length) % radios.length;
    } else if (event.key === 'Home') {
      nextIndex = 0;
    } else if (event.key === 'End') {
      nextIndex = radios.length - 1;
    }

    if (nextIndex === null) return;
    event.preventDefault();
    radios[nextIndex]?.focus();
    radios[nextIndex]?.click();
  }

  const modeOptions: {
    id: ThemeMode;
    label: () => string;
    icon: 'sun' | 'moon' | 'monitor';
  }[] = [
    { id: 'light', label: () => m.theme_mode_light(), icon: 'sun' },
    { id: 'dark', label: () => m.theme_mode_dark(), icon: 'moon' },
    { id: 'system', label: () => m.theme_mode_system(), icon: 'monitor' },
  ];

  const lightThemeOptions = $derived(
    listThemesForFace('light').map((def) => ({
      id: def.id as AppearancePackId,
      label: () =>
        def.id === 'porcelain-day'
          ? m.settings_theme_porcelain_day()
          : m.settings_theme_mist_studio(),
      stone: def.stone,
    })),
  );
  const darkThemeOptions = $derived(
    listThemesForFace('dark').map((def) => ({
      id: def.id as AppearancePackId,
      label: () =>
        def.id === 'obsidian-void'
          ? m.settings_theme_obsidian_void()
          : m.settings_theme_graphite_atelier(),
      stone: def.stone,
    })),
  );

  const langOptions: { id: Locale; label: () => string }[] = [
    { id: 'zh-CN', label: () => m.settings_lang_zh() },
    { id: 'en', label: () => m.settings_lang_en() },
  ];
</script>

<PageFrame width="standard">
  <PageHeader title={m.settings_title()} />

  <div
    class="grid min-w-0 gap-(--section-gap) lg:grid-cols-[minmax(0,3fr)_minmax(18rem,2fr)] lg:items-start"
  >
    <section aria-labelledby="settings-appearance-title" class="min-w-0">
      <h2 id="settings-appearance-title" class="mb-2 text-xs font-semibold text-ink-muted">
        {m.settings_appearance_group()}
      </h2>
      <div
        class="glass-panel divide-y divide-hairline overflow-hidden rounded-[var(--radius-panel)] border border-hairline"
      >
        <div
          class="grid min-w-0 gap-(--density-group-gap) p-(--density-panel-padding-compact) sm:grid-cols-[minmax(7rem,1fr)_minmax(0,2fr)] sm:items-center"
        >
          <div class="flex min-w-0 items-center gap-2 text-sm font-medium text-ink">
            <Icon name="sun" class="size-4 shrink-0 text-ink-muted" />
            <span id="settings-mode-label">{m.settings_mode_label()}</span>
          </div>
          <div
            class="grid min-w-0 grid-cols-3 gap-0.5 rounded-[var(--radius-control)] border border-hairline bg-surface-2 p-0.5"
            role="radiogroup"
            aria-labelledby="settings-mode-label"
          >
            {#each modeOptions as opt (opt.id)}
              <button
                type="button"
                role="radio"
                class={[
                  'inline-flex min-h-(--density-control-sm) min-w-0 items-center justify-center gap-1 rounded-[var(--radius-control)] border px-2 py-1 text-center text-sm leading-4 font-medium whitespace-normal transition-[color,background-color,border-color,box-shadow] duration-(--motion-fast) outline-none focus-visible:relative focus-visible:z-10 focus-visible:border-lantern-strong/60 focus-visible:shadow-[var(--focus-ring)] disabled:pointer-events-none disabled:cursor-not-allowed disabled:border-transparent disabled:bg-transparent disabled:text-ink-subtle disabled:opacity-50',
                  mode === opt.id
                    ? 'border-[var(--surface-selected-border)] bg-[var(--surface-selected)] text-ink'
                    : 'border-transparent bg-transparent text-ink-muted hover:border-hairline-strong hover:bg-surface-3 hover:text-ink',
                ]}
                aria-checked={mode === opt.id}
                tabindex={mode === opt.id ? 0 : -1}
                aria-label={opt.label()}
                onclick={() => chooseMode(opt.id)}
                onkeydown={handleRadioKeydown}
              >
                <Icon name={opt.icon} class="size-4 shrink-0" />
                <span class="min-w-0">{opt.label()}</span>
              </button>
            {/each}
          </div>
        </div>

        <div
          class="grid min-w-0 gap-(--density-group-gap) p-(--density-panel-padding-compact) sm:grid-cols-[minmax(7rem,1fr)_minmax(0,2fr)] sm:items-center"
        >
          <div class="flex min-w-0 items-center gap-2 text-sm font-medium text-ink">
            <Icon name="sun" class="size-4 shrink-0 text-ink-muted" />
            <span id="settings-light-theme-label">{m.settings_theme_light_label()}</span>
          </div>
          <div
            class="grid min-w-0 grid-cols-2 gap-0.5 rounded-[var(--radius-control)] border border-hairline bg-surface-2 p-0.5"
            role="radiogroup"
            aria-labelledby="settings-light-theme-label"
          >
            {#each lightThemeOptions as opt (opt.id)}
              {@const selected = lightThemeId === opt.id}
              <button
                type="button"
                role="radio"
                class={[
                  'inline-flex min-h-(--density-control-sm) min-w-0 items-center justify-center gap-1 rounded-[var(--radius-control)] border px-2 py-1 text-center text-sm leading-4 font-medium whitespace-normal transition-[color,background-color,border-color,box-shadow] duration-(--motion-fast) outline-none focus-visible:relative focus-visible:z-10 focus-visible:border-lantern-strong/60 focus-visible:shadow-[var(--focus-ring)] disabled:pointer-events-none disabled:cursor-not-allowed disabled:border-transparent disabled:bg-transparent disabled:text-ink-subtle disabled:opacity-50',
                  selected
                    ? 'border-[var(--surface-selected-border)] bg-[var(--surface-selected)] text-ink'
                    : 'border-transparent bg-transparent text-ink-muted hover:border-hairline-strong hover:bg-surface-3 hover:text-ink',
                ]}
                aria-checked={selected}
                tabindex={selected ? 0 : -1}
                aria-label={opt.label()}
                onclick={() => chooseLightTheme(opt.id)}
                onkeydown={handleRadioKeydown}
              >
                <span
                  class="size-3.5 shrink-0 rounded-sm border border-hairline-strong"
                  style:background={opt.stone}
                  aria-hidden="true"
                ></span>
                <span class="min-w-0">{opt.label()}</span>
              </button>
            {/each}
          </div>
        </div>

        <div
          class="grid min-w-0 gap-(--density-group-gap) p-(--density-panel-padding-compact) sm:grid-cols-[minmax(7rem,1fr)_minmax(0,2fr)] sm:items-center"
        >
          <div class="flex min-w-0 items-center gap-2 text-sm font-medium text-ink">
            <Icon name="moon" class="size-4 shrink-0 text-ink-muted" />
            <span id="settings-dark-theme-label">{m.settings_theme_dark_label()}</span>
          </div>
          <div
            class="grid min-w-0 grid-cols-2 gap-0.5 rounded-[var(--radius-control)] border border-hairline bg-surface-2 p-0.5"
            role="radiogroup"
            aria-labelledby="settings-dark-theme-label"
          >
            {#each darkThemeOptions as opt (opt.id)}
              {@const selected = darkThemeId === opt.id}
              <button
                type="button"
                role="radio"
                class={[
                  'inline-flex min-h-(--density-control-sm) min-w-0 items-center justify-center gap-1 rounded-[var(--radius-control)] border px-2 py-1 text-center text-sm leading-4 font-medium whitespace-normal transition-[color,background-color,border-color,box-shadow] duration-(--motion-fast) outline-none focus-visible:relative focus-visible:z-10 focus-visible:border-lantern-strong/60 focus-visible:shadow-[var(--focus-ring)] disabled:pointer-events-none disabled:cursor-not-allowed disabled:border-transparent disabled:bg-transparent disabled:text-ink-subtle disabled:opacity-50',
                  selected
                    ? 'border-[var(--surface-selected-border)] bg-[var(--surface-selected)] text-ink'
                    : 'border-transparent bg-transparent text-ink-muted hover:border-hairline-strong hover:bg-surface-3 hover:text-ink',
                ]}
                aria-checked={selected}
                tabindex={selected ? 0 : -1}
                aria-label={opt.label()}
                onclick={() => chooseDarkTheme(opt.id)}
                onkeydown={handleRadioKeydown}
              >
                <span
                  class="size-3.5 shrink-0 rounded-sm border border-hairline-strong"
                  style:background={opt.stone}
                  aria-hidden="true"
                ></span>
                <span class="min-w-0">{opt.label()}</span>
              </button>
            {/each}
          </div>
        </div>
      </div>
    </section>

    <section aria-labelledby="settings-language-title" class="min-w-0">
      <h2 id="settings-language-title" class="mb-2 text-xs font-semibold text-ink-muted">
        {m.settings_language_group()}
      </h2>
      <div class="glass-panel overflow-hidden rounded-[var(--radius-panel)] border border-hairline">
        <div
          class="grid min-w-0 gap-(--density-group-gap) p-(--density-panel-padding-compact) sm:grid-cols-[minmax(7rem,1fr)_minmax(0,2fr)] sm:items-center"
        >
          <div class="flex min-w-0 items-center gap-2 text-sm font-medium text-ink">
            <Icon name="translate" class="size-4 shrink-0 text-ink-muted" />
            <span id="settings-lang-label">{m.settings_language()}</span>
          </div>
          <div
            class="grid min-w-0 grid-cols-2 gap-0.5 rounded-[var(--radius-control)] border border-hairline bg-surface-2 p-0.5"
            role="radiogroup"
            aria-labelledby="settings-lang-label"
          >
            {#each langOptions as opt (opt.id)}
              <button
                type="button"
                role="radio"
                class={[
                  'inline-flex min-h-(--density-control-sm) min-w-0 items-center justify-center rounded-[var(--radius-control)] border px-2 py-1 text-center text-sm leading-4 font-medium whitespace-normal transition-[color,background-color,border-color,box-shadow] duration-(--motion-fast) outline-none focus-visible:relative focus-visible:z-10 focus-visible:border-lantern-strong/60 focus-visible:shadow-[var(--focus-ring)] disabled:pointer-events-none disabled:cursor-not-allowed disabled:border-transparent disabled:bg-transparent disabled:text-ink-subtle disabled:opacity-50',
                  locale === opt.id
                    ? 'border-[var(--surface-selected-border)] bg-[var(--surface-selected)] text-ink'
                    : 'border-transparent bg-transparent text-ink-muted hover:border-hairline-strong hover:bg-surface-3 hover:text-ink',
                ]}
                aria-checked={locale === opt.id}
                tabindex={locale === opt.id ? 0 : -1}
                aria-label={opt.label()}
                onclick={() => chooseLocale(opt.id)}
                onkeydown={handleRadioKeydown}
              >
                {opt.label()}
              </button>
            {/each}
          </div>
        </div>
      </div>
    </section>
  </div>
</PageFrame>
