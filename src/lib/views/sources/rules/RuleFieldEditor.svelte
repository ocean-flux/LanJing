<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Textarea } from '$lib/components/ui/textarea';
  import { m } from '$lib/i18n';
  import {
    legadoFieldCatalog,
    parentJsonPointer,
    type JsonValue,
    type LegadoCatalogField,
    type SourceAuthoringDocument,
    type SourceAuthoringPatch,
  } from '$lib/rules/authoring';

  type Props = {
    document: SourceAuthoringDocument;
    disabled?: boolean;
    onPatch: (patch: SourceAuthoringPatch) => void | Promise<void>;
  };

  type FieldGroup = {
    id: string;
    label: string;
    fields: readonly LegadoCatalogField[];
  };

  const fieldGroupOrder = [
    'basic',
    'preferences',
    'search',
    'explore',
    'book_info',
    'toc',
    'content',
    'review',
  ] as const;

  let { document, disabled = false, onPatch }: Props = $props();

  const groups = $derived(
    fieldGroupOrder.map((id): FieldGroup => ({
      id,
      label: groupLabel(id),
      fields: legadoFieldCatalog.fields.filter((field) => field.group === id),
    })),
  );

  function groupLabel(group: string): string {
    switch (group) {
      case 'basic':
        return m.sources_rules_form_group_basic();
      case 'preferences':
        return m.sources_rules_form_group_preferences();
      case 'search':
        return m.sources_rules_form_group_search();
      case 'explore':
        return m.sources_rules_form_group_explore();
      case 'book_info':
        return m.sources_rules_form_group_book_info();
      case 'toc':
        return m.sources_rules_form_group_toc();
      case 'content':
        return m.sources_rules_form_group_content();
      case 'review':
        return m.sources_rules_form_group_review();
      default:
        return group;
    }
  }

  function fieldName(field: LegadoCatalogField): string {
    return field.pointer.slice(field.pointer.lastIndexOf('/') + 1) || field.pointer;
  }

  function lookupField(field: LegadoCatalogField) {
    return document.lookupPointer(field.pointer);
  }

  function fieldPresent(field: LegadoCatalogField): boolean {
    return lookupField(field).kind === 'found';
  }

  function fieldNodeType(field: LegadoCatalogField): string | null {
    const lookup = lookupField(field);
    return lookup.kind === 'found' ? lookup.entry.node.type : null;
  }

  function fieldValue(field: LegadoCatalogField): JsonValue | undefined {
    const lookup = lookupField(field);
    if (lookup.kind !== 'found') return undefined;
    const { node } = lookup.entry;
    if (node.type === 'string' || node.type === 'number' || node.type === 'boolean') {
      return node.value as string | number | boolean;
    }
    if (node.type === 'null') return null;
    return undefined;
  }

  function parentPresent(field: LegadoCatalogField): boolean {
    const parent = parentJsonPointer(field.pointer);
    return parent !== null && document.lookupPointer(parent).kind === 'found';
  }

  function fieldEditable(field: LegadoCatalogField): boolean {
    return !disabled && field.support !== 'blocked';
  }

  function supportLabel(field: LegadoCatalogField): string {
    if (field.support === 'executable') return m.sources_rules_support_executable();
    if (field.support === 'preserved') return m.sources_rules_support_preserved();
    return m.sources_rules_support_blocked();
  }

  function supportVariant(field: LegadoCatalogField): 'outline' | 'secondary' | 'destructive' {
    if (field.support === 'executable') return 'outline';
    if (field.support === 'preserved') return 'secondary';
    return 'destructive';
  }

  function defaultFieldValue(field: LegadoCatalogField): JsonValue {
    switch (field.type) {
      case 'integer':
        return 0;
      case 'boolean':
        return false;
      case 'object_or_string':
        return {};
      default:
        return '';
    }
  }

  function dispatchSet(field: LegadoCatalogField, value: JsonValue): void {
    void onPatch({
      operation: 'set',
      pointer: field.pointer,
      value,
      expectedEpoch: document.epoch,
    });
  }

  function addField(field: LegadoCatalogField): void {
    dispatchSet(field, defaultFieldValue(field));
  }

  function removeField(field: LegadoCatalogField): void {
    void onPatch({
      operation: 'remove',
      pointer: field.pointer,
      expectedEpoch: document.epoch,
    });
  }

  function commitText(field: LegadoCatalogField, event: Event): void {
    const value = (event.currentTarget as HTMLInputElement | HTMLTextAreaElement).value;
    dispatchSet(field, value);
  }

  function commitInteger(field: LegadoCatalogField, event: Event): void {
    const input = event.currentTarget as HTMLInputElement;
    const value = Number(input.value);
    if (Number.isSafeInteger(value)) dispatchSet(field, value);
  }

  function commitIntegerOrString(field: LegadoCatalogField, event: Event): void {
    const value = (event.currentTarget as HTMLInputElement).value;
    dispatchSet(
      field,
      fieldNodeType(field) === 'number' && /^-?\d+$/.test(value) ? Number(value) : value,
    );
  }

  function commitBoolean(field: LegadoCatalogField, event: Event): void {
    dispatchSet(field, (event.currentTarget as HTMLInputElement).checked);
  }
</script>

<section class="flex min-h-0 min-w-0 flex-col" aria-labelledby="rule-field-editor-title">
  <header class="border-b border-hairline px-4 py-3">
    <h2 id="rule-field-editor-title" class="text-sm font-semibold text-ink">
      {m.sources_rules_form_title()}
    </h2>
    <p class="mt-1 text-xs leading-5 text-ink-muted">{m.sources_rules_form_description()}</p>
  </header>

  {#if document.syntaxTree}
    <div class="min-h-0 flex-1 space-y-3 overflow-y-auto p-3 sm:p-4">
      {#each groups as group (group.id)}
        <section class="glass-control overflow-hidden rounded-xl border border-hairline">
          <h3 class="border-b border-hairline px-3 py-2.5 text-sm font-semibold text-ink">
            {group.label}
          </h3>
          <div class="divide-y divide-hairline">
            {#each group.fields as field (field.pointer)}
              {@const present = fieldPresent(field)}
              {@const value = fieldValue(field)}
              {@const nodeType = fieldNodeType(field)}
              <div
                class="grid min-w-0 gap-3 px-3 py-3 lg:grid-cols-[minmax(10rem,0.42fr)_minmax(0,1fr)]"
              >
                <div class="min-w-0">
                  <div class="flex flex-wrap items-center gap-1.5">
                    <code class="font-mono text-xs break-all text-ink">{fieldName(field)}</code>
                    <Badge variant={supportVariant(field)}>{supportLabel(field)}</Badge>
                    <span class="text-[0.6875rem] font-medium text-ink-subtle">
                      {field.required
                        ? m.sources_rules_form_required()
                        : m.sources_rules_form_optional()}
                    </span>
                  </div>
                  <p class="mt-1 font-mono text-[0.6875rem] break-all text-ink-subtle">
                    {field.pointer}
                  </p>
                </div>

                <div class="min-w-0">
                  {#if !present}
                    <div class="flex min-h-11 flex-wrap items-center justify-between gap-2">
                      <p class="text-xs text-ink-muted">
                        {parentPresent(field)
                          ? m.sources_rules_form_optional()
                          : m.sources_rules_form_parent_missing()}
                      </p>
                      <Button
                        type="button"
                        variant="outline"
                        class="min-h-11"
                        disabled={!fieldEditable(field) || !parentPresent(field)}
                        onclick={() => addField(field)}
                      >
                        <Icon name="plus" class="size-4" />
                        <span>{m.sources_rules_form_add()}</span>
                      </Button>
                    </div>
                  {:else if nodeType === 'object' || nodeType === 'array'}
                    <div
                      class="flex min-h-11 flex-wrap items-center justify-between gap-2 rounded-lg border border-hairline bg-surface-2 px-3 py-2"
                    >
                      <p class="text-xs leading-5 text-ink-muted">
                        {m.sources_rules_form_structured()}
                      </p>
                      {#if !field.required}
                        <Button
                          type="button"
                          variant="ghost"
                          class="min-h-11"
                          disabled={!fieldEditable(field)}
                          onclick={() => removeField(field)}
                        >
                          <Icon name="trash" class="size-4" />
                          <span>{m.sources_rules_form_remove()}</span>
                        </Button>
                      {/if}
                    </div>
                  {:else}
                    <div class="flex min-w-0 items-start gap-2">
                      <label class="min-w-0 flex-1">
                        <span class="sr-only">
                          {m.sources_rules_form_value({ field: fieldName(field) })}
                        </span>
                        {#if field.type === 'boolean'}
                          <span
                            class="flex min-h-11 items-center gap-3 rounded-lg border border-hairline bg-surface-1 px-3"
                          >
                            <Input
                              type="checkbox"
                              class="size-5 w-5 shrink-0"
                              checked={value === true}
                              disabled={!fieldEditable(field)}
                              onchange={(event) => commitBoolean(field, event)}
                            />
                            <span class="text-sm text-ink">{String(value === true)}</span>
                          </span>
                        {:else if field.type === 'integer'}
                          <Input
                            type="number"
                            class="min-h-11"
                            value={typeof value === 'number' ? value : ''}
                            disabled={!fieldEditable(field)}
                            onchange={(event) => commitInteger(field, event)}
                          />
                        {:else if field.type === 'object_or_string'}
                          <Textarea
                            class="min-h-24 font-mono text-sm"
                            value={typeof value === 'string' ? value : ''}
                            disabled={!fieldEditable(field)}
                            onchange={(event) => commitText(field, event)}
                          />
                        {:else}
                          <Input
                            type="text"
                            class="min-h-11 font-mono"
                            value={value === null || value === undefined ? '' : String(value)}
                            disabled={!fieldEditable(field)}
                            onchange={(event) =>
                              field.type === 'integer_or_string'
                                ? commitIntegerOrString(field, event)
                                : commitText(field, event)}
                          />
                        {/if}
                      </label>
                      {#if !field.required}
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon"
                          class="min-h-11 min-w-11"
                          aria-label={`${m.sources_rules_form_remove()} ${fieldName(field)}`}
                          disabled={!fieldEditable(field)}
                          onclick={() => removeField(field)}
                        >
                          <Icon name="trash" class="size-4" />
                        </Button>
                      {/if}
                    </div>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </section>
      {/each}
    </div>
  {:else}
    <div
      class="grid min-h-64 place-items-center px-5 text-center text-sm text-ink-muted"
      role="status"
    >
      {m.sources_rules_tree_empty()}
    </div>
  {/if}
</section>
