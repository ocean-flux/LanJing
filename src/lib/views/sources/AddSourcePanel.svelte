<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { m } from '$lib/i18n';

  type Entry = {
    key: 'url' | 'subscription' | 'package' | 'directory' | 'file';
    label: string;
    description: string;
    result: string;
  };

  type Props = {
    onimportlocal?: () => void;
  };

  let { onimportlocal }: Props = $props();

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

  function handleRadioKeydown(event: KeyboardEvent & { currentTarget: HTMLElement }): void {
    const radios = Array.from(
      event.currentTarget
        .closest('[role="radiogroup"]')
        ?.querySelectorAll<HTMLElement>('[role="radio"]') ?? [],
    );
    const currentIndex = radios.indexOf(event.currentTarget);
    if (currentIndex < 0) return;

    let nextIndex: number | null = null;
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
      nextIndex = (currentIndex + 1) % radios.length;
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
      nextIndex = (currentIndex - 1 + radios.length) % radios.length;
    } else if (event.key === 'Home') {
      nextIndex = 0;
    } else if (event.key === 'End') {
      nextIndex = radios.length - 1;
    }

    if (nextIndex === null) return;
    event.preventDefault();
    radios[nextIndex]?.focus();
    radios[nextIndex]?.click();
  }
</script>

<!-- Ethereal 装源入口壳：double-bezel + lantern 选择态；不另造 prepare/install FSM -->
<section
  class="double-bezel p-3 sm:p-3.5"
  aria-labelledby="add-source-title"
  data-testid="add-source-panel"
>
  <div class="flex flex-wrap items-start justify-between gap-2">
    <div class="min-w-0">
      <h2 id="add-source-title" class="text-sm font-semibold tracking-tight text-ink">
        {m.sources_add_title()}
      </h2>
      <p class="mt-1 max-w-prose text-xs leading-5 text-ink-muted">
        {m.sources_add_desc()}
      </p>
    </div>
    {#if onimportlocal}
      <Button type="button" size="sm" class="min-h-11 rounded-lg px-3" onclick={onimportlocal}>
        {m.action_import_local()}
      </Button>
    {/if}
  </div>

  <div
    class="mt-3 grid gap-1.5 sm:grid-cols-2 md:grid-cols-5"
    role="radiogroup"
    aria-label={m.sources_entry_type_label()}
  >
    {#each entries as entry (entry.key)}
      {@const active = selected.key === entry.key}
      <Button
        type="button"
        role="radio"
        variant={active ? 'secondary' : 'outline'}
        class={[
          'motion-nav-capsule h-auto min-h-11 flex-col items-start justify-center rounded-lg px-2.5 py-2 text-left text-xs transition-colors',
          active
            ? 'border-lantern/45 bg-lantern-soft text-ink shadow-[0_0_0_1px_color-mix(in_oklab,var(--lantern)_22%,transparent)] hover:bg-lantern-soft'
            : 'hover:bg-surface-3',
        ]}
        aria-checked={active}
        tabindex={active ? 0 : -1}
        onclick={() => (selected = entry)}
        onkeydown={handleRadioKeydown}
      >
        <span class="block font-medium">{entry.label}</span>
        <span class="mt-0.5 line-clamp-2 text-[0.68rem] font-normal leading-4 text-ink-subtle">
          {entry.description}
        </span>
      </Button>
    {/each}
  </div>

  <div
    class="mt-3 rounded-lg border border-hairline bg-surface-2 px-3 py-2.5 text-xs shadow-[inset_0_1px_0_color-mix(in_oklab,white_4%,transparent)]"
    role="status"
    data-testid="add-source-precheck"
  >
    <div class="flex items-center gap-2">
      <span class="inline-flex h-1.5 w-1.5 shrink-0 rounded-full bg-lantern" aria-hidden="true"
      ></span>
      <span class="font-medium text-ink">{m.sources_precheck({ label: selected.label })}</span>
    </div>
    <p class="mt-1 pl-3.5 text-ink-muted">{selected.result}</p>
  </div>
</section>
