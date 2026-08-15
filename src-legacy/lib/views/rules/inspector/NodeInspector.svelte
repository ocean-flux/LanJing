<script lang="ts">
  import { m } from '$lib/i18n';
  import Icon from '$lib/components/Icon.svelte';
  import { Badge } from '$lib/components/ui/badge/index.js';
  import { Button } from '$lib/components/ui/button/index.js';
  import { Input } from '$lib/components/ui/input/index.js';
  import { RadioGroup, RadioGroupItem } from '$lib/components/ui/radio-group/index.js';
  import { Separator } from '$lib/components/ui/separator/index.js';
  import { Textarea } from '$lib/components/ui/textarea/index.js';
  import JsConfigEditor from './JsConfigEditor.svelte';
  import ExtractRuleList from './ExtractRuleList.svelte';
  import ConfigSelect from './ConfigSelect.svelte';
  import type { FlowNodeKind } from '$lib/rules/native-authoring/wire';
  import { canonicalConfig } from '$lib/rules/native-authoring/node-defaults';
  import {
    collectionPatch,
    collectionSelector,
    conditionExpression,
    conditionPatch,
    literalText,
    mergeInputValues,
    numberValue,
    parseLiteral,
    recordValue,
    stringArray,
    stringValue,
    type CollectionSelector,
    type ConditionExpressionPatch,
    type MergeInputValue,
  } from './node-config';

  type Props = {
    nodeId: string | null;
    nodeType: FlowNodeKind | null;
    config: Record<string, unknown> | null;
    onChange: (patch: Record<string, unknown>) => void;
  };

  let { nodeId, nodeType, config, onChange: _onChange }: Props = $props();

  const typeLabels: Record<FlowNodeKind, () => string> = {
    http: () => m.rules_node_inspector_type_http(),
    js: () => m.rules_node_inspector_type_js(),
    extract: () => m.rules_node_inspector_type_extract(),
    mapper: () => m.rules_node_inspector_type_mapper(),
    merge: () => m.rules_node_inspector_type_merge(),
    condition: () => m.rules_node_inspector_type_condition(),
    loop: () => m.rules_node_inspector_type_loop(),
  };

  const activeConfig = $derived(nodeType ? canonicalConfig(nodeType, config) : {});
  const expression = $derived(conditionExpression(activeConfig));
  const selector = $derived(collectionSelector(activeConfig));
  const mergeInputs = $derived(mergeInputValues(activeConfig.inputs));
  const branches = $derived(stringArray(activeConfig.branches));
  const headers = $derived(Object.entries(recordValue(activeConfig.headers)));
  const identityFields = $derived(stringArray(activeConfig.identity_fields));

  function setField(field: string, value: unknown) {
    _onChange({ [field]: value });
  }

  function updateHeaders(next: Array<[string, string]>) {
    _onChange({ headers: Object.fromEntries(next.filter(([key]) => key.trim().length > 0)) });
  }

  function updateIdentityFields(next: string[]) {
    _onChange({ identity_fields: next });
  }

  function updateBranches(next: string[]) {
    const valid = next.map((branch) => branch.trim()).filter(Boolean);
    if (valid.length < 2) return;
    _onChange(conditionPatch(activeConfig, { branches: valid }));
  }

  function updateCondition(patch: ConditionExpressionPatch) {
    _onChange(conditionPatch(activeConfig, patch));
  }

  function updatePredicate(patch: Record<string, unknown>) {
    if (expression.mode !== 'typed') return;
    updateCondition({
      mode: 'typed',
      predicate: { ...expression.predicate, ...patch },
      true_branch: expression.true_branch,
      false_branch: expression.false_branch,
    });
  }

  function setConditionMode(mode: 'typed' | 'js') {
    if (mode === 'js') {
      _onChange({
        expression: { mode: 'js', code: expression.mode === 'js' ? expression.code : '' },
      });
      return;
    }
    const typed =
      expression.mode === 'typed'
        ? expression
        : {
            mode: 'typed' as const,
            predicate: { operator: 'exists', pointer: '' },
            true_branch: branches[0] ?? 'true',
            false_branch: branches[1] ?? 'false',
          };
    updateCondition(typed);
  }

  function setCollectionMode(mode: 'typed' | 'js') {
    const next: CollectionSelector =
      mode === 'js'
        ? { mode: 'js', code: selector.mode === 'js' ? selector.code : '' }
        : { mode: 'typed', pointer: selector.mode === 'typed' ? selector.pointer : '' };
    _onChange(collectionPatch(activeConfig, next));
  }

  function updateMergeInput(index: number, patch: Partial<MergeInputValue>) {
    const next = mergeInputs.map((input, inputIndex) =>
      inputIndex === index ? { ...input, ...patch } : input,
    );
    _onChange({ inputs: next.map((input, order) => ({ ...input, order })) });
  }

  function moveMergeInput(index: number, direction: -1 | 1) {
    const target = index + direction;
    if (target < 0 || target >= mergeInputs.length) return;
    const next = [...mergeInputs];
    const [item] = next.splice(index, 1);
    next.splice(target, 0, item);
    _onChange({ inputs: next.map((input, order) => ({ ...input, order })) });
  }

  function addMergeInput() {
    const index = mergeInputs.length;
    _onChange({
      inputs: [
        ...mergeInputs,
        {
          input_id: `input_${index + 1}`,
          handle: `in:${index}`,
          order: index,
          activation: 'optional',
        },
      ],
    });
  }

  function removeMergeInput(index: number) {
    if (mergeInputs.length <= 2) return;
    _onChange({
      inputs: mergeInputs
        .filter((_, inputIndex) => inputIndex !== index)
        .map((input, order) => ({ ...input, order })),
    });
  }

  function fieldLabel(field: string): string {
    const labels: Record<string, () => string> = {
      url: () => 'URL',
      method: () => m.rules_node_inspector_field_method(),
      headers: () => m.rules_node_inspector_field_headers(),
      body: () => m.rules_node_inspector_field_body(),
      expected_type: () => '预期类型',
      charset: () => '字符集',
      code: () => m.rules_node_inspector_field_script(),
      output: () => '输出类型',
      rules: () => '规则列表',
      field_rules: () => '字段规则',
      output_target: () => '产出目标',
      identity_fields: () => '身份字段',
      inputs: () => '输入声明',
      strategy: () => '合并策略',
      branches: () => '分支 handles',
      expression: () => m.rules_node_inspector_field_expression(),
      collection: () => 'collection',
      item_binding: () => 'item binding',
      index_binding: () => 'index binding',
      max_iterations: () => m.rules_node_inspector_field_max_iterations(),
    };
    return labels[field]?.() ?? field;
  }

  const methodOptions = [
    { value: 'Get', label: 'GET' },
    { value: 'Post', label: 'POST' },
  ];
  const expectedTypeOptions = [
    { value: 'Html', label: 'HTML' },
    { value: 'Xml', label: 'XML' },
    { value: 'Json', label: 'JSON' },
  ];
  const httpOutputOptions = [
    { value: 'json', label: 'JSON' },
    { value: 'raw', label: 'RAW' },
  ];
  const extractOutputOptions = [
    { value: 'Media', label: 'Media' },
    { value: 'Units', label: 'Units' },
    { value: 'Asset', label: 'Asset' },
  ];
  const mapperOutputOptions = [
    { value: 'items', label: 'Items' },
    { value: 'discovery', label: 'Discovery' },
    { value: 'units', label: 'Units' },
    { value: 'assets', label: 'Assets' },
  ];
  const mergeStrategyOptions = [
    { value: 'single_active', label: 'Single active' },
    { value: 'collect_array', label: 'Collect array' },
    { value: 'concat_arrays', label: 'Concat arrays' },
    { value: 'overlay_objects', label: 'Overlay objects' },
  ];
  const activationOptions = [
    { value: 'required', label: 'Required' },
    { value: 'optional', label: 'Optional' },
  ];
  const conditionOperators = [
    'exists',
    'is_null',
    'eq',
    'ne',
    'lt',
    'lte',
    'gt',
    'gte',
    'contains',
  ].map((operator) => ({ value: operator, label: operator }));
</script>

{#if !nodeId || !nodeType}
  <div class="flex min-h-40 items-center justify-center px-4">
    <p class="text-center text-sm text-ink-muted">{m.rules_node_inspector_no_selection()}</p>
  </div>
{:else}
  <div class="flex flex-col gap-3">
    <div class="flex items-center gap-2 px-1">
      <Badge variant="outline" class="tracking-wider uppercase">{typeLabels[nodeType]()}</Badge>
      <code class="truncate text-xs text-ink-muted">{nodeId}</code>
    </div>
    <Separator />

    {#if nodeType === 'http'}
      <section class="flex flex-col gap-2 px-1" aria-labelledby="http-config-title">
        <h3 id="http-config-title" class="text-xs font-semibold text-ink">请求</h3>
        <label class="flex flex-col gap-1 text-xs text-ink-muted">
          {fieldLabel('method')}
          <ConfigSelect
            value={stringValue(activeConfig.method, 'Get')}
            options={methodOptions}
            label={fieldLabel('method')}
            onChange={(value) => setField('method', value)}
          />
        </label>
        <label class="flex flex-col gap-1 text-xs text-ink-muted">
          {fieldLabel('url')}
          <Input
            value={stringValue(activeConfig.url)}
            oninput={(event) => setField('url', event.currentTarget.value)}
          />
        </label>
        <label class="flex flex-col gap-1 text-xs text-ink-muted">
          {fieldLabel('expected_type')}
          <ConfigSelect
            value={stringValue(activeConfig.expected_type, 'Html')}
            options={expectedTypeOptions}
            label={fieldLabel('expected_type')}
            onChange={(value) => setField('expected_type', value)}
          />
        </label>
        <label class="flex flex-col gap-1 text-xs text-ink-muted">
          {fieldLabel('charset')}
          <Input
            value={stringValue(activeConfig.charset)}
            placeholder="可选"
            oninput={(event) => setField('charset', event.currentTarget.value || null)}
          />
        </label>
        <label class="flex flex-col gap-1 text-xs text-ink-muted">
          {fieldLabel('body')}
          <Textarea
            value={stringValue(activeConfig.body)}
            oninput={(event) => setField('body', event.currentTarget.value || null)}
          />
        </label>
        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between">
            <p class="text-xs text-ink-muted">{fieldLabel('headers')}</p>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              aria-label="添加请求头"
              onclick={() => updateHeaders([...headers, ['', '']])}
            >
              <Icon name="plus" class="size-3.5" />
            </Button>
          </div>
          {#each headers as [key, value], index (index)}
            <div class="flex gap-1.5">
              <Input
                class="min-w-0 flex-1"
                aria-label="请求头名称"
                value={key}
                oninput={(event) => {
                  const next = [...headers] as Array<[string, string]>;
                  next[index] = [event.currentTarget.value, value];
                  updateHeaders(next);
                }}
              />
              <Input
                class="min-w-0 flex-1"
                aria-label="请求头值"
                {value}
                oninput={(event) => {
                  const next = [...headers] as Array<[string, string]>;
                  next[index] = [key, event.currentTarget.value];
                  updateHeaders(next);
                }}
              />
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                aria-label="删除请求头"
                onclick={() => updateHeaders(headers.filter((_, itemIndex) => itemIndex !== index))}
              >
                <Icon name="trash" class="size-3.5 text-ink-subtle" />
              </Button>
            </div>
          {/each}
        </div>
      </section>
    {:else if nodeType === 'js'}
      <section class="flex flex-col gap-2 px-1">
        <label class="flex flex-col gap-1 text-xs text-ink-muted">
          {fieldLabel('output')}
          <ConfigSelect
            value={stringValue(activeConfig.output, 'json')}
            options={httpOutputOptions}
            label={fieldLabel('output')}
            onChange={(value) => setField('output', value)}
          />
        </label>
        <label class="flex flex-col gap-1 text-xs text-ink-muted">
          {fieldLabel('code')}
          <JsConfigEditor
            value={stringValue(activeConfig.code)}
            onChange={(code) => setField('code', code)}
            minLines={16}
          />
        </label>
      </section>
    {:else if nodeType === 'extract'}
      <section class="flex flex-col gap-3 px-1">
        <label class="flex flex-col gap-1 text-xs text-ink-muted">
          预期类型
          <ConfigSelect
            value={stringValue(activeConfig.expected_type, 'Html')}
            options={expectedTypeOptions}
            label="预期类型"
            onChange={(value) => setField('expected_type', value)}
          />
        </label>
        <label class="flex flex-col gap-1 text-xs text-ink-muted">
          产出目标
          <ConfigSelect
            value={stringValue(activeConfig.output_target, 'Media')}
            options={extractOutputOptions}
            label="产出目标"
            onChange={(value) => setField('output_target', value)}
          />
        </label>
        <ExtractRuleList
          rules={Array.isArray(activeConfig.rules) ? activeConfig.rules : []}
          onChange={(rules) => setField('rules', rules)}
        />
        <div
          class="rounded-md border border-hairline bg-surface-1 px-2 py-2 text-[11px] text-ink-subtle"
        >
          field_rules 当前包含 {typeof activeConfig.field_rules === 'object' &&
          activeConfig.field_rules !== null
            ? Object.keys(activeConfig.field_rules).length
            : 0} 个字段组；请在规则校验中确认列表模式完整性。
        </div>
      </section>
    {:else if nodeType === 'mapper'}
      <section class="flex flex-col gap-2 px-1">
        <label class="flex flex-col gap-1 text-xs text-ink-muted">
          产出类型
          <ConfigSelect
            value={stringValue(activeConfig.output, 'items')}
            options={mapperOutputOptions}
            label="产出类型"
            onChange={(value) => setField('output', value)}
          />
        </label>
        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between">
            <p class="text-xs text-ink-muted">{fieldLabel('identity_fields')}</p>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              aria-label="添加身份字段"
              onclick={() => updateIdentityFields([...identityFields, ''])}
            >
              <Icon name="plus" class="size-3.5" />
            </Button>
          </div>
          {#each identityFields as field, index (index)}
            <div class="flex gap-1.5">
              <Input
                class="min-w-0 flex-1"
                aria-label="身份字段"
                value={field}
                oninput={(event) => {
                  const next = [...identityFields];
                  next[index] = event.currentTarget.value;
                  updateIdentityFields(next);
                }}
              />
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                aria-label="删除身份字段"
                onclick={() =>
                  updateIdentityFields(
                    identityFields.filter((_, itemIndex) => itemIndex !== index),
                  )}
              >
                <Icon name="trash" class="size-3.5 text-ink-subtle" />
              </Button>
            </div>
          {/each}
        </div>
      </section>
    {:else if nodeType === 'merge'}
      <section class="flex flex-col gap-2 px-1">
        <label class="flex flex-col gap-1 text-xs text-ink-muted">
          {fieldLabel('strategy')}
          <ConfigSelect
            value={stringValue(activeConfig.strategy, 'single_active')}
            options={mergeStrategyOptions}
            label={fieldLabel('strategy')}
            onChange={(value) => setField('strategy', value)}
          />
        </label>
        <div class="flex items-center justify-between">
          <p class="text-xs font-medium text-ink-muted">输入声明</p>
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            aria-label="添加合并输入"
            onclick={addMergeInput}
          >
            <Icon name="plus" class="size-3.5" />
          </Button>
        </div>
        {#each mergeInputs as input, index (input.input_id)}
          <div class="flex flex-col gap-2 rounded-md border border-hairline bg-surface-1 p-2">
            <div class="flex items-center gap-1">
              <span class="flex-1 text-[11px] text-ink-subtle">order {input.order}</span>
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                aria-label="上移输入"
                disabled={index === 0}
                onclick={() => moveMergeInput(index, -1)}
                ><Icon name="caret-up" class="size-3.5" /></Button
              >
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                aria-label="下移输入"
                disabled={index === mergeInputs.length - 1}
                onclick={() => moveMergeInput(index, 1)}
                ><Icon name="caret-down" class="size-3.5" /></Button
              >
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                aria-label="删除合并输入"
                disabled={mergeInputs.length <= 2}
                onclick={() => removeMergeInput(index)}
                ><Icon name="trash" class="size-3.5 text-ink-subtle" /></Button
              >
            </div>
            <Input
              aria-label="输入 ID"
              value={input.input_id}
              oninput={(event) => updateMergeInput(index, { input_id: event.currentTarget.value })}
            />
            <Input
              aria-label="输入句柄"
              value={input.handle}
              oninput={(event) => updateMergeInput(index, { handle: event.currentTarget.value })}
            />
            <ConfigSelect
              value={input.activation}
              options={activationOptions}
              label="激活条件"
              onChange={(value) =>
                updateMergeInput(index, { activation: value as 'required' | 'optional' })}
            />
          </div>
        {/each}
      </section>
    {:else if nodeType === 'condition'}
      <section class="flex flex-col gap-3 px-1">
        <RadioGroup
          value={expression.mode}
          onValueChange={(value) => setConditionMode(value as 'typed' | 'js')}
          orientation="horizontal"
          class="flex items-center gap-1 rounded-md border border-hairline p-1"
          aria-label="条件表达式模式"
        >
          <div
            class="group relative min-w-0 flex-1 rounded-md has-data-[state=checked]:bg-surface-2 has-data-[state=checked]:text-ink"
          >
            <RadioGroupItem
              value="typed"
              aria-label="Typed"
              class="absolute inset-0 z-10 size-full rounded-md border-transparent bg-transparent opacity-0 focus-visible:opacity-100 focus-visible:ring-2"
            />
            <span
              class="pointer-events-none flex h-(--density-control-sm) items-center justify-center text-xs text-ink-muted group-has-data-[state=checked]:text-ink"
              >Typed</span
            >
          </div>
          <div
            class="group relative min-w-0 flex-1 rounded-md has-data-[state=checked]:bg-surface-2 has-data-[state=checked]:text-ink"
          >
            <RadioGroupItem
              value="js"
              aria-label="JS"
              class="absolute inset-0 z-10 size-full rounded-md border-transparent bg-transparent opacity-0 focus-visible:opacity-100 focus-visible:ring-2"
            />
            <span
              class="pointer-events-none flex h-(--density-control-sm) items-center justify-center text-xs text-ink-muted group-has-data-[state=checked]:text-ink"
              >JS</span
            >
          </div>
        </RadioGroup>
        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between">
            <p class="text-xs font-medium text-ink-muted">{fieldLabel('branches')}</p>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              aria-label="添加条件分支"
              onclick={() => updateBranches([...branches, `branch_${branches.length + 1}`])}
              ><Icon name="plus" class="size-3.5" /></Button
            >
          </div>
          {#each branches as branch, index (index)}
            <div class="flex gap-1.5">
              <Input
                class="min-w-0 flex-1"
                aria-label="分支 handle"
                value={branch}
                oninput={(event) => {
                  const next = [...branches];
                  next[index] = event.currentTarget.value;
                  updateBranches(next);
                }}
              />
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                aria-label="删除条件分支"
                disabled={branches.length <= 2}
                onclick={() =>
                  updateBranches(branches.filter((_, itemIndex) => itemIndex !== index))}
                ><Icon name="trash" class="size-3.5 text-ink-subtle" /></Button
              >
            </div>
          {/each}
        </div>
        {#if expression.mode === 'typed'}
          <label class="flex flex-col gap-1 text-xs text-ink-muted">
            操作符
            <ConfigSelect
              value={expression.predicate.operator}
              options={conditionOperators}
              label="操作符"
              onChange={(operator) => updatePredicate({ operator })}
            />
          </label>
          <label class="flex flex-col gap-1 text-xs text-ink-muted">
            JSON Pointer
            <Input
              value={expression.predicate.pointer}
              oninput={(event) => updatePredicate({ pointer: event.currentTarget.value })}
            />
          </label>
          {#if !['exists', 'is_null'].includes(expression.predicate.operator)}
            <label class="flex flex-col gap-1 text-xs text-ink-muted">
              Typed literal
              <Input
                value={literalText(expression.predicate.value)}
                oninput={(event) =>
                  updatePredicate({ value: parseLiteral(event.currentTarget.value) })}
              />
            </label>
          {/if}
          <div class="grid grid-cols-2 gap-2">
            <label class="flex flex-col gap-1 text-xs text-ink-muted">
              <span>true branch</span>
              <ConfigSelect
                value={expression.true_branch}
                options={branches.map((branch) => ({ value: branch, label: branch }))}
                label="true branch"
                onChange={(value) =>
                  updateCondition({
                    mode: 'typed',
                    true_branch: value,
                    false_branch: expression.false_branch,
                    predicate: expression.predicate,
                  })}
              />
            </label>
            <label class="flex flex-col gap-1 text-xs text-ink-muted">
              <span>false branch</span>
              <ConfigSelect
                value={expression.false_branch}
                options={branches.map((branch) => ({ value: branch, label: branch }))}
                label="false branch"
                onChange={(value) =>
                  updateCondition({
                    mode: 'typed',
                    true_branch: expression.true_branch,
                    false_branch: value,
                    predicate: expression.predicate,
                  })}
              />
            </label>
          </div>
        {:else}
          <label class="flex flex-col gap-1 text-xs text-ink-muted"
            >JS expression<JsConfigEditor
              value={expression.code}
              onChange={(code) => setField('expression', { mode: 'js', code })}
              minLines={12}
            /></label
          >
        {/if}
      </section>
    {:else if nodeType === 'loop'}
      <section class="flex flex-col gap-3 px-1">
        <RadioGroup
          value={selector.mode}
          onValueChange={(value) => setCollectionMode(value as 'typed' | 'js')}
          orientation="horizontal"
          class="flex items-center gap-1 rounded-md border border-hairline p-1"
          aria-label="集合选择模式"
        >
          <div
            class="group relative min-w-0 flex-1 rounded-md has-data-[state=checked]:bg-surface-2 has-data-[state=checked]:text-ink"
          >
            <RadioGroupItem
              value="typed"
              aria-label="Typed"
              class="absolute inset-0 z-10 size-full rounded-md border-transparent bg-transparent opacity-0 focus-visible:opacity-100 focus-visible:ring-2"
            />
            <span
              class="pointer-events-none flex h-(--density-control-sm) items-center justify-center text-xs text-ink-muted group-has-data-[state=checked]:text-ink"
              >Typed</span
            >
          </div>
          <div
            class="group relative min-w-0 flex-1 rounded-md has-data-[state=checked]:bg-surface-2 has-data-[state=checked]:text-ink"
          >
            <RadioGroupItem
              value="js"
              aria-label="JS"
              class="absolute inset-0 z-10 size-full rounded-md border-transparent bg-transparent opacity-0 focus-visible:opacity-100 focus-visible:ring-2"
            />
            <span
              class="pointer-events-none flex h-(--density-control-sm) items-center justify-center text-xs text-ink-muted group-has-data-[state=checked]:text-ink"
              >JS</span
            >
          </div>
        </RadioGroup>
        {#if selector.mode === 'typed'}
          <label class="flex flex-col gap-1 text-xs text-ink-muted"
            >collection JSON Pointer<Input
              value={selector.pointer}
              oninput={(event) =>
                setField('collection', { mode: 'typed', pointer: event.currentTarget.value })}
            /></label
          >
        {:else}
          <label class="flex flex-col gap-1 text-xs text-ink-muted"
            >collection JS<JsConfigEditor
              value={selector.code}
              onChange={(code) => setField('collection', { mode: 'js', code })}
              minLines={10}
            /></label
          >
        {/if}
        <div class="grid grid-cols-2 gap-2">
          <label class="flex flex-col gap-1 text-xs text-ink-muted"
            >item binding<Input
              value={stringValue(activeConfig.item_binding, 'item')}
              oninput={(event) => setField('item_binding', event.currentTarget.value)}
            /></label
          >
          <label class="flex flex-col gap-1 text-xs text-ink-muted"
            >index binding<Input
              value={stringValue(activeConfig.index_binding, 'index')}
              oninput={(event) => setField('index_binding', event.currentTarget.value)}
            /></label
          >
        </div>
        <label class="flex flex-col gap-1 text-xs text-ink-muted"
          >{fieldLabel('max_iterations')} (1-256)<Input
            type="number"
            min="1"
            max="256"
            value={numberValue(activeConfig.max_iterations, 64)}
            oninput={(event) =>
              setField(
                'max_iterations',
                Math.min(256, Math.max(1, Number(event.currentTarget.value) || 64)),
              )}
          /></label
        >
      </section>
    {/if}

    <p class="px-1 text-[11px] leading-4 text-ink-subtle">
      字段直接映射到当前规则 Definition；保存前由 compiler 做最终校验。
    </p>
  </div>
{/if}
