<script lang="ts">
  import { resolve } from '$app/paths';
  import { BookOpen, Boxes, Database, Radio } from '@lucide/svelte';
  import { getLocale, locales, m, setLocale, type Locale } from '$lib/i18n';
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
  let locale = $state<Locale>(
    (locales as readonly string[]).includes(getLocale()) ? (getLocale() as Locale) : 'zh-CN',
  );

  function chooseMode(next: ThemeMode) {
    mode = next;
    setMode(next);
  }

  function chooseTheme(next: AppearancePackId) {
    themeId = next;
    setAppearancePack({ id: next });
  }

  function chooseLocale(next: Locale) {
    if (next === locale) return;
    locale = next;
    // paraglide 默认重载以切换文案包
    setLocale(next);
  }

  const modeOptions: { id: ThemeMode; label: () => string }[] = [
    { id: 'light', label: () => m.theme_mode_light() },
    { id: 'dark', label: () => m.theme_mode_dark() },
    { id: 'system', label: () => m.theme_mode_system() },
  ];

  const themeOptions: {
    id: AppearancePackId;
    label: () => string;
    core: string;
    ring: string;
  }[] = [
    {
      id: 'inkstone-precision',
      label: () => m.settings_theme_inkstone(),
      core: '#f4f5f5',
      ring: '#2a6f7a',
    },
    {
      id: 'cold-cinnabar',
      label: () => m.settings_theme_cinnabar(),
      core: '#f2f2f0',
      ring: '#c45a3c',
    },
  ];

  const shortcuts: {
    href: '/' | '/sources' | '/library' | '/apps' | '/apps/novel';
    label: () => string;
    icon: typeof Database;
  }[] = [
    { href: '/sources', label: () => m.nav_sources(), icon: Radio },
    { href: '/library', label: () => m.nav_library(), icon: Database },
    { href: '/apps', label: () => m.nav_apps(), icon: Boxes },
    { href: '/apps/novel', label: () => m.nav_novel(), icon: BookOpen },
  ];

  const langOptions: { id: Locale; label: () => string }[] = [
    { id: 'zh-CN', label: () => m.settings_lang_zh() },
    { id: 'en', label: () => m.settings_lang_en() },
  ];
</script>

<section class="w-full max-w-2xl" data-testid="settings-home">
  <h1 class="mb-2 text-base font-semibold tracking-tight text-ink">{m.settings_title()}</h1>

  <div class="overflow-hidden rounded-lg border border-hairline bg-surface-1">
    <!-- 快捷入口：密排 icon+字 -->
    <div class="border-b border-hairline px-3 py-2">
      <p class="mb-1.5 text-[0.7rem] font-medium tracking-wide text-ink-muted">
        {m.settings_shortcuts()}
      </p>
      <div class="grid grid-cols-4 gap-1">
        {#each shortcuts as item (item.href)}
          {@const Icon = item.icon}
          <a
            href={resolve(item.href as '/')}
            class="motion-nav-capsule flex min-h-11 flex-col items-center justify-center gap-0.5 rounded-md px-1 text-ink-muted outline-none transition-colors hover:bg-lantern-soft hover:text-ink focus-visible:bg-lantern-soft focus-visible:text-ink"
          >
            <Icon size={16} strokeWidth={1.75} aria-hidden="true" />
            <span class="truncate text-[0.65rem] font-medium leading-none">{item.label()}</span>
          </a>
        {/each}
      </div>
    </div>

    <!-- 外观 -->
    <div class="border-b border-hairline px-3 py-1.5">
      <p class="text-[0.7rem] font-medium tracking-wide text-ink-muted">
        {m.settings_appearance()}
      </p>
    </div>

    <div class="flex items-center justify-between gap-3 border-b border-hairline px-3 py-2">
      <span class="shrink-0 text-[0.8125rem] font-medium text-ink">{m.settings_mode_label()}</span>
      <div
        class="inline-flex max-w-[min(100%,18rem)] flex-1 rounded-md border border-hairline bg-surface-3 p-px sm:flex-none"
        role="group"
        aria-label={m.settings_mode_label()}
      >
        {#each modeOptions as opt (opt.id)}
          <button
            type="button"
            class={[
              'motion-nav-capsule min-h-7 flex-1 rounded-[5px] px-2 text-[0.75rem] font-medium outline-none transition-colors focus-visible:shadow-[var(--focus-ring)]',
              mode === opt.id
                ? 'bg-surface-1 text-ink shadow-[var(--surface-panel-shadow)]'
                : 'text-ink-muted hover:text-ink',
            ]}
            aria-pressed={mode === opt.id}
            onclick={() => chooseMode(opt.id)}
          >
            {opt.label()}
          </button>
        {/each}
      </div>
    </div>

    <div class="flex items-center justify-between gap-3 border-b border-hairline px-3 py-2">
      <span class="shrink-0 text-[0.8125rem] font-medium text-ink">{m.settings_theme_label()}</span>
      <div
        class="flex flex-wrap justify-end gap-1.5"
        role="group"
        aria-label={m.settings_theme_label()}
      >
        {#each themeOptions as opt (opt.id)}
          {@const selected = themeId === opt.id}
          <button
            type="button"
            class={[
              'motion-nav-capsule inline-flex min-h-8 items-center gap-2 rounded-md border px-2 py-1 outline-none transition-colors focus-visible:shadow-[var(--focus-ring)]',
              selected
                ? 'border-lantern/35 bg-lantern-soft'
                : 'border-hairline bg-canvas hover:bg-surface-2',
            ]}
            aria-pressed={selected}
            aria-label={opt.label()}
            onclick={() => chooseTheme(opt.id)}
          >
            <span
              class={[
                'relative inline-flex size-6 shrink-0 items-center justify-center rounded-full',
                selected && 'ring-2 ring-lantern/30 ring-offset-1 ring-offset-surface-1',
              ]}
              style:background={opt.ring}
              aria-hidden="true"
            >
              <span class="size-2.5 rounded-full border border-black/5" style:background={opt.core}
              ></span>
            </span>
            <span class="text-[0.75rem] font-medium text-ink">{opt.label()}</span>
          </button>
        {/each}
      </div>
    </div>

    <!-- 语言 -->
    <div class="flex items-center justify-between gap-3 px-3 py-2">
      <span class="shrink-0 text-[0.8125rem] font-medium text-ink">{m.settings_language()}</span>
      <div
        class="inline-flex rounded-md border border-hairline bg-surface-3 p-px"
        role="group"
        aria-label={m.settings_language()}
      >
        {#each langOptions as opt (opt.id)}
          <button
            type="button"
            class={[
              'motion-nav-capsule min-h-7 rounded-[5px] px-2.5 text-[0.75rem] font-medium outline-none transition-colors focus-visible:shadow-[var(--focus-ring)]',
              locale === opt.id
                ? 'bg-surface-1 text-ink shadow-[var(--surface-panel-shadow)]'
                : 'text-ink-muted hover:text-ink',
            ]}
            aria-pressed={locale === opt.id}
            onclick={() => chooseLocale(opt.id)}
          >
            {opt.label()}
          </button>
        {/each}
      </div>
    </div>
  </div>
</section>
