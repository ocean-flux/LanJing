<script lang="ts">
  import { dev } from '$app/environment';
  import { page } from '$app/state';
  import ModeShell from '$lib/app/ModeShell.svelte';
  // 导入主题模块以触发初始化副作用（读取偏好、应用 DOM、注册系统主题监听）
  import '$lib/stores/theme.svelte';
  import '../index.css';

  let { children }: { children?: import('svelte').Snippet } = $props();
  const isThrowawayPrototype = $derived(dev && page.url.pathname === '/prototype/media-shell');
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
