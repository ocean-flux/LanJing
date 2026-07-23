<script lang="ts">
  import { browser } from '$app/environment';
  import ModeShell from '$lib/app/ModeShell.svelte';
  import { getLocale, getTextDirection } from '$lib/i18n';
  // 根布局只启动主题持久化并同步文档语言语义。
  import { startThemePreferences } from '$lib/stores/theme.svelte';
  import { onMount } from 'svelte';
  import '../index.css';

  let { children }: { children?: import('svelte').Snippet } = $props();

  onMount(() => {
    if (!browser) return;
    const locale = getLocale();
    document.documentElement.lang = locale;
    document.documentElement.dir = getTextDirection(locale);
    void startThemePreferences();
  });
</script>

<ModeShell>
  {#if children}
    {@render children()}
  {/if}
</ModeShell>
