<script lang="ts">
  import { resolve } from '$app/paths';
  import { getTextReaderTheme } from '$lib/stores/theme.svelte';
  import { m } from '$lib/i18n';

  const theme = getTextReaderTheme();
  let pageMode = $state(theme.pageMode);

  const widthClass =
    theme.contentWidth === 'narrow'
      ? 'max-w-[620px]'
      : theme.contentWidth === 'wide'
        ? 'max-w-[var(--content-reading-wide-width)]'
        : 'max-w-[var(--content-reading-max-width)]';
  const colorClass =
    theme.colorScheme === 'dark' || theme.colorScheme === 'black'
      ? 'bg-[var(--reader-canvas)] text-[var(--reader-ink)]'
      : theme.colorScheme === 'white'
        ? 'bg-white text-neutral-950'
        : theme.colorScheme === 'gray'
          ? 'bg-neutral-100 text-neutral-950'
          : 'bg-(--surface-reader) text-[#241b12]';
  const fontClass = theme.fontFamily === 'serif' ? 'font-serif' : 'font-sans';
  const readerColumns = $derived(
    pageMode === 'paged'
      ? 'lg:columns-2 lg:gap-12 lg:[column-rule:1px_solid_color-mix(in_oklab,currentColor_12%,transparent)]'
      : '',
  );
</script>

<article
  class={[
    'min-h-screen px-4 pb-[calc(6rem+env(safe-area-inset-bottom,0px))] pt-4 md:px-8 md:py-8',
    colorClass,
  ]}
  aria-labelledby="reader-title"
>
  <div class={['mx-auto', widthClass, fontClass]}>
    <header
      class="sticky top-3 z-10 mb-8 rounded-full border border-current/10 bg-inherit px-3 py-2 shadow-sm backdrop-blur-none"
    >
      <nav class="flex flex-wrap items-center justify-between gap-2" aria-label={m.reader_title()}>
        <a
          href={resolve('/apps/novel' as '/')}
          class="inline-flex min-h-10 items-center rounded-full px-4 text-sm font-semibold hover:bg-current/5"
        >
          {m.reader_back()}
        </a>
        <p class="px-2 text-xs font-semibold text-current/65" aria-live="polite">
          {m.reader_progress({ current: '1', total: pageMode === 'paged' ? '1' : '2' })}
        </p>
        <div class="flex gap-2">
          <button
            type="button"
            class="min-h-10 rounded-full px-4 text-sm font-semibold hover:bg-current/5"
            aria-label={m.reader_previous()}
          >
            {m.reader_previous()}
          </button>
          <button
            type="button"
            class="min-h-10 rounded-full px-4 text-sm font-semibold hover:bg-current/5"
            aria-label={m.reader_next()}
          >
            {m.reader_next()}
          </button>
          <button
            type="button"
            class="min-h-10 rounded-full px-4 text-sm font-semibold hover:bg-current/5"
            aria-pressed={pageMode === 'paged'}
            onclick={() => (pageMode = pageMode === 'paged' ? 'scroll' : 'paged')}
          >
            {pageMode === 'paged' ? m.reader_single_page() : m.reader_two_page()}
          </button>
        </div>
      </nav>
    </header>

    <div class="reader-frame surface-reader motion-page-turn px-5 py-7 md:px-10 md:py-10">
      <p class="text-xs font-semibold uppercase tracking-[0.24em] opacity-60">{m.novel_title()}</p>
      <h1 id="reader-title" class="mt-2 text-3xl font-semibold">{m.reader_title()}</h1>
      <p class="mt-2 text-sm opacity-70">
        {m.reader_theme_meta({
          scheme: theme.colorScheme,
          font: theme.fontFamily,
          size: theme.fontSize,
        })}
      </p>

      <div
        class={['mt-8 motion-page-turn', readerColumns]}
        style:font-size={`${theme.fontSize}px`}
        style:line-height={theme.lineHeight}
      >
        <p
          style:margin-bottom={theme.paragraphSpacing}
          style:text-indent={theme.indentFirstLine ? '2em' : '0'}
        >
          {m.reader_para_one()}
        </p>
        <p
          style:margin-bottom={theme.paragraphSpacing}
          style:text-indent={theme.indentFirstLine ? '2em' : '0'}
        >
          {m.reader_para_two()}
        </p>
      </div>
    </div>
  </div>

  <div
    class="fixed inset-x-0 bottom-0 z-20 border-t border-current/10 bg-inherit px-4 py-3 pb-[calc(0.75rem+env(safe-area-inset-bottom,0px))] md:hidden"
  >
    <div class="mx-auto flex max-w-[var(--content-reading-max-width)] justify-between gap-2">
      <button
        type="button"
        class="min-h-11 rounded-full px-4 text-sm font-semibold hover:bg-current/5"
      >
        {m.reader_previous()}
      </button>
      <button
        type="button"
        class="min-h-11 rounded-full px-4 text-sm font-semibold hover:bg-current/5"
      >
        {m.reader_next()}
      </button>
    </div>
  </div>
</article>
