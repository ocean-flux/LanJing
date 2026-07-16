<script lang="ts">
  import { resolve } from '$app/paths';
  import { BookOpen, FilePlus, Search } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';
  import { m } from '$lib/i18n';
</script>

<section class="mx-auto flex max-w-[var(--content-max-width)] flex-col gap-5 py-4">
  <div class="surface-panel p-5 md:p-6">
    <p class="text-sm font-medium text-muted-foreground">{m.media_label_novel()}</p>
    <h1 class="mt-2 text-2xl font-semibold tracking-tight md:text-3xl">{m.novel_title()}</h1>
    <p class="mt-2 max-w-2xl text-sm leading-6 text-muted-foreground">{m.novel_home_desc()}</p>
  </div>

  <div class="grid gap-3 md:grid-cols-3">
    <Button
      type="button"
      variant="outline"
      class="motion-dock-wake h-auto flex-col items-start justify-start rounded-[1.5rem] bg-surface-1 p-5 text-left hover:bg-accent"
    >
      <BookOpen class="mb-4" size={20} aria-hidden="true" />
      <span class="block font-semibold">{m.novel_add_source()}</span>
      <span class="mt-2 block text-sm font-normal text-muted-foreground">
        {m.novel_add_source_desc()}
      </span>
    </Button>
    <Button
      type="button"
      variant="outline"
      class="motion-dock-wake h-auto flex-col items-start justify-start rounded-[1.5rem] bg-surface-1 p-5 text-left hover:bg-accent"
    >
      <FilePlus class="mb-4" size={20} aria-hidden="true" />
      <span class="block font-semibold">{m.novel_import_book()}</span>
      <span class="mt-2 block text-sm font-normal text-muted-foreground">
        {m.novel_import_book_desc()}
      </span>
    </Button>
    <Button
      type="button"
      variant="outline"
      class="motion-dock-wake h-auto flex-col items-start justify-start rounded-[1.5rem] bg-surface-1 p-5 text-left hover:bg-accent"
    >
      <Search class="mb-4" size={20} aria-hidden="true" />
      <span class="block font-semibold">{m.novel_search_sources()}</span>
      <span class="mt-2 block text-sm font-normal text-muted-foreground">
        {m.novel_search_sources_desc()}
      </span>
    </Button>
  </div>

  <a
    href={resolve('/apps/novel/item' as '/')}
    class="inline-flex min-h-11 w-fit items-center rounded-full bg-primary px-5 text-sm font-semibold text-primary-foreground hover:bg-primary/90"
  >
    {m.action_view_novel_detail()}
  </a>
</section>
