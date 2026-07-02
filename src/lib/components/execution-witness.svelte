<script lang="ts">
  import { onMount } from 'svelte';
  import { loadRules, getRules } from '$lib/stores/rules.svelte';
  import {
    startSearch,
    startDiscover,
    selectBook,
    selectChapter,
    goBack,
    cleanup,
    getBooks,
    getRawContent,
    getNodeOutputs,
    getLoading,
    getError,
    getCurrentSegment,
    getSelectedBook,
    getSelectedChapter,
    type BookMedia,
    type Chapter,
  } from '$lib/stores/execution.svelte';
  import { Button } from '$lib/components/ui/button';

  let selectedRuleId = $state('');
  let searchQuery = $state('');
  let nodePanelOpen = $state(true);

  onMount(() => {
    loadRules();
    return () => cleanup();
  });

  function handleSearch() {
    if (!selectedRuleId || !searchQuery.trim()) return;
    startSearch(selectedRuleId, searchQuery.trim());
  }

  function handleDiscover() {
    if (!selectedRuleId) return;
    startDiscover(selectedRuleId);
  }

  /** 从 Media/Book 递归找章节列表。 */
  function chaptersForBook(book: BookMedia): Chapter[] {
    return book.chapters ?? [];
  }
</script>

<div class="flex flex-col gap-4 h-full overflow-auto">
  <h2 class="text-lg font-semibold">执行 Witness</h2>

  <!-- 顶部：规则选择 + 搜索 -->
  <div class="flex flex-wrap items-end gap-2">
    <div class="flex flex-col gap-1 min-w-40 flex-1">
      <label for="rule-select" class="text-xs text-muted-foreground">选择规则</label>
      <select
        id="rule-select"
        bind:value={selectedRuleId}
        class="h-9 rounded-md border border-input bg-background px-3 text-sm"
      >
        <option value="">-- 请选择规则 --</option>
        {#each getRules() as rule (rule.id)}
          <option value={rule.id}>
            {rule.source_url || `规则 ${rule.id.slice(0, 8)}`}（{rule.node_count} 节点）
          </option>
        {/each}
      </select>
    </div>

    <input
      type="text"
      placeholder="搜索关键词"
      bind:value={searchQuery}
      disabled={getLoading() || !selectedRuleId}
      class="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
    />

    <Button
      onclick={handleSearch}
      disabled={getLoading() || !selectedRuleId || !searchQuery.trim()}
    >
      搜索
    </Button>

    <Button onclick={handleDiscover} disabled={getLoading() || !selectedRuleId} variant="outline">
      发现 / 分类浏览
    </Button>
  </div>

  <!-- 返回按钮 -->
  {#if getCurrentSegment() === 'detail_toc' || getCurrentSegment() === 'content'}
    <div>
      <Button onclick={goBack} variant="ghost" size="sm">← 返回</Button>
    </div>
  {/if}

  <!-- 错误提示 -->
  {#if getError()}
    <div
      class="rounded-md border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
    >
      {getError()}
    </div>
  {/if}

  <!-- Loading -->
  {#if getLoading()}
    <div class="flex items-center justify-center py-12 text-muted-foreground">
      <span class="animate-pulse">执行中…</span>
    </div>
  {/if}

  <!-- 段 1：搜索结果 / BookMedia cards -->
  {#if getCurrentSegment() === 'search' && !getLoading()}
    {#if getBooks().length === 0}
      <div class="flex flex-col items-center justify-center py-12 text-muted-foreground gap-2">
        <p>暂无结果</p>
        {#if searchQuery}
          <p class="text-xs">尝试换一个关键词</p>
        {/if}
      </div>
    {:else}
      <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-3">
        {#each getBooks() as book (book.book_url ?? book.title)}
          <button
            type="button"
            class="flex flex-col gap-2 rounded-lg border bg-card p-3 text-left transition-colors hover:bg-accent hover:border-accent-foreground/30 cursor-pointer"
            onclick={() => selectBook(book, selectedRuleId)}
          >
            {#if book.cover_url}
              <div class="aspect-[3/4] bg-muted rounded overflow-hidden">
                <img
                  src={book.cover_url}
                  alt={book.title}
                  class="w-full h-full object-cover"
                  loading="lazy"
                />
              </div>
            {/if}
            <div class="space-y-1">
              <p class="font-semibold text-sm leading-tight">{book.title}</p>
              {#if book.author}
                <p class="text-xs text-muted-foreground">{book.author}</p>
              {/if}
              {#if book.kind}
                <span
                  class="inline-block text-[0.7rem] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary"
                  >{book.kind}</span
                >
              {/if}
              {#if book.description}
                <p class="text-xs text-muted-foreground line-clamp-2">{book.description}</p>
              {/if}
            </div>
          </button>
        {/each}
      </div>
    {/if}
  {/if}

  <!-- 段 2：detail_toc — 章节列表 + 选中书籍信息 -->
  {#if getCurrentSegment() === 'detail_toc'}
    {#if getSelectedBook()}
      <div class="rounded-lg border bg-card p-3 space-y-2">
        <h3 class="font-semibold">{getSelectedBook()!.title}</h3>
        {#if getSelectedBook()!.description}
          <p class="text-sm text-muted-foreground">{getSelectedBook()!.description}</p>
        {/if}
      </div>
    {/if}

    {#if getLoading()}
      <div class="flex items-center justify-center py-8 text-muted-foreground">
        <span class="animate-pulse">加载章节列表…</span>
      </div>
    {:else}
      {@const chapters = getSelectedBook() ? chaptersForBook(getSelectedBook()!) : []}
      {#if chapters.length === 0}
        <div class="flex flex-col items-center justify-center py-12 text-muted-foreground gap-2">
          <p>暂无章节</p>
        </div>
      {:else}
        <div class="space-y-1">
          {#each chapters as ch (ch.chapter_url)}
            <button
              type="button"
              class="w-full text-left px-3 py-2 rounded-md text-sm transition-colors hover:bg-accent cursor-pointer"
              onclick={() => selectChapter(ch, selectedRuleId)}
            >
              {ch.title}
            </button>
          {/each}
        </div>
      {/if}
    {/if}
  {/if}

  <!-- 段 3：content — 正文渲染 -->
  {#if getCurrentSegment() === 'content'}
    {@const chapter = getSelectedChapter()}
    {#if chapter}
      <div class="rounded-lg border bg-card p-3">
        <h3 class="font-semibold">{chapter.title}</h3>
      </div>
    {/if}

    {#if getLoading()}
      <div class="flex items-center justify-center py-8 text-muted-foreground">
        <span class="animate-pulse">加载正文…</span>
      </div>
    {:else}
      {@const content = getRawContent()}
      {#if content}
        <div class="prose prose-sm dark:prose-invert max-w-none whitespace-pre-wrap">
          {content}
        </div>
      {:else}
        <div class="flex flex-col items-center justify-center py-12 text-muted-foreground">
          <p>正文为空</p>
        </div>
      {/if}
    {/if}
  {/if}

  <!-- Node-output 调试面板 -->
  {#if getNodeOutputs().length > 0}
    <div class="rounded-md border bg-card">
      <button
        type="button"
        class="flex items-center justify-between w-full px-3 py-2 text-sm font-semibold cursor-pointer hover:bg-accent"
        onclick={() => (nodePanelOpen = !nodePanelOpen)}
      >
        <span>节点输出调试（{getNodeOutputs().length}）</span>
        <span class="text-muted-foreground">{nodePanelOpen ? '▼' : '▶'}</span>
      </button>
      {#if nodePanelOpen}
        <div class="max-h-60 overflow-y-auto border-t p-2 space-y-1">
          {#each getNodeOutputs() as no, i (no.node_id)}
            <div
              class="text-xs font-mono px-2 py-1 rounded {i === getNodeOutputs().length - 1
                ? 'bg-primary/10 border border-primary/30'
                : 'bg-muted/30'}"
            >
              <span class="font-semibold text-primary">#{i}</span>
              <span class="text-muted-foreground">ID:</span>
              {no.node_id.slice(0, 8)}…
              <span class="text-muted-foreground">类型:</span>
              {no.variant}
              <span class="text-muted-foreground">摘要:</span>
              {no.summary}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>
