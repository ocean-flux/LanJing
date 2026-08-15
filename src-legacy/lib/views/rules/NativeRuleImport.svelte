<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
  import { Button } from '$lib/components/ui/button/index.js';
  import { Input } from '$lib/components/ui/input/index.js';
  import { m } from '$lib/i18n';
  import type { NativeRuleEditorSession } from '$lib/rules/native-authoring/session.svelte';
  import {
    MAX_NATIVE_RULE_IMPORT_BYTES,
    parseNativeRuleDefinition,
    type NativeRuleImportIssue,
  } from '$lib/rules/native-authoring/native-import';

  type Props = {
    session: NativeRuleEditorSession;
    oncreated: () => void | Promise<void>;
    class?: string;
  };

  let { session, oncreated, class: className }: Props = $props();
  let importing = $state(false);
  let error = $state<string | null>(null);

  function openFilePicker(event: MouseEvent): void {
    const button = event.currentTarget;
    if (!(button instanceof HTMLElement)) return;
    button.parentElement?.querySelector<HTMLInputElement>('input[type="file"]')?.click();
  }

  function issueMessage(issue: NativeRuleImportIssue): string {
    switch (issue) {
      case 'invalid_json':
        return m.rules_import_invalid_json();
      case 'root_not_object':
        return m.rules_import_root_not_object();
      case 'contract_mismatch':
        return m.rules_import_contract_mismatch();
      case 'schema_unsupported':
        return m.rules_import_schema_unsupported();
      case 'unknown_root_field':
        return m.rules_import_unknown_field();
      case 'invalid_source_identity':
        return m.rules_import_invalid_source_identity();
      case 'invalid_intent_exports':
        return m.rules_import_invalid_intent_exports();
      case 'invalid_flow':
        return m.rules_import_invalid_flow();
      case 'invalid_capability_manifest':
        return m.rules_import_invalid_capability_manifest();
      case 'invalid_source_id_rules':
        return m.rules_import_invalid_source_id_rules();
      case 'file_too_large':
        return m.rules_import_file_too_large();
    }
  }

  function errorMessage(caught: unknown): string {
    if (caught instanceof Error) return caught.message;
    if (typeof caught === 'object' && caught !== null) {
      const message = (caught as { message?: unknown }).message;
      if (typeof message === 'string' && message.length > 0) return message;
    }
    return m.rules_import_create_failed();
  }

  async function handleFileChange(event: Event): Promise<void> {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;

    error = null;
    if (file.size > MAX_NATIVE_RULE_IMPORT_BYTES) {
      error = issueMessage('file_too_large');
      return;
    }

    importing = true;
    try {
      const result = parseNativeRuleDefinition(await file.text());
      if (!result.ok) {
        error = issueMessage(result.issue);
        return;
      }
      const title = file.name.replace(/\.json$/i, '').trim() || m.rules_import_default_title();
      await session.createImported({ title, definition: result.definition });
      await oncreated();
    } catch (caught) {
      error = errorMessage(caught);
    } finally {
      importing = false;
    }
  }
</script>

<div class="flex min-w-0 flex-col gap-1.5">
  <Button
    type="button"
    variant="outline"
    size="sm"
    class={className}
    disabled={importing}
    aria-busy={importing}
    onclick={openFilePicker}
  >
    <Icon name="upload-simple" class="size-3.5" />
    <span>{importing ? m.rules_importing() : m.rules_import_native()}</span>
  </Button>
  <Input
    type="file"
    accept="application/json,.json"
    class="!absolute !h-px !w-px !overflow-hidden"
    aria-label={m.rules_import_native()}
    onchange={handleFileChange}
  />
  {#if error}
    <p role="alert" class="max-w-64 text-[11px] leading-4 break-words text-destructive">{error}</p>
  {/if}
</div>
