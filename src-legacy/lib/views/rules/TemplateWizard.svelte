<script lang="ts">
  import { m } from '$lib/i18n';
  import Icon from '$lib/components/Icon.svelte';
  import { Button } from '$lib/components/ui/button/index.js';
  import { Input } from '$lib/components/ui/input/index.js';
  import { Label } from '$lib/components/ui/label/index.js';
  import {
    SelectTrigger,
    SelectContent,
    SelectItem,
    Select,
  } from '$lib/components/ui/select/index.js';
  import type { StandardIntent, ExpectedDataType } from '$lib/rules/native-authoring/wire';
  import type { NativeRuleEditorSession } from '$lib/rules/native-authoring/session.svelte';

  type Props = {
    session: NativeRuleEditorSession;
    oncancel: () => void;
    oncreated: () => void;
  };

  let { session, oncancel, oncreated }: Props = $props();

  let title = $state('');
  let intent = $state<StandardIntent>('Search');
  let dataType = $state<ExpectedDataType>('html');
  let baseUrl = $state('');
  let creating = $state(false);
  let error = $state<string | null>(null);

  const intentOptions: { value: StandardIntent; label: () => string }[] = [
    { value: 'Search', label: () => m.rules_template_wizard_intent_search() },
    { value: 'Discover', label: () => m.rules_template_wizard_intent_discover() },
    { value: 'ResolveItem', label: () => m.rules_template_wizard_intent_resolve_item() },
    { value: 'ListUnits', label: () => m.rules_template_wizard_intent_list_units() },
    { value: 'ResolveAsset', label: () => m.rules_template_wizard_intent_resolve_asset() },
    { value: 'ContinueAction', label: () => m.rules_template_wizard_intent_continue_action() },
  ];

  const dataTypeOptions: { value: ExpectedDataType; label: () => string }[] = [
    { value: 'html', label: () => m.rules_template_wizard_data_type_html() },
    { value: 'xml', label: () => m.rules_template_wizard_data_type_xml() },
    { value: 'json', label: () => m.rules_template_wizard_data_type_json() },
  ];

  let titleTouched = $state(false);
  let baseUrlTouched = $state(false);

  function titleValid(): boolean {
    return title.trim().length > 0;
  }
  function baseUrlValid(): boolean {
    return baseUrl.trim().length > 0;
  }
  function canCreate(): boolean {
    return titleValid() && baseUrlValid() && !creating;
  }

  async function handleCreate() {
    if (!canCreate()) return;
    creating = true;
    error = null;
    try {
      await session.createTemplate({
        title: title.trim(),
        intent,
        dataType,
        baseUrl: baseUrl.trim(),
      });
      oncreated();
    } catch (caught) {
      error = String(caught);
      creating = false;
    }
  }

  async function handleCreateBlank() {
    creating = true;
    error = null;
    try {
      await session.createBlank();
      oncreated();
    } catch (caught) {
      error = String(caught);
      creating = false;
    }
  }
</script>

<div class="flex flex-col gap-4">
  {#if error}
    <div
      role="alert"
      class="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive"
    >
      {error}
    </div>
  {/if}

  <div class="flex flex-col gap-1.5">
    <Label for="wizard-title">
      {m.rules_template_wizard_title_label()}
      <span class="text-destructive">*</span>
    </Label>
    <Input
      id="wizard-title"
      bind:value={title}
      placeholder={m.rules_template_wizard_title_placeholder()}
      onblur={() => {
        titleTouched = true;
      }}
      aria-invalid={titleTouched && !titleValid() ? true : undefined}
    />
    {#if titleTouched && !titleValid()}
      <p class="text-xs text-destructive">{m.rules_template_wizard_required()}</p>
    {/if}
  </div>

  <div class="flex flex-col gap-1.5">
    <Label for="wizard-intent">{m.rules_template_wizard_intent_label()}</Label>
    <Select type="single" bind:value={intent}>
      <SelectTrigger id="wizard-intent" class="w-full">
        <span>{intentOptions.find((o) => o.value === intent)?.label() ?? intent}</span>
      </SelectTrigger>
      <SelectContent>
        {#each intentOptions as option (option.value)}
          <SelectItem value={option.value}>{option.label()}</SelectItem>
        {/each}
      </SelectContent>
    </Select>
  </div>

  <div class="flex flex-col gap-1.5">
    <Label for="wizard-data-type">{m.rules_template_wizard_data_type_label()}</Label>
    <Select type="single" bind:value={dataType}>
      <SelectTrigger id="wizard-data-type" class="w-full">
        <span>{dataTypeOptions.find((o) => o.value === dataType)?.label() ?? dataType}</span>
      </SelectTrigger>
      <SelectContent>
        {#each dataTypeOptions as option (option.value)}
          <SelectItem value={option.value}>{option.label()}</SelectItem>
        {/each}
      </SelectContent>
    </Select>
  </div>

  <div class="flex flex-col gap-1.5">
    <Label for="wizard-base-url">
      {m.rules_template_wizard_base_url_label()}
      <span class="text-destructive">*</span>
    </Label>
    <Input
      id="wizard-base-url"
      type="url"
      bind:value={baseUrl}
      placeholder={m.rules_template_wizard_base_url_placeholder()}
      onblur={() => {
        baseUrlTouched = true;
      }}
      aria-invalid={baseUrlTouched && !baseUrlValid() ? true : undefined}
    />
    {#if baseUrlTouched && !baseUrlValid()}
      <p class="text-xs text-destructive">{m.rules_template_wizard_required()}</p>
    {/if}
  </div>

  <div class="flex items-center justify-between gap-2 pt-2">
    <Button type="button" variant="ghost" onclick={oncancel}>
      {m.rules_template_wizard_cancel()}
    </Button>
    <div class="flex gap-2">
      <Button type="button" variant="outline" disabled={creating} onclick={handleCreateBlank}>
        {m.rules_blank_graph()}
      </Button>
      <Button type="button" disabled={!canCreate()} onclick={handleCreate}>
        {#if creating}
          <Icon name="arrow-clockwise" class="size-4" />
        {/if}
        <span
          >{creating ? m.rules_template_wizard_creating() : m.rules_template_wizard_create()}</span
        >
      </Button>
    </div>
  </div>
</div>
