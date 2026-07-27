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
  const linkageLabel = $derived(
    source.document_ref
      ? m.sources_rules_documents_linked()
      : m.sources_rules_original_unavailable(),
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
    'grid min-w-0 gap-3 px-(--density-panel-padding-compact) py-2.5 sm:px-(--density-panel-padding) md:grid-cols-[minmax(0,1fr)_auto] md:items-start',
    highlighted && 'bg-lantern-soft/30 ring-1 ring-lantern/40 ring-inset',
  ]}
  data-source-id={source.source_id}
  data-highlighted={highlighted ? 'true' : 'false'}
>
  <div class="min-w-0">
    <div class="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
      <h4 class="min-w-0 font-semibold break-words text-ink">{source.profile.title}</h4>
      <span class="min-w-0 font-mono text-[0.6875rem] break-all text-ink-subtle">
        {source.source_id}
      </span>
    </div>

    <dl class="mt-1.5 flex flex-wrap gap-x-4 gap-y-1 text-xs">
      <div class="flex min-w-0 gap-1.5">
        <dt class="text-ink-muted">{m.sources_group_label()}</dt>
        <dd class="min-w-0 font-medium break-words text-ink">{groupLabel}</dd>
      </div>
      <div class="flex min-w-0 gap-1.5">
        <dt class="text-ink-muted">{m.sources_install_version()}</dt>
        <dd class="min-w-0 font-medium break-words text-ink">{source.version}</dd>
      </div>
    </dl>

    <div class="mt-2">
      <p class="text-xs font-medium text-ink-muted">{m.sources_supported_intents()}</p>
      {#if source.profile.supported_intents.length > 0}
        <ul class="mt-1 flex flex-wrap gap-1" aria-label={m.sources_supported_intents()}>
          {#each source.profile.supported_intents as intent (intent)}
            <li
              class="rounded-md border border-hairline bg-surface-2 px-1.5 py-0.5 text-xs text-ink"
            >
              {intentLabels[intent]()}
            </li>
          {/each}
        </ul>
      {:else}
        <p class="mt-1 text-xs text-ink-subtle">{m.sources_no_intents()}</p>
      {/if}
    </div>

    {#if source.profile.risk_notes.length > 0}
      <div class="mt-2">
        <p class="text-xs font-medium text-ink-muted">{m.sources_risk_notes()}</p>
        <ul class="mt-1 list-disc space-y-0.5 pl-4 text-xs leading-5 break-words text-ink-muted">
          {#each source.profile.risk_notes as note (note)}
            <li>{note}</li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>

  <div class="flex min-w-0 flex-wrap items-center gap-2 md:max-w-56 md:flex-col md:items-end">
    <div class="flex items-center gap-1.5 text-xs font-medium text-positive">
      <Icon name="check-circle" class="size-4" />
      <span>{m.sources_status_installed()}</span>
    </div>
    <p
      class={[
        'max-w-full text-xs leading-5 md:text-right',
        source.document_ref ? 'text-ink-muted' : 'text-warning',
      ]}
    >
      {linkageLabel}
    </p>
    <Button
      href={workspaceHref}
      variant="outline"
      size="sm"
      class="h-auto min-h-(--density-control-sm) max-w-full whitespace-normal"
    >
      <Icon name={source.document_ref ? 'pencil-simple' : 'file-text'} class="size-4" />
      <span>
        {source.document_ref ? m.sources_rules_edit_source() : m.sources_rules_repaste_action()}
      </span>
    </Button>
  </div>
</article>
