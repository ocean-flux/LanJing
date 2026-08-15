<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
  import { Button } from '$lib/components/ui/button/index.js';
  import { Input } from '$lib/components/ui/input/index.js';
  import ConfigSelect from './ConfigSelect.svelte';

  type RuleKind = 'CssSelector' | 'XPath' | 'JsonPath' | 'Regex';
  type RuleView = {
    kind: RuleKind;
    value: string;
    extractType: string;
    group: number;
  };

  type Props = {
    rules: unknown[];
    onChange: (rules: unknown[]) => void;
  };

  let { rules, onChange }: Props = $props();

  const ruleKinds: RuleKind[] = ['CssSelector', 'XPath', 'JsonPath', 'Regex'];
  const extractTypes = ['Text', 'Href', 'Src', 'Html', 'OwnText'];
  const keyForKind: Record<RuleKind, string> = {
    CssSelector: 'selector',
    XPath: 'expression',
    JsonPath: 'path',
    Regex: 'pattern',
  };

  function readRule(value: unknown): RuleView {
    if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
      const record = value as Record<string, unknown>;
      for (const kind of ruleKinds) {
        const body = record[kind];
        if (typeof body === 'object' && body !== null && !Array.isArray(body)) {
          const fields = body as Record<string, unknown>;
          return {
            kind,
            value:
              typeof fields[keyForKind[kind]] === 'string'
                ? (fields[keyForKind[kind]] as string)
                : '',
            extractType:
              typeof fields.extract_type === 'string' ? (fields.extract_type as string) : 'Text',
            group: typeof fields.group === 'number' ? fields.group : 0,
          };
        }
      }
    }
    return { kind: 'CssSelector', value: '', extractType: 'Text', group: 0 };
  }

  function writeRule(rule: RuleView): unknown {
    const body: Record<string, unknown> = { [keyForKind[rule.kind]]: rule.value };
    if (rule.kind === 'Regex') {
      body.group = Math.max(0, Math.floor(rule.group));
    } else {
      body.extract_type = rule.extractType;
      body.regex_clean = null;
    }
    return { [rule.kind]: body };
  }

  function update(index: number, patch: Partial<RuleView>) {
    const next = rules.map(readRule);
    next[index] = { ...next[index], ...patch };
    onChange(next.map(writeRule));
  }

  function addRule() {
    onChange([
      ...rules,
      writeRule({ kind: 'CssSelector', value: '', extractType: 'Text', group: 0 }),
    ]);
  }

  function removeRule(index: number) {
    onChange(rules.filter((_, itemIndex) => itemIndex !== index));
  }
</script>

<div class="flex flex-col gap-2">
  <div class="flex items-center justify-between">
    <p class="text-xs font-medium text-ink-muted">提取规则</p>
    <Button
      type="button"
      variant="ghost"
      size="icon-xs"
      aria-label="添加提取规则"
      onclick={addRule}
    >
      <Icon name="plus" class="size-3.5" />
    </Button>
  </div>

  {#if rules.length === 0}
    <p
      class="rounded-md border border-dashed border-hairline px-2 py-2 text-[11px] text-ink-subtle"
    >
      尚未配置规则
    </p>
  {:else}
    {#each rules as rule, index (index)}
      {@const view = readRule(rule)}
      <div class="flex flex-col gap-2 rounded-md border border-hairline bg-surface-1 p-2">
        <div class="flex items-center gap-2">
          <ConfigSelect
            value={view.kind}
            options={ruleKinds.map((kind) => ({ value: kind, label: kind }))}
            label="规则类型"
            class="min-w-0 flex-1"
            onChange={(kind) => update(index, { kind: kind as RuleKind })}
          />
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            aria-label="删除规则"
            onclick={() => removeRule(index)}
          >
            <Icon name="trash" class="size-3.5 text-ink-subtle" />
          </Button>
        </div>
        <Input
          aria-label="规则表达式"
          class="text-xs"
          value={view.value}
          placeholder={keyForKind[view.kind]}
          oninput={(event) => update(index, { value: event.currentTarget.value })}
        />
        {#if view.kind === 'Regex'}
          <label class="flex items-center gap-2 text-[11px] text-ink-muted">
            <span>匹配组</span>
            <Input
              aria-label="匹配组"
              type="number"
              min="0"
              class="w-20 text-xs"
              value={view.group}
              oninput={(event) => update(index, { group: Number(event.currentTarget.value) || 0 })}
            />
          </label>
        {:else}
          <ConfigSelect
            value={view.extractType}
            options={extractTypes.map((extractType) => ({
              value: extractType,
              label: extractType,
            }))}
            label="提取类型"
            class="text-xs"
            onChange={(extractType) => update(index, { extractType })}
          />
        {/if}
      </div>
    {/each}
  {/if}
</div>
