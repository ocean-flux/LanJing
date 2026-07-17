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

  const modeOptions: {
    id: ThemeMode;
    label: () => string;
    icon: typeof Sun;
  }[] = [
    { id: 'light', label: () => m.theme_mode_light(), icon: Sun },
    { id: 'dark', label: () => m.theme_mode_dark(), icon: Moon },
    { id: 'system', label: () => m.theme_mode_system(), icon: Monitor },
  ];

  const themeOptions: {
    id: ThemeId;
    label: () => string;
    stone: string;
  }[] = [
    {
      id: 'inkstone-precision',
      label: () => m.settings_theme_inkstone(),
      stone: '#2a6f7a',
    },
    {
      id: 'cold-cinnabar',
      label: () => m.settings_theme_cinnabar(),
      stone: '#c45a3c',
    },
  ];

  const langOptions: { id: Locale; label: () => string }[] = [
    { id: 'zh-CN', label: () => m.settings_lang_zh() },
    { id: 'en', label: () => m.settings_lang_en() },
  ];
</script>

<!-- 墨台 denselist：无页内 H1、无 stage 卡、无快捷四格 -->
<section class="w-full max-w-none" data-testid="settings-home" aria-label={m.settings_title()}>
  <div class="border-t border-hairline">
    <!-- 明暗模式 -->
    <div
      class="grid min-h-11 grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-2 border-b border-hairline py-1"
    >
      <div class="grid h-7 w-7 place-items-center text-ink-muted" aria-hidden="true">
        <Sun size={18} strokeWidth={1.75} />
      </div>
      <div class="min-w-0 text-[0.8125rem] font-medium text-ink" id="settings-mode-label">
        {m.settings_mode_label()}
      </div>
      <div
        class="inline-flex rounded-lg border border-hairline bg-surface-3 p-0.5"
        role="radiogroup"
        aria-labelledby="settings-mode-label"
      >
        {#each modeOptions as opt (opt.id)}
          {@const Icon = opt.icon}
          <button
            type="button"
            role="radio"
            class={[
              'motion-nav-capsule grid h-8 w-9 place-items-center rounded-md outline-none transition-colors focus-visible:shadow-[var(--focus-ring)] [@media(pointer:coarse)]:h-11 [@media(pointer:coarse)]:w-11',
              mode === opt.id
                ? 'bg-surface-1 text-ink shadow-[var(--surface-panel-shadow)]'
                : 'text-ink-muted hover:text-ink',
            ]}
            aria-checked={mode === opt.id}
            aria-label={opt.label()}
            title={opt.label()}
            onclick={() => chooseMode(opt.id)}
          >
            <Icon size={16} strokeWidth={1.75} aria-hidden="true" />
          </button>
        {/each}
      </div>
    </div>

    <!-- 亮色主题 -->
    <div
      class="grid min-h-11 grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-2 border-b border-hairline py-1"
    >
      <div class="grid h-7 w-7 place-items-center text-ink-muted" aria-hidden="true">
        <span class="size-3.5 rounded-full border border-hairline bg-lantern"></span>
      </div>
      <div class="min-w-0 text-[0.8125rem] font-medium text-ink" id="settings-light-theme-label">
        {m.settings_theme_light_label()}
      </div>
      <div
        class="flex items-center gap-1.5"
        role="radiogroup"
        aria-labelledby="settings-light-theme-label"
      >
        {#each themeOptions as opt (opt.id)}
          {@const selected = lightThemeId === opt.id}
          <button
            type="button"
            role="radio"
            class={[
              'relative grid size-7 place-items-center rounded-full border-2 outline-none transition-colors focus-visible:shadow-[var(--focus-ring)] [@media(pointer:coarse)]:size-11',
              selected ? 'border-lantern' : 'border-transparent',
            ]}
            aria-checked={selected}
            aria-label={opt.label()}
            title={opt.label()}
            onclick={() => chooseLightTheme(opt.id)}
          >
            <span
              class="absolute inset-[3px] rounded-full shadow-[inset_0_0_0_1px_rgb(0_0_0/0.12)]"
              style:background={opt.stone}
              aria-hidden="true"
            >
              <span
                class="absolute left-1/2 top-1/2 size-2 -translate-x-1/2 -translate-y-1/2 rounded-full bg-canvas"
              ></span>
            </span>
          </button>
        {/each}
      </div>
    </div>

    <!-- 暗色主题 -->
    <div
      class="grid min-h-11 grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-2 border-b border-hairline py-1"
    >
      <div class="grid h-7 w-7 place-items-center text-ink-muted" aria-hidden="true">
        <Moon size={18} strokeWidth={1.75} />
      </div>
      <div class="min-w-0 text-[0.8125rem] font-medium text-ink" id="settings-dark-theme-label">
        {m.settings_theme_dark_label()}
      </div>
      <div
        class="flex items-center gap-1.5"
        role="radiogroup"
        aria-labelledby="settings-dark-theme-label"
      >
        {#each themeOptions as opt (opt.id)}
          {@const selected = darkThemeId === opt.id}
          <button
            type="button"
            role="radio"
            class={[
              'relative grid size-7 place-items-center rounded-full border-2 outline-none transition-colors focus-visible:shadow-[var(--focus-ring)] [@media(pointer:coarse)]:size-11',
              selected ? 'border-lantern' : 'border-transparent',
            ]}
            aria-checked={selected}
            aria-label={opt.label()}
            title={opt.label()}
            onclick={() => chooseDarkTheme(opt.id)}
          >
            <span
              class="absolute inset-[3px] rounded-full shadow-[inset_0_0_0_1px_rgb(0_0_0/0.12)]"
              style:background={opt.stone}
              aria-hidden="true"
            >
              <span
                class="absolute left-1/2 top-1/2 size-2 -translate-x-1/2 -translate-y-1/2 rounded-full bg-canvas"
              ></span>
            </span>
          </button>
        {/each}
      </div>
    </div>

    <!-- 界面语言 -->
    <div
      class="grid min-h-11 grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-2 border-b border-hairline py-1"
    >
      <div class="grid h-7 w-7 place-items-center text-ink-muted" aria-hidden="true">
        <Languages size={18} strokeWidth={1.75} />
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
              'motion-nav-capsule px-1 py-1 font-medium outline-none focus-visible:rounded-sm focus-visible:shadow-[var(--focus-ring)] [@media(pointer:coarse)]:min-h-11 [@media(pointer:coarse)]:px-2',
              index > 0 && 'ml-2 border-l border-hairline pl-2',
              locale === opt.id
                ? 'font-bold text-lantern-strong'
                : 'text-ink-subtle hover:text-ink',
            ]}
            aria-checked={locale === opt.id}
            onclick={() => chooseLocale(opt.id)}
          >
            {opt.label()}
          </button>
        {/each}
      </div>
    </div>
  </div>
</section>
