<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { m } from '$lib/i18n';

  type Entry = {
    key: 'url' | 'subscription' | 'package' | 'directory' | 'file';
    label: string;
    description: string;
    result: string;
  };

  const entries: Entry[] = [
    {
      key: 'url',
      label: m.sources_type_url(),
      description: m.sources_type_url_desc(),
      result: m.sources_type_url_result(),
    },
    {
      key: 'subscription',
      label: m.sources_type_subscription(),
      description: m.sources_type_subscription_desc(),
      result: m.sources_type_subscription_result(),
    },
    {
      key: 'package',
      label: m.sources_type_package(),
      description: m.sources_type_package_desc(),
      result: m.sources_type_package_result(),
    },
    {
      key: 'directory',
      label: m.sources_type_directory(),
      description: m.sources_type_directory_desc(),
      result: m.sources_type_directory_result(),
    },
    {
      key: 'file',
      label: m.sources_type_file(),
      description: m.sources_type_file_desc(),
      result: m.sources_type_file_result(),
    },
  ];

  let selected = $state<Entry>(entries[0]);
</script>

<section class="border border-hairline bg-surface-1 p-3" aria-labelledby="add-source-title">
  <div class="flex flex-wrap items-start justify-between gap-2">
    <div class="min-w-0">
      <h2 id="add-source-title" class="text-sm font-semibold text-ink">{m.sources_add_title()}</h2>
      <p class="mt-1 max-w-prose text-xs leading-5 text-ink-muted">
        {m.sources_add_desc()}
      </p>
    </div>
    <Button type="button" size="sm" class="rounded-lg">
      {m.action_import_local()}
    </Button>
  </div>

  <div
    class="mt-3 grid gap-1.5 sm:grid-cols-2 md:grid-cols-5"
    role="list"
    aria-label={m.sources_title()}
  >
    {#each entries as entry (entry.key)}
      <Button
        type="button"
        variant={selected.key === entry.key ? 'secondary' : 'outline'}
        class="motion-nav-capsule h-auto min-h-11 flex-col items-start justify-center rounded-lg px-2 py-2 text-left text-xs hover:bg-surface-3"
        aria-pressed={selected.key === entry.key}
        onclick={() => (selected = entry)}
      >
        <span class="block font-medium">{entry.label}</span>
      </Button>
    {/each}
  </div>

  <div class="mt-3 border border-hairline bg-surface-2 px-3 py-2 text-xs" role="status">
    <span class="font-medium text-ink">{m.sources_precheck({ label: selected.label })}</span>
    <p class="mt-0.5 text-ink-muted">{selected.result}</p>
  </div>
</section>
