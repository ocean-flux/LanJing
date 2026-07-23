<script lang="ts">
  import { resolve } from '$app/paths';
  import Icon from '$lib/components/Icon.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { m } from '$lib/i18n';
</script>

<section class="mx-auto flex w-full max-w-6xl flex-col gap-6" aria-labelledby="realm-title">
  <PageHeader title={m.realm_title()} />

  <div
    class="glass-panel flex min-h-72 flex-col items-start justify-center rounded-xl border border-hairline px-6 py-8 sm:px-8"
    role="status"
  >
    <Icon name="compass" class="size-7 text-lantern-strong" />
    <h2 id="realm-title" class="mt-4 text-lg font-semibold text-ink">{m.realm_empty_title()}</h2>
    <a
      href={resolve('/sources' as '/')}
      class="glass-control mt-6 inline-flex min-h-11 items-center gap-2 rounded-lg border border-hairline-strong px-4 text-sm font-semibold text-ink outline-none hover:bg-surface-2 focus-visible:shadow-[var(--focus-ring)]"
    >
      <span>{m.action_manage_sources()}</span>
      <Icon name="arrow-right" class="size-4" />
    </a>
  </div>
</section>
