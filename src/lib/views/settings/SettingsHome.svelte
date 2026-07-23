<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
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
    type ThemeId,
    type ThemeMode,
  } from '$lib/stores/theme.svelte';

  let mode = $state<ThemeMode>(getMode());
  let lightThemeId = $state<ThemeId>(getLightThemeId());
  let darkThemeId = $state<ThemeId>(getDarkThemeId());
  let locale = $state<Locale>(
    (locales as readonly string[]).includes(getLocale()) ? (getLocale() as Locale) : 'zh-CN',
  );

  function chooseMode(next: ThemeMode): void {
    mode = next;
    setMode(next);
  }

  function chooseLightTheme(next: ThemeId): void {
    lightThemeId = next;
    setLightThemeId(next);
  }

  function chooseDarkTheme(next: ThemeId): void {
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
      id: def.id as ThemeId,
      label: () =>
        def.id === 'porcelain-day'
          ? m.settings_theme_porcelain_day()
          : m.settings_theme_mist_studio(),
      stone: def.stone,
    })),
  );
  const darkThemeOptions = $derived(
    listThemesForFace('dark').map((def) => ({
      id: def.id as ThemeId,
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

<section class="mx-auto flex w-full max-w-4xl flex-col gap-6" data-testid="settings-home">
  <PageHeader title={m.settings_title()} />

  <section aria-labelledby="settings-appearance-title" class="space-y-3">
    <h2 id="settings-appearance-title" class="text-sm font-semibold text-ink">
      {m.settings_appearance_group()}
    </h2>
    <div class="glass-panel divide-y divide-hairline rounded-xl border border-hairline">
      <div class="grid gap-3 px-4 py-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
        <div class="flex items-center gap-2 text-sm font-medium text-ink">
          <Icon name="sun" class="size-4 text-ink-muted" />
          <span id="settings-mode-label">{m.settings_mode_label()}</span>
        </div>
        <div class="flex flex-wrap gap-1.5" role="radiogroup" aria-labelledby="settings-mode-label">
          {#each modeOptions as opt (opt.id)}
            <button
              type="button"
              role="radio"
              class={[
                'inline-flex min-h-11 items-center gap-2 rounded-lg border px-3 text-sm outline-none focus-visible:shadow-[var(--focus-ring)] [@media(pointer:fine)]:min-h-9',
                mode === opt.id
                  ? 'border-lantern-strong/55 bg-lantern-soft/35 text-ink'
                  : 'glass-control border-hairline text-ink-muted hover:bg-surface-3 hover:text-ink',
              ]}
              aria-checked={mode === opt.id}
              tabindex={mode === opt.id ? 0 : -1}
              aria-label={opt.label()}
              onclick={() => chooseMode(opt.id)}
              onkeydown={handleRadioKeydown}
            >
              <Icon name={opt.icon} class="size-4" />
              <span>{opt.label()}</span>
            </button>
          {/each}
        </div>
      </div>

      <div class="grid gap-3 px-4 py-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
        <div class="flex items-center gap-2 text-sm font-medium text-ink">
          <Icon name="sun" class="size-4 text-ink-muted" />
          <span id="settings-light-theme-label">{m.settings_theme_light_label()}</span>
        </div>
        <div
          class="flex flex-wrap gap-1.5"
          role="radiogroup"
          aria-labelledby="settings-light-theme-label"
        >
          {#each lightThemeOptions as opt (opt.id)}
            {@const selected = lightThemeId === opt.id}
            <button
              type="button"
              role="radio"
              class={[
                'inline-flex min-h-11 items-center gap-2 rounded-lg border px-3 text-left text-sm outline-none focus-visible:shadow-[var(--focus-ring)] [@media(pointer:fine)]:min-h-9',
                selected
                  ? 'border-lantern-strong/55 bg-lantern-soft/35 text-ink'
                  : 'glass-control border-hairline text-ink-muted hover:bg-surface-3 hover:text-ink',
              ]}
              aria-checked={selected}
              tabindex={selected ? 0 : -1}
              aria-label={opt.label()}
              onclick={() => chooseLightTheme(opt.id)}
              onkeydown={handleRadioKeydown}
            >
              <span
                class="h-3.5 w-3.5 shrink-0 rounded-[5px]"
                style:background={opt.stone}
                aria-hidden="true"
              ></span>
              <span>{opt.label()}</span>
            </button>
          {/each}
        </div>
      </div>

      <div class="grid gap-3 px-4 py-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
        <div class="flex items-center gap-2 text-sm font-medium text-ink">
          <Icon name="moon" class="size-4 text-ink-muted" />
          <span id="settings-dark-theme-label">{m.settings_theme_dark_label()}</span>
        </div>
        <div
          class="flex flex-wrap gap-1.5"
          role="radiogroup"
          aria-labelledby="settings-dark-theme-label"
        >
          {#each darkThemeOptions as opt (opt.id)}
            {@const selected = darkThemeId === opt.id}
            <button
              type="button"
              role="radio"
              class={[
                'inline-flex min-h-11 items-center gap-2 rounded-lg border px-3 text-left text-sm outline-none focus-visible:shadow-[var(--focus-ring)] [@media(pointer:fine)]:min-h-9',
                selected
                  ? 'border-lantern-strong/55 bg-lantern-soft/35 text-ink'
                  : 'glass-control border-hairline text-ink-muted hover:bg-surface-3 hover:text-ink',
              ]}
              aria-checked={selected}
              tabindex={selected ? 0 : -1}
              aria-label={opt.label()}
              onclick={() => chooseDarkTheme(opt.id)}
              onkeydown={handleRadioKeydown}
            >
              <span
                class="h-3.5 w-3.5 shrink-0 rounded-[5px]"
                style:background={opt.stone}
                aria-hidden="true"
              ></span>
              <span>{opt.label()}</span>
            </button>
          {/each}
        </div>
      </div>
    </div>
  </section>

  <section aria-labelledby="settings-language-title" class="space-y-3">
    <h2 id="settings-language-title" class="text-sm font-semibold text-ink">
      {m.settings_language_group()}
    </h2>
    <div class="glass-panel rounded-xl border border-hairline px-4 py-4">
      <div class="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
        <div class="flex items-center gap-2 text-sm font-medium text-ink">
          <Icon name="translate" class="size-4 text-ink-muted" />
          <span id="settings-lang-label">{m.settings_language()}</span>
        </div>
        <div class="flex flex-wrap gap-1.5" role="radiogroup" aria-labelledby="settings-lang-label">
          {#each langOptions as opt (opt.id)}
            <button
              type="button"
              role="radio"
              class={[
                'min-h-11 rounded-lg border px-3 text-sm font-medium outline-none focus-visible:shadow-[var(--focus-ring)] [@media(pointer:fine)]:min-h-9',
                locale === opt.id
                  ? 'border-lantern-strong/55 bg-lantern-soft/35 text-ink'
                  : 'glass-control border-hairline text-ink-muted hover:bg-surface-3 hover:text-ink',
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
</section>
