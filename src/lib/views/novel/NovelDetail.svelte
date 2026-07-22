<script lang="ts">
  import { resolve } from '$app/paths';
  import BookOpen from '@lucide/svelte/icons/book-open';
  import { m } from '$lib/i18n';
</script>

<section class="flex w-full flex-col gap-3" aria-labelledby="novel-detail-title">
  <header class="border-b border-hairline pb-2">
    <p class="text-xs font-medium text-ink-muted">{m.novel_title()}</p>
    <h1 id="novel-detail-title" class="mt-0.5 text-base font-semibold tracking-tight text-ink">
      {m.novel_detail_placeholder_title()}
    </h1>
  </header>

  <div
    class="flex flex-col gap-3 border border-hairline bg-surface-2 px-4 py-5 sm:flex-row sm:items-start sm:gap-4"
    role="status"
  >
    <div
      class="media-void grid aspect-[3/4] max-h-36 w-24 shrink-0 place-items-center rounded-xl text-ink-muted"
      aria-hidden="true"
    >
      <BookOpen size={28} strokeWidth={1.5} />
    </div>
    <div class="min-w-0">
      <p class="text-sm font-semibold text-ink">{m.novel_placeholder_title()}</p>
      <p class="mt-1 max-w-prose text-sm leading-6 text-ink-muted">
        {m.novel_detail_placeholder_desc()}
      </p>
      <div class="mt-3 flex flex-wrap gap-2">
        <a
          href={resolve('/apps/novel' as '/')}
          class="motion-nav-capsule inline-flex min-h-11 items-center rounded-lg border border-hairline bg-surface-1 px-4 text-sm font-medium text-ink outline-none hover:bg-lantern-soft focus-visible:shadow-[var(--focus-ring)]"
        >
          {m.reader_back()}
        </a>
        <a
          href={resolve('/sources' as '/')}
          class="motion-nav-capsule inline-flex min-h-11 items-center rounded-lg px-4 text-sm font-medium text-ink-muted outline-none hover:bg-lantern-soft hover:text-ink focus-visible:shadow-[var(--focus-ring)]"
        >
          {m.action_add_source()}
        </a>
      </div>
    </div>
  </div>
</section>
