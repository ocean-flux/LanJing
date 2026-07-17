<script lang="ts">
  import { browser, dev } from '$app/environment';
  import { page } from '$app/state';
  import ModeShell from '$lib/app/ModeShell.svelte';
  import { getLocale, getTextDirection } from '$lib/i18n';
  // 主题模块：DOM 初值 + 系统监听；持久化在 onMount 启动 RuneStore
  import { startThemePreferences } from '$lib/stores/theme.svelte';
  import { onMount } from 'svelte';
  import '../index.css';

  let { children }: { children?: import('svelte').Snippet } = $props();
  const isThrowawayPrototype = $derived(dev && page.url.pathname === '/prototype/media-shell');

  onMount(() => {
    if (!browser) return;
    // SPA 模板固定为基准语言；挂载后按 Paraglide 当前 locale 校准辅助技术语义。
    const locale = getLocale();
    document.documentElement.lang = locale;
    document.documentElement.dir = getTextDirection(locale);
    void startThemePreferences();
  });
</script>

{#if isThrowawayPrototype}
  {#if children}
    {@render children()}
  {/if}
{:else}
  <ModeShell>
    {#if children}
      {@render children()}
    {/if}
  </ModeShell>
{/if}
