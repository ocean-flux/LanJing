<script lang="ts">
  import { resolve } from '$app/paths';
  import BookOpen from '@lucide/svelte/icons/book-open';
  import ListTree from '@lucide/svelte/icons/list-tree';
  import Play from '@lucide/svelte/icons/play';
  import { m } from '$lib/i18n';

  const chapters = [
    { index: 1, title: m.novel_chapter_1() },
    { index: 2, title: m.novel_chapter_2() },
    { index: 3, title: m.novel_chapter_3() },
    { index: 4, title: m.novel_chapter_4() },
    { index: 5, title: m.novel_chapter_5() },
  ];
</script>

<section class="flex w-full flex-col gap-4">
  <!-- 紧凑详情头：无 6xl hero / 大 surface-panel -->
  <header class="grid gap-4 border-b border-hairline pb-4 sm:grid-cols-[120px_1fr] sm:gap-5">
    <div
      class="media-void grid aspect-[3/4] max-h-40 place-items-center rounded-xl text-ink-muted sm:max-h-none"
      aria-label={m.novel_detail_title()}
    >
      <BookOpen size={32} aria-hidden="true" />
    </div>
    <div class="min-w-0 self-center">
      <p class="text-xs font-medium text-ink-muted">{m.novel_title()}</p>
      <h1 class="mt-1 text-xl font-semibold tracking-tight text-ink sm:text-2xl">
        {m.novel_detail_title()}
      </h1>
      <p class="mt-1 text-xs text-ink-subtle">{m.novel_detail_meta()}</p>
      <p class="mt-2 max-w-prose text-sm leading-6 text-ink-muted">{m.novel_detail_desc()}</p>
      <div class="mt-3 flex flex-wrap gap-2">
        <a
          href={resolve('/apps/novel/read' as '/')}
          class="motion-nav-capsule inline-flex min-h-10 items-center gap-2 rounded-lg bg-primary px-4 text-sm font-semibold text-primary-foreground hover:bg-primary/90"
        >
          <Play size={15} aria-hidden="true" />
          {m.action_open_reader()}
        </a>
        <a
          href="#novel-directory"
          class="motion-nav-capsule inline-flex min-h-10 items-center gap-2 rounded-lg border border-hairline px-4 text-sm font-medium text-ink hover:bg-surface-3"
        >
          <ListTree size={15} aria-hidden="true" />
          {m.novel_toc_title()}
        </a>
      </div>
    </div>
  </header>

  <div class="grid gap-4 md:grid-cols-2">
    <section id="novel-directory" aria-labelledby="novel-toc-title">
      <h2 id="novel-toc-title" class="flex items-center gap-2 text-sm font-semibold text-ink">
        <ListTree size={16} aria-hidden="true" />
        {m.novel_toc_title()}
      </h2>
      <p class="mt-1 text-xs leading-5 text-ink-muted">{m.novel_toc_desc()}</p>
      <ol class="mt-2 grid gap-0.5 border-t border-hairline">
        {#each chapters as chapter (chapter.index)}
          <li>
            <a
              href={resolve('/apps/novel/read' as '/')}
              class="motion-nav-capsule flex min-h-10 items-center justify-between border-b border-hairline px-1 text-sm text-ink-muted hover:bg-lantern-soft hover:text-ink"
            >
              <span>{m.novel_chapter({ index: chapter.index, title: chapter.title })}</span>
              <span aria-hidden="true">→</span>
            </a>
          </li>
        {/each}
      </ol>
    </section>
    <section aria-labelledby="novel-reader-entry-title">
      <h2
        id="novel-reader-entry-title"
        class="flex items-center gap-2 text-sm font-semibold text-ink"
      >
        <Play size={16} aria-hidden="true" />
        {m.novel_read_entry_title()}
      </h2>
      <p class="mt-1 text-xs leading-5 text-ink-muted">{m.novel_read_entry_desc()}</p>
      <a
        href={resolve('/apps/novel/read' as '/')}
        class="mt-3 inline-flex min-h-10 items-center rounded-lg bg-primary px-4 text-sm font-semibold text-primary-foreground hover:bg-primary/90"
      >
        {m.action_open_reader()}
      </a>
    </section>
  </div>
</section>
