<script lang="ts">
  import Languages from '@lucide/svelte/icons/languages';
  import Monitor from '@lucide/svelte/icons/monitor';
  import Moon from '@lucide/svelte/icons/moon';
  import Sun from '@lucide/svelte/icons/sun';
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

  function chooseMode(next: ThemeMode) {
    mode = next;
    setMode(next);
  }

  function chooseLightTheme(next: ThemeId) {
    lightThemeId = next;
    setLightThemeId(next);
  }

  function chooseDarkTheme(next: ThemeId) {
    darkThemeId = next;
    setDarkThemeId(next);
  }

  function chooseLocale(next: Locale) {
    if (next === locale) return;
    locale = next;
    // paraglide 默认重载以切换文案包
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
    icon: typeof Sun;
  }[] = [
    { id: 'light', label: () => m.theme_mode_light(), icon: Sun },
    { id: 'dark', label: () => m.theme_mode_dark(), icon: Moon },
    { id: 'system', label: () => m.theme_mode_system(), icon: Monitor },
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

<!-- Ethereal denselist：无页内 H1、无 stage 卡、无快捷四格；double-bezel 密列表 -->
<section class="w-full max-w-none" data-testid="settings-home" aria-label={m.settings_title()}>
  <div
    class="double-bezel divide-y divide-hairline overflow-hidden"
    data-testid="settings-denselist"
  >
    <!-- 明暗模式 -->
    <div class="grid min-h-12 grid-cols-[2rem_minmax(0,1fr)_auto] items-center gap-3 px-3 py-2.5">
      <div
        class="grid h-8 w-8 place-items-center rounded-lg border border-hairline bg-surface-2 text-ink-muted"
        aria-hidden="true"
      >
        <Sun size={16} strokeWidth={1.75} />
      </div>
      <div class="min-w-0 text-[0.8125rem] font-medium text-ink" id="settings-mode-label">
        {m.settings_mode_label()}
      </div>
      <div
        class="inline-flex rounded-lg border border-hairline bg-surface-2 p-0.5 shadow-[inset_0_1px_0_color-mix(in_oklab,white_5%,transparent)]"
        role="radiogroup"
        aria-labelledby="settings-mode-label"
      >
        {#each modeOptions as opt (opt.id)}
          {@const Icon = opt.icon}
          <button
            type="button"
            role="radio"
            class={[
              'motion-nav-capsule grid h-11 w-11 place-items-center rounded-md outline-none transition-colors focus-visible:shadow-[var(--focus-ring)] sm:h-8 sm:w-9 [@media(pointer:coarse)]:h-11 [@media(pointer:coarse)]:w-11',
              mode === opt.id
                ? 'bg-lantern-soft text-ink shadow-[var(--surface-panel-shadow)]'
                : 'text-ink-muted hover:bg-surface-3/70 hover:text-ink',
            ]}
            aria-checked={mode === opt.id}
            tabindex={mode === opt.id ? 0 : -1}
            aria-label={opt.label()}
            title={opt.label()}
            onclick={() => chooseMode(opt.id)}
            onkeydown={handleRadioKeydown}
          >
            <Icon size={16} strokeWidth={1.75} aria-hidden="true" />
          </button>
        {/each}
      </div>
    </div>

    <!-- 亮色主题：方圆色条芯片，禁用圆环色石 -->
    <div class="grid min-h-12 grid-cols-[2rem_minmax(0,1fr)_auto] items-center gap-3 px-3 py-2.5">
      <div
        class="grid h-8 w-8 place-items-center rounded-lg border border-hairline bg-surface-2 text-ink-muted"
        aria-hidden="true"
      >
        <Sun size={16} strokeWidth={1.75} />
      </div>
      <div class="min-w-0 text-[0.8125rem] font-medium text-ink" id="settings-light-theme-label">
        {m.settings_theme_light_label()}
      </div>
      <div
        class="flex max-w-[min(100%,18rem)] flex-wrap items-center justify-end gap-1.5"
        role="radiogroup"
        aria-labelledby="settings-light-theme-label"
      >
        {#each lightThemeOptions as opt (opt.id)}
          {@const selected = lightThemeId === opt.id}
          <button
            type="button"
            role="radio"
            class={[
              'theme-chip motion-nav-capsule inline-flex min-h-11 max-w-[9.5rem] items-center gap-2 rounded-lg border px-2 py-1 text-left outline-none transition-[border-color,background-color,box-shadow] focus-visible:shadow-[var(--focus-ring)] sm:min-h-8 [@media(pointer:coarse)]:min-h-11',
              selected
                ? 'border-lantern-strong/55 bg-lantern-soft/35 text-ink'
                : 'border-hairline bg-surface-2/70 text-ink-muted hover:border-hairline-strong hover:bg-surface-3/60 hover:text-ink',
            ]}
            aria-checked={selected}
            tabindex={selected ? 0 : -1}
            aria-label={opt.label()}
            title={opt.label()}
            onclick={() => chooseLightTheme(opt.id)}
            onkeydown={handleRadioKeydown}
          >
            <span
              class="theme-chip-swatch h-3.5 w-3.5 shrink-0 rounded-[5px]"
              style:background={opt.stone}
              aria-hidden="true"
            ></span>
            <span class="min-w-0 truncate text-[0.7rem] font-medium leading-none tracking-tight">
              {opt.label()}
            </span>
          </button>
        {/each}
      </div>
    </div>

    <!-- 暗色主题：同上芯片，无圆环 -->
    <div class="grid min-h-12 grid-cols-[2rem_minmax(0,1fr)_auto] items-center gap-3 px-3 py-2.5">
      <div
        class="grid h-8 w-8 place-items-center rounded-lg border border-hairline bg-surface-2 text-ink-muted"
        aria-hidden="true"
      >
        <Moon size={16} strokeWidth={1.75} />
      </div>
      <div class="min-w-0 text-[0.8125rem] font-medium text-ink" id="settings-dark-theme-label">
        {m.settings_theme_dark_label()}
      </div>
      <div
        class="flex max-w-[min(100%,18rem)] flex-wrap items-center justify-end gap-1.5"
        role="radiogroup"
        aria-labelledby="settings-dark-theme-label"
      >
        {#each darkThemeOptions as opt (opt.id)}
          {@const selected = darkThemeId === opt.id}
          <button
            type="button"
            role="radio"
            class={[
              'theme-chip motion-nav-capsule inline-flex min-h-11 max-w-[9.5rem] items-center gap-2 rounded-lg border px-2 py-1 text-left outline-none transition-[border-color,background-color,box-shadow] focus-visible:shadow-[var(--focus-ring)] sm:min-h-8 [@media(pointer:coarse)]:min-h-11',
              selected
                ? 'border-lantern-strong/55 bg-lantern-soft/35 text-ink'
                : 'border-hairline bg-surface-2/70 text-ink-muted hover:border-hairline-strong hover:bg-surface-3/60 hover:text-ink',
            ]}
            aria-checked={selected}
            tabindex={selected ? 0 : -1}
            aria-label={opt.label()}
            title={opt.label()}
            onclick={() => chooseDarkTheme(opt.id)}
            onkeydown={handleRadioKeydown}
          >
            <span
              class="theme-chip-swatch h-3.5 w-3.5 shrink-0 rounded-[5px]"
              style:background={opt.stone}
              aria-hidden="true"
            ></span>
            <span class="min-w-0 truncate text-[0.7rem] font-medium leading-none tracking-tight">
              {opt.label()}
            </span>
          </button>
        {/each}
      </div>
    </div>

    <!-- 界面语言 -->
    <div class="grid min-h-12 grid-cols-[2rem_minmax(0,1fr)_auto] items-center gap-3 px-3 py-2.5">
      <div
        class="grid h-8 w-8 place-items-center rounded-lg border border-hairline bg-surface-2 text-ink-muted"
        aria-hidden="true"
      >
        <Languages size={16} strokeWidth={1.75} />
      </div>
      <div class="min-w-0 text-[0.8125rem] font-medium text-ink" id="settings-lang-label">
        {m.settings_language()}
      </div>
      <div
        class="inline-flex items-baseline text-[0.8125rem]"
        role="radiogroup"
        aria-labelledby="settings-lang-label"
      >
        {#each langOptions as opt, index (opt.id)}
          <button
            type="button"
            role="radio"
            class={[
              'motion-nav-capsule min-h-11 min-w-11 rounded-md px-2 py-1 font-medium outline-none transition-colors focus-visible:shadow-[var(--focus-ring)] sm:min-h-8 sm:min-w-0 [@media(pointer:coarse)]:min-h-11 [@media(pointer:coarse)]:min-w-11 [@media(pointer:coarse)]:px-2.5',
              index > 0 && 'ml-1.5 border-l border-hairline pl-2.5',
              locale === opt.id
                ? 'font-semibold text-lantern-strong'
                : 'text-ink-subtle hover:text-ink',
            ]}
            aria-checked={locale === opt.id}
            tabindex={locale === opt.id ? 0 : -1}
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

<style>
  .theme-chip-swatch {
    box-shadow:
      inset 0 0 0 1px color-mix(in oklab, var(--ink) 12%, transparent),
      0 0 0 1px color-mix(in oklab, var(--canvas) 35%, transparent);
  }
</style>
