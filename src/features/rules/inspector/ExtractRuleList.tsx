//! Extract 节点的提取规则列表。
//!
//! 规则是 Rust 侧的外部标签枚举（`{ CssSelector: { selector, extract_type } }`），
//! 这里在「扁平视图」与「枚举 JSON」之间来回转换，画布与 core 只见 JSON。

import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import { useMessages } from '@/shared/i18n/messages';
import { ConfigListHeader, ConfigSelect, ConfigTextInput } from './fields';

type RuleKind = 'CssSelector' | 'XPath' | 'JsonPath' | 'Regex';

type RuleView = {
  kind: RuleKind;
  value: string;
  extractType: string;
  group: number;
};

const RULE_KINDS: readonly RuleKind[] = ['CssSelector', 'XPath', 'JsonPath', 'Regex'];
const EXTRACT_TYPES = ['Text', 'Href', 'Src', 'Html', 'OwnText'];

/** 每种规则承载表达式的字段名；也用作输入框的 placeholder。 */
const VALUE_KEY: Record<RuleKind, string> = {
  CssSelector: 'selector',
  XPath: 'expression',
  JsonPath: 'path',
  Regex: 'pattern',
};

const KIND_OPTIONS = RULE_KINDS.map((kind) => ({ value: kind, label: kind }));
const TYPE_OPTIONS = EXTRACT_TYPES.map((type) => ({ value: type, label: type }));

const DEFAULT_RULE: RuleView = { kind: 'CssSelector', value: '', extractType: 'Text', group: 0 };

function readRule(value: unknown): RuleView {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return DEFAULT_RULE;
  const record = value as Record<string, unknown>;
  for (const kind of RULE_KINDS) {
    const body = record[kind];
    if (typeof body !== 'object' || body === null || Array.isArray(body)) continue;
    const fields = body as Record<string, unknown>;
    const expression = fields[VALUE_KEY[kind]];
    return {
      kind,
      value: typeof expression === 'string' ? expression : '',
      extractType: typeof fields.extract_type === 'string' ? fields.extract_type : 'Text',
      group: typeof fields.group === 'number' ? fields.group : 0,
    };
  }
  return DEFAULT_RULE;
}

function writeRule(rule: RuleView): unknown {
  const body: Record<string, unknown> =
    rule.kind === 'Regex'
      ? { [VALUE_KEY[rule.kind]]: rule.value, group: Math.max(0, Math.floor(rule.group)) }
      : { [VALUE_KEY[rule.kind]]: rule.value, extract_type: rule.extractType, regex_clean: null };
  return { [rule.kind]: body };
}

export function ExtractRuleList({
  rules,
  onChange,
}: {
  rules: readonly unknown[];
  onChange: (rules: unknown[]) => void;
}) {
  const m = useMessages();
  const views = rules.map(readRule);

  const update = (index: number, patch: Partial<RuleView>) =>
    onChange(views.map((view, i) => writeRule(i === index ? { ...view, ...patch } : view)));

  return (
    <div className="flex flex-col gap-2">
      <ConfigListHeader
        title={m.rules_node_inspector_extract_rules()}
        addLabel={m.rules_node_inspector_extract_rule_add()}
        onAdd={() => onChange([...rules, writeRule(DEFAULT_RULE)])}
      />

      {views.length === 0 ? (
        <p className="border border-dashed border-hairline px-2 py-2 text-ui-sm text-ink-subtle">
          {m.rules_node_inspector_extract_rule_empty()}
        </p>
      ) : (
        views.map((view, index) => (
          // 规则没有稳定 id，两条完全相同的规则是合法配置，只能用下标。
          // eslint-disable-next-line react/no-array-index-key
          <div key={index} className="flex flex-col gap-2 border border-hairline bg-surface-1 p-2">
            <div className="flex items-center gap-2">
              <ConfigSelect
                label={m.rules_node_inspector_extract_rule_kind()}
                value={view.kind}
                options={KIND_OPTIONS}
                className="min-w-0 flex-1"
                onChange={(kind) => update(index, { kind: kind as RuleKind })}
              />
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label={m.rules_node_inspector_extract_rule_remove()}
                title={m.rules_node_inspector_extract_rule_remove()}
                onClick={() => onChange(rules.filter((_, i) => i !== index))}
              >
                <Icon name="trash" className="text-ink-subtle" />
              </Button>
            </div>

            <ConfigTextInput
              label={m.rules_node_inspector_extract_rule_expression()}
              placeholder={VALUE_KEY[view.kind]}
              className="font-mono"
              value={view.value}
              onChange={(value) => update(index, { value })}
            />

            {view.kind === 'Regex' ? (
              <div className="flex items-center gap-2">
                <span className="text-ui-sm text-ink-muted">
                  {m.rules_node_inspector_extract_rule_group()}
                </span>
                <ConfigTextInput
                  label={m.rules_node_inspector_extract_rule_group()}
                  type="number"
                  className="w-20"
                  value={view.group}
                  onChange={(value) => update(index, { group: Number(value) || 0 })}
                />
              </div>
            ) : (
              <ConfigSelect
                label={m.rules_node_inspector_extract_rule_type()}
                value={view.extractType}
                options={TYPE_OPTIONS}
                onChange={(extractType) => update(index, { extractType })}
              />
            )}
          </div>
        ))
      )}
    </div>
  );
}
