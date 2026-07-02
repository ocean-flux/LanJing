<script lang="ts">
  import AppShell from '$lib/app/AppShell.svelte';
  // 导入主题模块以触发初始化副作用（读取偏好、应用 DOM、注册系统主题监听）
  import '$lib/stores/theme.svelte';
  import '../index.css';

  let { children }: { children?: import('svelte').Snippet } = $props();
</script>

<AppShell>
  {#if children}
    {@render children()}
  {/if}
</AppShell>
