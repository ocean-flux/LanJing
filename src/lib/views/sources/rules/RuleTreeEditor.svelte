<script lang="ts">
  import { SvelteSet } from 'svelte/reactivity';
  import Icon from '$lib/components/Icon.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
  } from '$lib/components/ui/dialog';
  import { Input } from '$lib/components/ui/input';
  import { Label } from '$lib/components/ui/label';
  import { m } from '$lib/i18n';
  import {
    appendJsonPointer,
    getCatalogField,
    parentJsonPointer,
    type AuthoringPointerNode,
    type JsonValue,
    type SourceAuthoringDocument,
    type SourceAuthoringPatch,
    type SupportClass,
  } from '$lib/rules/authoring';

  type Props = {
    document: SourceAuthoringDocument;
    disabled?: boolean;
    onPatch: (patch: SourceAuthoringPatch) => void | Promise<void>;
  };

  type TreeRow = {
    id: string;
    pointer: string;
    depth: number;
    entry: AuthoringPointerNode;
    support: SupportClass;
  };

  type NewValueType = 'string' | 'number' | 'boolean' | 'null' | 'object' | 'array';

  let { document, disabled = false, onPatch }: Props = $props();

  const collapsedPointers = new SvelteSet<string>();
  let addOpen = $state(false);
  let addParent = $state<TreeRow | null>(null);
  let newKey = $state('');
  let newValue = $state('');
  let newType = $state<NewValueType>('string');

  const rows = $derived.by(() => {
    const nextRows: TreeRow[] = [];
    for (const [pointer, entries] of document.pointerIndex) {
      for (const entry of entries) {
        nextRows.push({
          id: `${pointer}:${entry.node.offset}`,
          pointer,
          depth: entry.path.length,
          entry,
          support: getCatalogField(pointer)?.support ?? 'unknown',
        });
      }
    }
    return nextRows.sort((left, right) => left.entry.node.offset - right.entry.node.offset);
  });

  const visibleRows = $derived(rows.filter((row) => !hasCollapsedAncestor(row.pointer)));
  const addTargetPointer = $derived.by(() => {
    if (!addParent) return null;
    if (addParent.entry.node.type === 'array') {
      return appendJsonPointer(
        addParent.pointer,
        String(addParent.entry.node.children?.length ?? 0),
      );
    }
    const key = newKey.trim();
    return key ? appendJsonPointer(addParent.pointer, key) : null;
  });
  const canAdd = $derived(
    Boolean(
      addTargetPointer &&
      addParent &&
      !disabled &&
      addParent.support !== 'blocked' &&
      document.lookupPointer(addTargetPointer).kind === 'missing' &&
      (newType !== 'number' || Number.isFinite(Number(newValue))),
    ),
  );

  function hasCollapsedAncestor(pointer: string): boolean {
    let parent = parentJsonPointer(pointer);
    while (parent !== null) {
      if (collapsedPointers.has(parent)) return true;
      parent = parentJsonPointer(parent);
    }
    return false;
  }

  function isCollection(row: TreeRow): boolean {
    return row.entry.node.type === 'object' || row.entry.node.type === 'array';
  }

  function rowLabel(row: TreeRow): string {
    if (row.pointer === '') return m.sources_rules_tree_root();
    return row.pointer.slice(row.pointer.lastIndexOf('/') + 1);
  }

  function pointerLabel(row: TreeRow): string {
    return row.pointer || '/';
  }

  function supportLabel(support: SupportClass): string {
    if (support === 'executable') return m.sources_rules_support_executable();
    if (support === 'preserved') return m.sources_rules_support_preserved();
    if (support === 'blocked') return m.sources_rules_support_blocked();
    return m.sources_rules_support_unknown();
  }

  function supportVariant(
    support: SupportClass,
  ): 'outline' | 'secondary' | 'destructive' | 'ghost' {
    if (support === 'executable') return 'outline';
    if (support === 'preserved') return 'secondary';
    if (support === 'blocked') return 'destructive';
    return 'ghost';
  }

  function rowEditable(row: TreeRow): boolean {
    return !disabled && row.support !== 'blocked';
  }

  function scalarValue(row: TreeRow): string {
    const { node } = row.entry;
    if (node.type === 'null') return 'null';
    if (node.type === 'string' || node.type === 'number' || node.type === 'boolean') {
      return String(node.value);
    }
    return '';
  }

  function toggleRow(row: TreeRow): void {
    if (collapsedPointers.has(row.pointer)) collapsedPointers.delete(row.pointer);
    else collapsedPointers.add(row.pointer);
  }

  function dispatchSet(pointer: string, value: JsonValue): void {
    void onPatch({ operation: 'set', pointer, value, expectedEpoch: document.epoch });
  }

  function commitScalar(row: TreeRow, event: Event): void {
    const input = event.currentTarget as HTMLInputElement;
    if (row.entry.node.type === 'number') {
      const value = Number(input.value);
      if (Number.isFinite(value)) dispatchSet(row.pointer, value);
      return;
    }
    dispatchSet(row.pointer, input.value);
  }

  function commitBoolean(row: TreeRow, event: Event): void {
    dispatchSet(row.pointer, (event.currentTarget as HTMLInputElement).checked);
  }

  function removeRow(row: TreeRow): void {
    void onPatch({ operation: 'remove', pointer: row.pointer, expectedEpoch: document.epoch });
  }

  function openAddDialog(row: TreeRow): void {
    addParent = row;
    newKey = '';
    newValue = '';
    newType = 'string';
    addOpen = true;
  }

  function closeAddDialog(): void {
    addOpen = false;
    addParent = null;
    newValue = '';
  }

  function createNewValue(): JsonValue | undefined {
    switch (newType) {
      case 'string':
        return newValue;
      case 'number': {
        const value = Number(newValue);
        return Number.isFinite(value) ? value : undefined;
      }
      case 'boolean':
        return newValue === 'true';
      case 'null':
        return null;
      case 'object':
        return {};
      case 'array':
        return [];
    }
  }

  function addChild(): void {
    if (!canAdd || !addParent || !addTargetPointer) return;
    const value = createNewValue();
    if (value === undefined) return;
    const operation = addParent.entry.node.type === 'array' ? 'insert' : 'set';
    void onPatch({ operation, pointer: addTargetPointer, value, expectedEpoch: document.epoch });
    closeAddDialog();
  }

  function typeLabel(type: NewValueType): string {
    switch (type) {
      case 'string':
        return m.sources_rules_tree_type_string();
      case 'number':
        return m.sources_rules_tree_type_number();
      case 'boolean':
        return m.sources_rules_tree_type_boolean();
      case 'null':
        return m.sources_rules_tree_type_null();
      case 'object':
        return m.sources_rules_tree_type_object();
      case 'array':
        return m.sources_rules_tree_type_array();
    }
  }

  const newValueTypes: readonly NewValueType[] = [
    'string',
    'number',
    'boolean',
    'null',
    'object',
    'array',
  ];
</script>

<section class="flex min-h-0 min-w-0 flex-col" aria-labelledby="rule-tree-editor-title">
  <header class="border-b border-hairline px-4 py-3">
    <h2 id="rule-tree-editor-title" class="text-sm font-semibold text-ink">
      {m.sources_rules_tree_title()}
    </h2>
    <p class="mt-1 text-xs leading-5 text-ink-muted">{m.sources_rules_tree_description()}</p>
  </header>

  {#if document.syntaxTree}
    <ul
      class="min-h-0 flex-1 list-none overflow-y-auto py-2"
      aria-label={m.sources_rules_tree_title()}
    >
      {#each visibleRows as row (row.id)}
        <li
          class="group/tree-row flex min-w-0 items-center gap-2 border-b border-hairline/70 py-2 pr-2 last:border-b-0"
          style:padding-left={`${0.5 + Math.min(row.depth, 10) * 0.875}rem`}
          data-tree-pointer={row.pointer}
        >
          {#if isCollection(row)}
            <Button
              type="button"
              variant="ghost"
              size="icon"
              class="min-h-11 min-w-11 shrink-0"
              aria-label={collapsedPointers.has(row.pointer)
                ? m.sources_rules_tree_expand({ pointer: pointerLabel(row) })
                : m.sources_rules_tree_collapse({ pointer: pointerLabel(row) })}
              aria-expanded={!collapsedPointers.has(row.pointer)}
              onclick={() => toggleRow(row)}
            >
              <Icon
                name="arrow-right"
                class={collapsedPointers.has(row.pointer) ? 'size-4' : 'size-4 rotate-90'}
              />
            </Button>
          {:else}
            <span class="w-11 shrink-0" aria-hidden="true"></span>
          {/if}

          <div class="min-w-0 flex-1">
            <div class="flex min-w-0 flex-wrap items-center gap-1.5">
              <code class="font-mono text-xs break-all text-ink">{rowLabel(row)}</code>
              <span class="text-[0.6875rem] text-ink-subtle">{row.entry.node.type}</span>
              <Badge variant={supportVariant(row.support)}>{supportLabel(row.support)}</Badge>
            </div>
            <p class="mt-0.5 font-mono text-[0.6875rem] break-all text-ink-subtle">
              {m.sources_rules_tree_pointer({ pointer: pointerLabel(row) })}
            </p>
          </div>

          {#if isCollection(row)}
            <Button
              type="button"
              variant="ghost"
              size="icon"
              class="min-h-11 min-w-11 shrink-0"
              aria-label={m.sources_rules_tree_add_child()}
              disabled={!rowEditable(row)}
              onclick={() => openAddDialog(row)}
            >
              <Icon name="plus" class="size-4" />
            </Button>
          {:else if row.entry.node.type === 'boolean'}
            <label class="grid min-h-11 min-w-11 shrink-0 place-items-center">
              <span class="sr-only">
                {m.sources_rules_tree_value({ pointer: pointerLabel(row) })}
              </span>
              <Input
                type="checkbox"
                class="size-5 w-5"
                checked={row.entry.node.value === true}
                disabled={!rowEditable(row)}
                onchange={(event) => commitBoolean(row, event)}
              />
            </label>
          {:else if row.entry.node.type === 'null'}
            <code class="shrink-0 text-xs text-ink-muted">null</code>
          {:else}
            <label class="w-[min(18rem,42%)] min-w-28 shrink-0">
              <span class="sr-only">
                {m.sources_rules_tree_value({ pointer: pointerLabel(row) })}
              </span>
              <Input
                type={row.entry.node.type === 'number' ? 'number' : 'text'}
                class="min-h-11 font-mono"
                value={scalarValue(row)}
                disabled={!rowEditable(row)}
                onchange={(event) => commitScalar(row, event)}
              />
            </label>
          {/if}

          {#if row.pointer !== ''}
            <Button
              type="button"
              variant="ghost"
              size="icon"
              class="min-h-11 min-w-11 shrink-0 opacity-80 group-hover/tree-row:opacity-100 focus-visible:opacity-100"
              aria-label={m.sources_rules_tree_remove()}
              disabled={!rowEditable(row)}
              onclick={() => removeRow(row)}
            >
              <Icon name="trash" class="size-4" />
            </Button>
          {/if}
        </li>
      {/each}
    </ul>
  {:else}
    <div
      class="grid min-h-64 place-items-center px-5 text-center text-sm text-ink-muted"
      role="status"
    >
      {m.sources_rules_tree_empty()}
    </div>
  {/if}
</section>

<Dialog bind:open={addOpen}>
  <DialogContent class="sm:max-w-lg">
    <DialogHeader>
      <DialogTitle>{m.sources_rules_tree_new_title()}</DialogTitle>
      <DialogDescription>{m.sources_rules_tree_new_description()}</DialogDescription>
    </DialogHeader>

    <div class="space-y-4 py-2">
      {#if addParent?.entry.node.type === 'object'}
        <div class="space-y-1.5">
          <Label for="rule-tree-new-key">{m.sources_rules_tree_new_key()}</Label>
          <Input id="rule-tree-new-key" class="min-h-11 font-mono" bind:value={newKey} />
        </div>
      {/if}

      <fieldset class="space-y-2">
        <legend class="text-sm font-medium text-ink">{m.sources_rules_tree_new_type()}</legend>
        <div class="grid grid-cols-2 gap-2 sm:grid-cols-3">
          {#each newValueTypes as type (type)}
            <button
              type="button"
              class={[
                'min-h-11 rounded-lg border px-3 text-sm font-medium outline-none focus-visible:shadow-[var(--focus-ring)]',
                newType === type
                  ? 'border-lantern-strong/50 bg-lantern-soft text-ink'
                  : 'border-hairline bg-surface-1 text-ink-muted hover:bg-surface-2',
              ]}
              aria-pressed={newType === type}
              onclick={() => {
                newType = type;
                newValue = type === 'boolean' ? 'false' : '';
              }}
            >
              {typeLabel(type)}
            </button>
          {/each}
        </div>
      </fieldset>

      {#if newType === 'string' || newType === 'number'}
        <div class="space-y-1.5">
          <Label for="rule-tree-new-value">{m.sources_rules_tree_new_value()}</Label>
          <Input
            id="rule-tree-new-value"
            type={newType === 'number' ? 'number' : 'text'}
            class="min-h-11 font-mono"
            bind:value={newValue}
          />
        </div>
      {:else if newType === 'boolean'}
        <div class="grid grid-cols-2 gap-2">
          {#each ['false', 'true'] as value (value)}
            <button
              type="button"
              class={[
                'min-h-11 rounded-lg border px-3 font-mono text-sm outline-none focus-visible:shadow-[var(--focus-ring)]',
                newValue === value
                  ? 'border-lantern-strong/50 bg-lantern-soft text-ink'
                  : 'border-hairline bg-surface-1 text-ink-muted hover:bg-surface-2',
              ]}
              aria-pressed={newValue === value}
              onclick={() => (newValue = value)}
            >
              {value}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <DialogFooter class="gap-2 sm:gap-2">
      <Button type="button" variant="outline" class="min-h-11" onclick={closeAddDialog}>
        {m.sources_rules_action_cancel()}
      </Button>
      <Button type="button" class="min-h-11" disabled={!canAdd} onclick={addChild}>
        {m.sources_rules_tree_new_confirm()}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
