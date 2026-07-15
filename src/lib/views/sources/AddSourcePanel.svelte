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

<section class="surface-panel p-5" aria-labelledby="add-source-title">
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div>
      <h2 id="add-source-title" class="text-2xl font-semibold">{m.sources_add_title()}</h2>
      <p class="mt-2 max-w-xl text-sm leading-6 text-muted-foreground">
        {m.sources_add_desc()}
      </p>
    </div>
    <Button type="button" class="rounded-full">
      {m.action_import_local()}
    </Button>
  </div>

  <div class="mt-5 grid gap-3 md:grid-cols-5" role="list" aria-label={m.sources_title()}>
    {#each entries as entry (entry.key)}
      <Button
        type="button"
        variant={selected.key === entry.key ? 'secondary' : 'outline'}
        class="motion-nav-capsule h-auto min-h-16 flex-col items-start justify-center rounded-2xl bg-background/60 p-3 text-left text-sm hover:bg-accent"
        aria-pressed={selected.key === entry.key}
        onclick={() => (selected = entry)}
      >
        <span class="block font-semibold">{entry.label}</span>
      </Button>
    {/each}
  </div>

  <div class="surface-control mt-5 p-4 text-sm" role="status">
    <span class="font-semibold">{m.sources_precheck({ label: selected.label })}</span>
    <p class="mt-1 text-muted-foreground">{selected.result}</p>
  </div>
</section>
