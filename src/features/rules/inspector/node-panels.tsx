//! Inspector 的节点配置面板：字段、编辑器与顺序全部来自 descriptor 声明。
//!
//! 这里没有 kind → 面板的表。新增规则能力只要在 Rust descriptor 里声明字段，
//! 面板自动渲染；`specialized` 编辑器按 `editor_kind` 选择结构化子编辑器，与具体
//! 节点 kind 无关。
//!
//! 未安装能力（查不到 descriptor）不渲染任何字段，只给出 unavailable 说明：
//! 节点仍可展示、保存与 round-trip，但不能 validate/compile/execute。

import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import { useMessages } from '@/shared/i18n/messages';
import type { NodeFieldDescriptor, NodeFieldEditor } from '@/shared/tauri/rules';

import { messageKeyText } from '../labels';
import {
  collectionPatch,
  collectionSelector,
  conditionExpression,
  conditionPatch,
  literalText,
  mergeInputValues,
  parseLiteral,
  stringArray,
  stringValue,
  type CollectionSelector,
  type ConditionExpression,
  type JsonObject,
  type MergeInputValue,
} from '../model/node-config';
import { nodeDescriptor } from '../model/descriptor-registry';
import { ExtractRuleList } from './ExtractRuleList';
import { JsCodeEditor } from './JsCodeEditor';
import {
  ConfigField,
  ConfigListHeader,
  ConfigSelect,
  ConfigTextInput,
  ModeToggle,
  PairListEditor,
  StringListEditor,
} from './fields';

export type PanelProps = {
  /** 已经补齐默认值的 canonical config。 */
  config: JsonObject;
  onChange: (patch: JsonObject) => void;
};

/** 二元 predicate 需要比较值，一元的不需要。 */
const UNARY_OPERATORS = new Set(['exists', 'is_null']);

/** 竖排字段的标签文案。 */
function fieldLabel(m: ReturnType<typeof useMessages>, field: NodeFieldDescriptor): string {
  return messageKeyText(m, field.label_key) ?? field.name;
}

/** Select 字段。 */
function SelectField({ field, config, onChange }: PanelProps & { field: NodeFieldDescriptor }) {
  const m = useMessages();
  const label = fieldLabel(m, field);
  const editor = field.editor as Extract<NodeFieldEditor, { editor: 'select' }>;
  const value = stringValue(config[field.name], editor.options[0]?.value ?? '');
  return (
    <ConfigField label={label}>
      {(id) => (
        <ConfigSelect
          id={id}
          label={label}
          value={value}
          options={editor.options}
          onChange={(next) => onChange({ [field.name]: next })}
        />
      )}
    </ConfigField>
  );
}

/** 单行文本 / 数值字段。 */
function TextField({ field, config, onChange }: PanelProps & { field: NodeFieldDescriptor }) {
  const m = useMessages();
  const label = fieldLabel(m, field);
  if (field.editor.editor === 'number') {
    const { min, max } = field.editor;
    const fallback = min ?? 0;
    return (
      <ConfigField label={label}>
        {(id) => (
          <ConfigTextInput
            id={id}
            type="number"
            label={label}
            value={Number(config[field.name] ?? fallback)}
            onChange={(next) => {
              const parsed = Number(next);
              if (!Number.isFinite(parsed)) return;
              onChange({ [field.name]: Math.min(max ?? parsed, Math.max(min ?? parsed, parsed)) });
            }}
          />
        )}
      </ConfigField>
    );
  }
  return (
    <ConfigField label={label}>
      {(id) => (
        <ConfigTextInput
          id={id}
          label={label}
          value={stringValue(config[field.name])}
          onChange={(next) => onChange({ [field.name]: next })}
        />
      )}
    </ConfigField>
  );
}

/** 多行源码字段。 */
function CodeField({ field, config, onChange }: PanelProps & { field: NodeFieldDescriptor }) {
  const m = useMessages();
  const label = fieldLabel(m, field);
  return (
    <ConfigField label={label}>
      {() => (
        <JsCodeEditor
          aria-label={label}
          value={stringValue(config[field.name])}
          onChange={(next) => onChange({ [field.name]: next })}
        />
      )}
    </ConfigField>
  );
}

/** 字符串列表字段。 */
function StringListField({ field, config, onChange }: PanelProps & { field: NodeFieldDescriptor }) {
  const m = useMessages();
  const editor = field.editor as Extract<NodeFieldEditor, { editor: 'string_list' }>;
  return (
    <StringListEditor
      title={fieldLabel(m, field)}
      values={stringArray(config[field.name])}
      itemLabel={messageKeyText(m, editor.item_label_key) ?? field.name}
      addLabel={messageKeyText(m, editor.add_label_key) ?? field.name}
      removeLabel={messageKeyText(m, editor.remove_label_key) ?? field.name}
      minLength={editor.min_items}
      onChange={(next) => onChange({ [field.name]: next })}
    />
  );
}

/** Key/value 对列表字段。 */
function PairListField({ field, config, onChange }: PanelProps & { field: NodeFieldDescriptor }) {
  const m = useMessages();
  const editor = field.editor as Extract<NodeFieldEditor, { editor: 'pair_list' }>;
  const pairs = Object.entries(
    typeof config[field.name] === 'object' && config[field.name] !== null
      ? (config[field.name] as Record<string, string>)
      : {},
  ).filter((entry): entry is [string, string] => typeof entry[1] === 'string');
  return (
    <PairListEditor
      title={fieldLabel(m, field)}
      pairs={pairs}
      keyLabel={messageKeyText(m, editor.key_label_key) ?? field.name}
      valueLabel={messageKeyText(m, editor.value_label_key) ?? field.name}
      addLabel={messageKeyText(m, editor.add_label_key) ?? field.name}
      removeLabel={messageKeyText(m, editor.remove_label_key) ?? field.name}
      onChange={(next) => onChange({ [field.name]: Object.fromEntries(next) })}
    />
  );
}

/** Merge 的 inputs：可增删与上下移动，`order` 始终是位置。 */
function MergeInputsEditor({ config, onChange }: PanelProps) {
  const m = useMessages();
  const inputs = mergeInputValues(config.inputs);
  const commit = (next: MergeInputValue[]) =>
    onChange({
      inputs: next.map((input, index) => ({ ...input, order: index, handle: `in:${index}` })),
    });
  const move = (from: number, to: number) => {
    if (to < 0 || to >= inputs.length) return;
    const next = [...inputs];
    const [moved] = next.splice(from, 1);
    if (moved === undefined) return;
    next.splice(to, 0, moved);
    commit(next);
  };
  return (
    <div className="flex flex-col gap-2">
      <ConfigListHeader
        title={m.rules_node_inspector_field_inputs()}
        addLabel={m.rules_node_inspector_merge_input_add()}
        onAdd={() =>
          commit([
            ...inputs,
            {
              input_id: `input_${inputs.length + 1}`,
              handle: `in:${inputs.length}`,
              order: inputs.length,
              activation: 'optional',
            },
          ])
        }
      />
      {inputs.length <= 2 ? (
        <p className="text-ui-sm text-ink-muted">{m.rules_node_inspector_merge_input_min()}</p>
      ) : null}
      {inputs.map((input, index) => (
        <div
          key={`${input.input_id}-${String(index)}`}
          className="flex flex-col gap-1 border border-hairline p-2"
        >
          <ConfigTextInput
            label={m.rules_node_inspector_merge_input_id()}
            value={input.input_id}
            onChange={(next) =>
              commit(inputs.map((item, i) => (i === index ? { ...item, input_id: next } : item)))
            }
          />
          <ConfigTextInput
            label={m.rules_node_inspector_merge_input_handle()}
            value={input.handle}
            onChange={(next) =>
              commit(inputs.map((item, i) => (i === index ? { ...item, handle: next } : item)))
            }
          />
          <ConfigSelect
            label={m.rules_node_inspector_merge_input_activation()}
            value={input.activation}
            options={[
              { value: 'required', label: 'Required' },
              { value: 'optional', label: 'Optional' },
            ]}
            onChange={(next) =>
              commit(
                inputs.map((item, i) =>
                  i === index
                    ? { ...item, activation: next as MergeInputValue['activation'] }
                    : item,
                ),
              )
            }
          />
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={m.rules_node_inspector_merge_input_up()}
              disabled={index === 0}
              onClick={() => move(index, index - 1)}
            >
              <Icon name="arrow-left" />
            </Button>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={m.rules_node_inspector_merge_input_down()}
              disabled={index === inputs.length - 1}
              onClick={() => move(index, index + 1)}
            >
              <Icon name="arrow-right" />
            </Button>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={m.rules_node_inspector_merge_input_remove()}
              disabled={inputs.length <= 2}
              onClick={() => commit(inputs.filter((_, i) => i !== index))}
            >
              <Icon name="minus" />
            </Button>
          </div>
        </div>
      ))}
    </div>
  );
}

/** Condition 的 expression：typed predicate 或 js 表达式。 */
function ConditionExpressionEditor({ config, onChange }: PanelProps) {
  const m = useMessages();
  const expression = conditionExpression(config);
  const branches = stringArray(config.branches);
  const branchOptions = branches.map((branch) => ({ value: branch, label: branch }));
  const patchExpression = (patch: Partial<ConditionExpression>) =>
    onChange(conditionPatch(config, { ...expression, ...patch }));

  return (
    <div className="flex flex-col gap-2">
      <ModeToggle
        label={m.rules_node_inspector_condition_mode()}
        value={expression.mode}
        typedLabel={m.rules_node_inspector_mode_typed()}
        jsLabel={m.rules_node_inspector_mode_js()}
        onChange={(mode) => patchExpression(mode === 'js' ? { mode, code: '' } : { mode })}
      />
      {expression.mode === 'js' ? (
        <ConfigField label={m.rules_node_inspector_field_expression()}>
          {() => (
            <JsCodeEditor
              aria-label={m.rules_node_inspector_field_expression()}
              value={expression.code}
              onChange={(code) => patchExpression({ mode: 'js', code })}
            />
          )}
        </ConfigField>
      ) : (
        <>
          <ConfigField label={m.rules_node_inspector_field_operator()}>
            {(id) => (
              <ConfigSelect
                id={id}
                label={m.rules_node_inspector_field_operator()}
                value={expression.predicate.operator}
                options={OPERATOR_OPTIONS}
                onChange={(operator) =>
                  patchExpression({
                    mode: 'typed',
                    predicate: {
                      ...expression.predicate,
                      operator,
                      ...(UNARY_OPERATORS.has(operator) ? { value: undefined } : {}),
                    },
                  })
                }
              />
            )}
          </ConfigField>
          <ConfigField label={m.rules_node_inspector_field_pointer()}>
            {(id) => (
              <ConfigTextInput
                id={id}
                label={m.rules_node_inspector_field_pointer()}
                value={expression.predicate.pointer}
                onChange={(pointer) =>
                  patchExpression({
                    mode: 'typed',
                    predicate: { ...expression.predicate, pointer },
                  })
                }
              />
            )}
          </ConfigField>
          {UNARY_OPERATORS.has(expression.predicate.operator) ? null : (
            <ConfigField label={m.rules_node_inspector_field_value()}>
              {(id) => (
                <ConfigTextInput
                  id={id}
                  label={m.rules_node_inspector_field_value()}
                  value={literalText(expression.predicate.value)}
                  onChange={(next) =>
                    patchExpression({
                      mode: 'typed',
                      predicate: { ...expression.predicate, value: parseLiteral(next) },
                    })
                  }
                />
              )}
            </ConfigField>
          )}
          <ConfigField label={m.rules_node_inspector_true_branch()}>
            {(id) => (
              <ConfigSelect
                id={id}
                label={m.rules_node_inspector_true_branch()}
                value={expression.true_branch}
                options={branchOptions}
                onChange={(true_branch) => patchExpression({ mode: 'typed', true_branch })}
              />
            )}
          </ConfigField>
          <ConfigField label={m.rules_node_inspector_false_branch()}>
            {(id) => (
              <ConfigSelect
                id={id}
                label={m.rules_node_inspector_false_branch()}
                value={expression.false_branch}
                options={branchOptions}
                onChange={(false_branch) => patchExpression({ mode: 'typed', false_branch })}
              />
            )}
          </ConfigField>
        </>
      )}
    </div>
  );
}

const OPERATOR_OPTIONS = [
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

/** Loop 的 collection：pointer 或 js。 */
function LoopCollectionEditor({ config, onChange }: PanelProps) {
  const m = useMessages();
  const selector = collectionSelector(config);
  const commit = (next: CollectionSelector) => onChange(collectionPatch(config, next));
  return (
    <ConfigField label={m.rules_node_inspector_field_collection()}>
      {() => (
        <div className="flex flex-col gap-2">
          <ModeToggle
            label={m.rules_node_inspector_collection_mode()}
            value={selector.mode}
            typedLabel={m.rules_node_inspector_mode_typed()}
            jsLabel={m.rules_node_inspector_mode_js()}
            onChange={(mode) => commit(mode === 'js' ? { mode, code: '' } : { mode, pointer: '' })}
          />
          {selector.mode === 'js' ? (
            <JsCodeEditor
              aria-label={m.rules_node_inspector_field_collection()}
              minLines={10}
              value={selector.code}
              onChange={(code) => commit({ mode: 'js', code })}
            />
          ) : (
            <ConfigTextInput
              label={m.rules_node_inspector_field_collection()}
              value={selector.pointer}
              onChange={(pointer) => commit({ mode: 'typed', pointer })}
            />
          )}
        </div>
      )}
    </ConfigField>
  );
}

/** Extract 的 rules + field_rules 提示。 */
function ExtractRulesEditor({ config, onChange }: PanelProps) {
  const m = useMessages();
  const rules = Array.isArray(config.rules) ? config.rules : [];
  const fieldCount = Object.keys(
    typeof config.field_rules === 'object' && config.field_rules !== null
      ? (config.field_rules as Record<string, unknown>)
      : {},
  ).length;
  return (
    <div className="flex flex-col gap-2">
      <p className="text-ui-sm text-ink-muted">
        {m.rules_node_inspector_field_rules_hint({ count: fieldCount })}
      </p>
      <ExtractRuleList rules={rules} onChange={(next) => onChange({ rules: next })} />
    </div>
  );
}

/** `editor_kind` → 结构化子编辑器。按 editor_kind 选择，与节点 kind 无关。 */
const SPECIALIZED_EDITORS: Record<string, (props: PanelProps) => React.ReactElement> = {
  merge_inputs: MergeInputsEditor,
  condition_expression: ConditionExpressionEditor,
  loop_collection: LoopCollectionEditor,
  extract_rules: ExtractRulesEditor,
};

/** 一个字段的编辑器。 */
function FieldEditorControl({
  field,
  config,
  onChange,
}: PanelProps & { field: NodeFieldDescriptor }) {
  switch (field.editor.editor) {
    case 'select': {
      return <SelectField field={field} config={config} onChange={onChange} />;
    }
    case 'text':
    case 'number': {
      return <TextField field={field} config={config} onChange={onChange} />;
    }
    case 'code': {
      return <CodeField field={field} config={config} onChange={onChange} />;
    }
    case 'string_list': {
      return <StringListField field={field} config={config} onChange={onChange} />;
    }
    case 'pair_list': {
      return <PairListField field={field} config={config} onChange={onChange} />;
    }
    case 'specialized': {
      const Editor = SPECIALIZED_EDITORS[field.editor.editor_kind];
      return Editor === undefined ? null : <Editor config={config} onChange={onChange} />;
    }
  }
}

/** 未安装能力的节点：没有声明，如实说明不可校验/编译/执行。 */
function UnavailablePanel({ kind }: { kind: string }) {
  const m = useMessages();
  return (
    <p className="text-ui-sm text-ink-muted" data-node-capability="unavailable">
      {m.rules_node_unavailable({ kind })}
    </p>
  );
}

/**
 * 节点配置面板：字段与顺序由 descriptor 声明驱动。
 *
 * `kind` 是 wire 字符串；查不到声明即未安装能力，只提示、不渲染任何字段。
 */
export function NodeConfigPanel({ kind, config, onChange }: PanelProps & { kind: string }) {
  const descriptor = nodeDescriptor(kind);
  if (descriptor === undefined) return <UnavailablePanel kind={kind} />;
  return (
    <>
      {descriptor.fields.map((field) => (
        <FieldEditorControl key={field.name} field={field} config={config} onChange={onChange} />
      ))}
    </>
  );
}
