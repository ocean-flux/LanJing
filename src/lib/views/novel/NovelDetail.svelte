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

<section class="mx-auto flex max-w-[var(--content-detail-max-width)] flex-col gap-5 py-4">
  <div class="surface-panel relative overflow-hidden p-6 md:p-8">
    <div class="relative grid gap-6 md:grid-cols-[220px_1fr]">
      <div
        class="media-void grid aspect-[3/4] place-items-center rounded-[1.75rem] text-muted-foreground shadow-surface-panel"
        aria-label={m.novel_detail_title()}
      >
        <BookOpen size={48} aria-hidden="true" />
      </div>
      <div class="self-center">
        <p class="text-sm font-semibold text-muted-foreground">{m.novel_title()}</p>
        <h1 class="mt-3 max-w-3xl text-4xl font-semibold tracking-tight text-balance md:text-6xl">
          {m.novel_detail_title()}
        </h1>
        <p class="mt-3 text-sm text-muted-foreground">{m.novel_detail_meta()}</p>
        <p class="mt-5 max-w-2xl text-sm leading-6 text-muted-foreground">
          {m.novel_detail_desc()}
        </p>
        <div class="mt-7 flex flex-wrap gap-3">
          <a
            href={resolve('/apps/novel/read' as '/')}
            class="motion-nav-capsule inline-flex min-h-11 items-center gap-2 rounded-full bg-primary px-5 text-sm font-semibold text-primary-foreground hover:bg-primary/90"
          >
            <Play size={16} aria-hidden="true" />
            {m.action_open_reader()}
          </a>
          <a
            href="#novel-directory"
            class="motion-nav-capsule inline-flex min-h-11 items-center gap-2 rounded-full border border-border/70 bg-background/70 px-5 text-sm font-semibold hover:bg-accent"
          >
            <ListTree size={16} aria-hidden="true" />
            {m.novel_toc_title()}
          </a>
        </div>
      </div>
    </div>
  </div>

  <div class="grid gap-4 md:grid-cols-2">
    <section id="novel-directory" class="surface-panel p-5" aria-labelledby="novel-toc-title">
      <ListTree class="mb-4" size={20} aria-hidden="true" />
      <h2 id="novel-toc-title" class="text-xl font-semibold">{m.novel_toc_title()}</h2>
      <p class="mt-3 text-sm leading-6 text-muted-foreground">{m.novel_toc_desc()}</p>
      <ol class="mt-5 grid gap-1.5">
        {#each chapters as chapter (chapter.index)}
          <li>
            <a
              href={resolve('/apps/novel/read' as '/')}
              class="motion-nav-capsule flex min-h-10 items-center justify-between rounded-md px-3 text-sm text-ink-muted hover:bg-lantern-soft hover:text-ink"
            >
              <span>{m.novel_chapter({ index: chapter.index, title: chapter.title })}</span>
              <span aria-hidden="true">→</span>
            </a>
          </li>
        {/each}
      </ol>
    </section>
    <section class="surface-panel p-5" aria-labelledby="novel-reader-entry-title">
      <Play class="mb-4" size={20} aria-hidden="true" />
      <h2 id="novel-reader-entry-title" class="text-xl font-semibold">
        {m.novel_read_entry_title()}
      </h2>
      <p class="mt-3 text-sm leading-6 text-muted-foreground">{m.novel_read_entry_desc()}</p>
      <a
        href={resolve('/apps/novel/read' as '/')}
        class="mt-4 inline-flex min-h-11 items-center rounded-full bg-primary px-5 text-sm font-semibold text-primary-foreground hover:bg-primary/90"
      >
        {m.action_open_reader()}
      </a>
    </section>
  </div>
</section>
