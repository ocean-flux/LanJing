<script lang="ts">
  import { resolve } from '$app/paths';
  import Icon from '$lib/components/Icon.svelte';
  import { Button } from '$lib/components/ui/button';
  import { m } from '$lib/i18n';
  import type { InstalledSource, StandardIntent } from '$lib/stores/rules.svelte';

  type Props = {
    source: InstalledSource;
    highlighted?: boolean;
  };

  let { source, highlighted = false }: Props = $props();

  const intentLabels: Record<StandardIntent, () => string> = {
    Search: () => m.sources_intent_search(),
    Discover: () => m.sources_intent_discover(),
    ResolveItem: () => m.sources_intent_resolve_item(),
    ListUnits: () => m.sources_intent_list_units(),
    ResolveAsset: () => m.sources_intent_resolve_asset(),
    ContinueAction: () => m.sources_intent_continue_action(),
  };

  const groupLabel = $derived(
    source.profile.group && source.profile.group.trim().length > 0
      ? source.profile.group.trim()
      : m.sources_group_ungrouped(),
  );

  const workspaceBase = resolve('/sources/rules');

  const workspaceHref = $derived(
    source.document_ref
      ? `${workspaceBase}?document_id=${encodeURIComponent(source.document_ref.document_id)}`
      : `${workspaceBase}?legacy_source_id=${encodeURIComponent(source.source_id)}`,
  );
</script>

<article
  class={[
    'grid gap-4 px-4 py-3 sm:px-5 sm:py-4 md:grid-cols-[minmax(0,1fr)_auto]',
    highlighted && 'bg-lantern-soft/30 ring-1 ring-lantern/40 ring-inset',
  ]}
  data-source-id={source.source_id}
  data-highlighted={highlighted ? 'true' : 'false'}
>
  <div class="min-w-0">
    <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
      <h4 class="min-w-0 font-semibold break-words text-ink">{source.profile.title}</h4>
      <span class="min-w-0 text-xs break-all text-ink-subtle">{source.source_id}</span>
    </div>

    <dl class="mt-2 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-sm">
      <dt class="text-ink-muted">{m.sources_group_label()}</dt>
      <dd class="min-w-0 break-words text-ink">{groupLabel}</dd>
      <dt class="text-ink-muted">{m.sources_install_version()}</dt>
      <dd class="min-w-0 break-words text-ink">{source.version}</dd>
    </dl>

    <div class="mt-3">
      <p class="text-xs font-medium text-ink-muted">{m.sources_supported_intents()}</p>
      {#if source.profile.supported_intents.length > 0}
        <ul class="mt-1.5 flex flex-wrap gap-1.5" aria-label={m.sources_supported_intents()}>
          {#each source.profile.supported_intents as intent (intent)}
            <li class="rounded-md border border-hairline bg-surface-2 px-2 py-1 text-xs text-ink">
              {intentLabels[intent]()}
            </li>
          {/each}
        </ul>
      {:else}
        <p class="mt-1 text-xs text-ink-subtle">{m.sources_no_intents()}</p>
      {/if}
    </div>

    {#if source.profile.risk_notes.length > 0}
      <div class="mt-3">
        <p class="text-xs font-medium text-ink-muted">{m.sources_risk_notes()}</p>
        <ul class="mt-1 list-disc space-y-1 pl-5 text-xs leading-5 text-ink-muted">
          {#each source.profile.risk_notes as note (note)}
            <li>{note}</li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>
  <div class="flex min-w-44 flex-col items-start gap-2 md:items-end">
    <div class="flex min-h-6 items-center gap-1.5 text-xs font-medium text-positive">
      <Icon name="check-circle" class="size-5" />
      <span>{m.sources_status_installed()}</span>
    </div>
    {#if !source.document_ref}
      <p class="max-w-52 text-xs leading-5 text-ink-muted md:text-right">
        {m.sources_rules_original_unavailable()}
      </p>
    {/if}
    <Button href={workspaceHref} variant="outline" class="min-h-11 max-w-full whitespace-normal">
      <Icon name={source.document_ref ? 'pencil-simple' : 'file-text'} class="size-4" />
      <span>
        {source.document_ref ? m.sources_rules_edit_source() : m.sources_rules_repaste_action()}
      </span>
    </Button>
  </div>
</article>
