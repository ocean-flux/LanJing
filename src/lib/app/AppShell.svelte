<script lang="ts">
  import { Toaster } from '$lib/components/ui/sonner';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { Minus, Square, X } from '@lucide/svelte';

  let { children }: { children?: import('svelte').Snippet } = $props();

  // 桌面端才显示自定义标题栏；web 端隐藏
  const isDesktop = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

  async function minimize() {
    await getCurrentWindow().minimize();
  }

  async function toggleMaximize() {
    await getCurrentWindow().toggleMaximize();
  }

  async function closeWindow() {
    await getCurrentWindow().close();
  }
</script>

<div class="grid grid-rows-[auto_auto_1fr_auto] h-screen min-w-0">
  {#if isDesktop}
    <div
      class="flex items-center justify-between h-8 pl-3 pr-1 select-none border-b bg-card"
      data-tauri-drag-region
    >
      <span class="text-[0.78rem] font-semibold text-muted-foreground" data-tauri-drag-region
        >LanJing</span
      >
      <div class="inline-flex">
        <button
          type="button"
          class="inline-flex items-center justify-center w-10 h-8 bg-transparent border-none text-muted-foreground cursor-pointer transition-colors hover:bg-accent hover:text-accent-foreground"
          aria-label="最小化"
          onclick={minimize}
        >
          <Minus size={14} />
        </button>
        <button
          type="button"
          class="inline-flex items-center justify-center w-10 h-8 bg-transparent border-none text-muted-foreground cursor-pointer transition-colors hover:bg-accent hover:text-accent-foreground"
          aria-label="最大化"
          onclick={toggleMaximize}
        >
          <Square size={12} />
        </button>
        <button
          type="button"
          class="inline-flex items-center justify-center w-10 h-8 bg-transparent border-none text-muted-foreground cursor-pointer transition-colors hover:bg-red-500 hover:text-white"
          aria-label="关闭"
          onclick={closeWindow}
        >
          <X size={14} />
        </button>
      </div>
    </div>
  {/if}
  <nav class="flex items-center gap-4 justify-between py-4 px-[clamp(1rem,2vw,2rem)]">
    <div
      class="inline-flex items-center gap-2 min-h-10 py-1.5 pl-1.5 pr-[0.9rem] rounded-full border bg-card"
    >
      <span
        class="inline-flex items-center justify-center w-8 h-8 rounded-full bg-foreground text-background font-bold"
        >L</span
      >
      <span class="text-sm font-semibold whitespace-nowrap">LanJing</span>
    </div>
    <div class="flex items-center gap-2">
      <span
        class="inline-flex items-center gap-2 min-h-10 px-3.5 text-sm font-semibold whitespace-nowrap rounded-full border bg-card bg-primary/10 border-primary/24"
        >Welcome</span
      >
    </div>
  </nav>
  <main class="min-h-0 overflow-auto">
    {#if children}
      {@render children()}
    {/if}
  </main>
  <footer
    class="flex items-center justify-between py-2 px-[clamp(1rem,2vw,2rem)] text-xs text-muted-foreground border-t"
  >
    <span>LanJing v0.1.0</span>
  </footer>
</div>

<Toaster />
