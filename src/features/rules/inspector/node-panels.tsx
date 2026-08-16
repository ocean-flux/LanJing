//! 七类节点的配置面板。
//!
//! legacy 是一个 743 行的 if/else 链，这里拆成七个同签名的面板函数：
//! 每个只关心自己那份 config，写回一律走 `onChange(patch)` 的浅合并补丁，
//! 由 core 的 `setNodeConfig` 做不可变替换与历史记录。

import type { ReactElement } from 'react';
import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import { useMessages } from '@/shared/i18n/messages';
import type { FlowNodeKind } from '@/shared/tauri/rules';
import {
  collectionPatch,
  conditionExpression,
  conditionPatch,
  collectionSelector,
  literalText,
  mergeInputValues,
  numberValue,
  parseLiteral,
  recordValue,
  stringArray,
  stringValue,
  type CollectionSelector,
  type ConditionExpressionPatch,
  type JsonObject,
  type MergeInputValue,
} from '../model/node-config';
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

// 选项值是 Rust 侧的枚举变体名，不本地化。
const METHOD_OPTIONS = [
  { value: 'Get', label: 'GET' },
  { value: 'Post', label: 'POST' },
];
const EXPECTED_TYPE_OPTIONS = [
  { value: 'Html', label: 'HTML' },
  { value: 'Xml', label: 'XML' },
  { value: 'Json', label: 'JSON' },
];
const JS_OUTPUT_OPTIONS = [
  { value: 'json', label: 'JSON' },
  { value: 'raw', label: 'RAW' },
];
const EXTRACT_TARGET_OPTIONS = [
  { value: 'Media', label: 'Media' },
  { value: 'Units', label: 'Units' },
  { value: 'Asset', label: 'Asset' },
];
const MAPPER_OUTPUT_OPTIONS = [
  { value: 'items', label: 'Items' },
  { value: 'discovery', label: 'Discovery' },
  { value: 'units', label: 'Units' },
  { value: 'assets', label: 'Assets' },
];
const MERGE_STRATEGY_OPTIONS = [
  { value: 'single_active', label: 'Single active' },
  { value: 'collect_array', label: 'Collect array' },
  { value: 'concat_arrays', label: 'Concat arrays' },
  { value: 'overlay_objects', label: 'Overlay objects' },
];
const ACTIVATION_OPTIONS = [
  { value: 'required', label: 'Required' },
  { value: 'optional', label: 'Optional' },
];
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

/** 这两个操作符是一元的，不需要比较值。 */
const UNARY_OPERATORS = new Set(['exists', 'is_null']);

const MERGE_MIN_INPUTS = 2;
const CONDITION_MIN_BRANCHES = 2;
const LOOP_MAX_ITERATIONS = 256;
const LOOP_DEFAULT_ITERATIONS = 64;

/** 迭代上限由 compiler 限死在 1..256，输入框在这里先夹一次。 */
function clampIterations(raw: string): number {
  const parsed = Number(raw) || LOOP_DEFAULT_ITERATIONS;
  return Math.min(LOOP_MAX_ITERATIONS, Math.max(1, Math.floor(parsed)));
}

function HttpPanel({ config, onChange }: PanelProps) {
  const m = useMessages();
  const headers = Object.entries(recordValue(config.headers));

  return (
    <section className="flex flex-col gap-2" aria-label={m.rules_node_inspector_section_request()}>
      <ConfigField label={m.rules_node_inspector_field_method()}>
        {(id) => (
          <ConfigSelect
            id={id}
            label={m.rules_node_inspector_field_method()}
            value={stringValue(config.method, 'Get')}
            options={METHOD_OPTIONS}
            onChange={(method) => onChange({ method })}
          />
        )}
      </ConfigField>

      <ConfigField label="URL">
        {(id) => (
          <ConfigTextInput
            id={id}
            label="URL"
            className="font-mono"
            value={stringValue(config.url)}
            onChange={(url) => onChange({ url })}
          />
        )}
      </ConfigField>

      <ConfigField label={m.rules_node_inspector_field_expected_type()}>
        {(id) => (
          <ConfigSelect
            id={id}
            label={m.rules_node_inspector_field_expected_type()}
            value={stringValue(config.expected_type, 'Html')}
            options={EXPECTED_TYPE_OPTIONS}
            onChange={(expectedType) => onChange({ expected_type: expectedType })}
          />
        )}
      </ConfigField>

      <ConfigField label={m.rules_node_inspector_field_charset()}>
        {(id) => (
          <ConfigTextInput
            id={id}
            label={m.rules_node_inspector_field_charset()}
            placeholder={m.rules_node_inspector_charset_placeholder()}
            value={stringValue(config.charset)}
            onChange={(charset) => onChange({ charset: charset || null })}
          />
        )}
      </ConfigField>

      <ConfigField label={m.rules_node_inspector_field_body()}>
        {(id) => (
          <Textarea
            id={id}
            className="min-h-20 font-mono text-code"
            value={stringValue(config.body)}
            onChange={(event) => onChange({ body: event.target.value || null })}
          />
        )}
      </ConfigField>

      <PairListEditor
        title={m.rules_node_inspector_field_headers()}
        pairs={headers}
        keyLabel={m.rules_node_inspector_header_name()}
        valueLabel={m.rules_node_inspector_header_value()}
        addLabel={m.rules_node_inspector_header_add()}
        removeLabel={m.rules_node_inspector_header_remove()}
        onChange={(next) =>
          onChange({
            // 空键会覆盖彼此，落盘前丢掉；正在输入的空行仍留在界面上。
            headers: Object.fromEntries(next.filter(([key]) => key.trim().length > 0)),
          })
        }
      />
    </section>
  );
}

function JsPanel({ config, onChange }: PanelProps) {
  const m = useMessages();
  return (
    <section className="flex flex-col gap-2">
      <ConfigField label={m.rules_node_inspector_field_output()}>
        {(id) => (
          <ConfigSelect
            id={id}
            label={m.rules_node_inspector_field_output()}
            value={stringValue(config.output, 'json')}
            options={JS_OUTPUT_OPTIONS}
            onChange={(output) => onChange({ output })}
          />
        )}
      </ConfigField>

      <ConfigField label={m.rules_node_inspector_field_script()}>
        {() => (
          <JsCodeEditor
            aria-label={m.rules_node_inspector_field_script()}
            value={stringValue(config.code)}
            minLines={16}
            onChange={(code) => onChange({ code })}
          />
        )}
      </ConfigField>
    </section>
  );
}

function ExtractPanel({ config, onChange }: PanelProps) {
  const m = useMessages();
  const fieldRules = config.field_rules;
  const fieldGroupCount =
    typeof fieldRules === 'object' && fieldRules !== null && !Array.isArray(fieldRules)
      ? Object.keys(fieldRules).length
      : 0;

  return (
    <section className="flex flex-col gap-3">
      <ConfigField label={m.rules_node_inspector_field_expected_type()}>
        {(id) => (
          <ConfigSelect
            id={id}
            label={m.rules_node_inspector_field_expected_type()}
            value={stringValue(config.expected_type, 'Html')}
            options={EXPECTED_TYPE_OPTIONS}
            onChange={(expectedType) => onChange({ expected_type: expectedType })}
          />
        )}
      </ConfigField>

      <ConfigField label={m.rules_node_inspector_field_output_target()}>
        {(id) => (
          <ConfigSelect
            id={id}
            label={m.rules_node_inspector_field_output_target()}
            value={stringValue(config.output_target, 'Media')}
            options={EXTRACT_TARGET_OPTIONS}
            onChange={(target) => onChange({ output_target: target })}
          />
        )}
      </ConfigField>

      <ExtractRuleList
        rules={Array.isArray(config.rules) ? config.rules : []}
        onChange={(rules) => onChange({ rules })}
      />

      <p className="border border-hairline bg-surface-1 px-2 py-2 text-ui-sm text-ink-subtle">
        {m.rules_node_inspector_field_rules_hint({ count: fieldGroupCount })}
      </p>
    </section>
  );
}

function MapperPanel({ config, onChange }: PanelProps) {
  const m = useMessages();
  return (
    <section className="flex flex-col gap-3">
      <ConfigField label={m.rules_node_inspector_field_output()}>
        {(id) => (
          <ConfigSelect
            id={id}
            label={m.rules_node_inspector_field_output()}
            value={stringValue(config.output, 'items')}
            options={MAPPER_OUTPUT_OPTIONS}
            onChange={(output) => onChange({ output })}
          />
        )}
      </ConfigField>

      <StringListEditor
        title={m.rules_node_inspector_field_identity_fields()}
        values={stringArray(config.identity_fields)}
        itemLabel={m.rules_node_inspector_field_identity_fields()}
        addLabel={m.rules_node_inspector_identity_field_add()}
        removeLabel={m.rules_node_inspector_identity_field_remove()}
        onChange={(identityFields) => onChange({ identity_fields: identityFields })}
      />
    </section>
  );
}

function MergePanel({ config, onChange }: PanelProps) {
  const m = useMessages();
  const inputs = mergeInputValues(config.inputs);

  /** 写回时按数组下标重编 order，保证与显示顺序一致。 */
  const commit = (next: MergeInputValue[]) =>
    onChange({
      inputs: next.map((input, order) => ({
        input_id: input.input_id,
        handle: input.handle,
        order,
        activation: input.activation,
      })),
    });

  const patchInput = (index: number, patch: Partial<MergeInputValue>) =>
    commit(inputs.map((input, i) => (i === index ? { ...input, ...patch } : input)));

  const move = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    if (target < 0 || target >= inputs.length) return;
    const next = [...inputs];
    [next[index], next[target]] = [next[target], next[index]];
    commit(next);
  };

  return (
    <section className="flex flex-col gap-3">
      <ConfigField label={m.rules_node_inspector_field_strategy()}>
        {(id) => (
          <ConfigSelect
            id={id}
            label={m.rules_node_inspector_field_strategy()}
            value={stringValue(config.strategy, 'single_active')}
            options={MERGE_STRATEGY_OPTIONS}
            onChange={(strategy) => onChange({ strategy })}
          />
        )}
      </ConfigField>

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

      {inputs.map((input, index) => (
        // 编辑过程中 input_id 与 handle 都可以临时重复，只有下标是唯一的。
        // eslint-disable-next-line react/no-array-index-key
        <div key={index} className="flex flex-col gap-2 border border-hairline bg-surface-1 p-2">
          <div className="flex items-center gap-1">
            <span className="flex-1 font-mono text-ui-sm text-ink-subtle">order {input.order}</span>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={m.rules_node_inspector_merge_input_up()}
              title={m.rules_node_inspector_merge_input_up()}
              disabled={index === 0}
              onClick={() => move(index, -1)}
            >
              <Icon name="caret-up" />
            </Button>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={m.rules_node_inspector_merge_input_down()}
              title={m.rules_node_inspector_merge_input_down()}
              disabled={index === inputs.length - 1}
              onClick={() => move(index, 1)}
            >
              <Icon name="caret-down" />
            </Button>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={m.rules_node_inspector_merge_input_remove()}
              title={
                inputs.length <= MERGE_MIN_INPUTS
                  ? m.rules_node_inspector_merge_input_min()
                  : m.rules_node_inspector_merge_input_remove()
              }
              disabled={inputs.length <= MERGE_MIN_INPUTS}
              onClick={() => commit(inputs.filter((_, i) => i !== index))}
            >
              <Icon name="trash" className="text-ink-subtle" />
            </Button>
          </div>

          <ConfigTextInput
            label={m.rules_node_inspector_merge_input_id()}
            className="font-mono"
            value={input.input_id}
            onChange={(inputId) => patchInput(index, { input_id: inputId })}
          />
          <ConfigTextInput
            label={m.rules_node_inspector_merge_input_handle()}
            className="font-mono"
            value={input.handle}
            onChange={(handle) => patchInput(index, { handle })}
          />
          <ConfigSelect
            label={m.rules_node_inspector_merge_input_activation()}
            value={input.activation}
            options={ACTIVATION_OPTIONS}
            onChange={(activation) =>
              patchInput(index, { activation: activation as MergeInputValue['activation'] })
            }
          />
        </div>
      ))}
    </section>
  );
}

function ConditionPanel({ config, onChange }: PanelProps) {
  const m = useMessages();
  const expression = conditionExpression(config);
  const branches = stringArray(config.branches);
  const branchOptions = branches.map((branch) => ({ value: branch, label: branch }));

  const patchCondition = (patch: ConditionExpressionPatch) =>
    onChange(conditionPatch(config, patch));

  /** 改分支列表要连带修正 true/false 指向，否则会指到已删除的 handle。 */
  const setBranches = (next: string[]) => {
    const valid = next.map((branch) => branch.trim()).filter(Boolean);
    if (valid.length < CONDITION_MIN_BRANCHES) return;
    patchCondition({ branches: valid });
  };

  const patchPredicate = (patch: Record<string, unknown>) => {
    if (expression.mode !== 'typed') return;
    patchCondition({
      mode: 'typed',
      predicate: { ...expression.predicate, ...patch },
      true_branch: expression.true_branch,
      false_branch: expression.false_branch,
    });
  };

  const setMode = (mode: 'typed' | 'js') => {
    if (mode === 'js') {
      onChange({
        expression: { mode: 'js', code: expression.mode === 'js' ? expression.code : '' },
      });
      return;
    }
    patchCondition(
      expression.mode === 'typed'
        ? expression
        : {
            mode: 'typed',
            predicate: { operator: 'exists', pointer: '' },
            true_branch: branches[0] ?? 'true',
            false_branch: branches[1] ?? 'false',
          },
    );
  };

  return (
    <section className="flex flex-col gap-3">
      <ModeToggle
        label={m.rules_node_inspector_condition_mode()}
        value={expression.mode}
        onChange={setMode}
        typedLabel={m.rules_node_inspector_mode_typed()}
        jsLabel={m.rules_node_inspector_mode_js()}
      />

      <StringListEditor
        title={m.rules_node_inspector_field_branches()}
        values={branches}
        itemLabel={m.rules_node_inspector_branch_handle()}
        addLabel={m.rules_node_inspector_branch_add()}
        removeLabel={m.rules_node_inspector_branch_remove()}
        minLength={CONDITION_MIN_BRANCHES}
        onChange={setBranches}
        makeNewValue={(count) => `branch_${count + 1}`}
      />

      {expression.mode === 'typed' ? (
        <>
          <ConfigField label={m.rules_node_inspector_field_operator()}>
            {(id) => (
              <ConfigSelect
                id={id}
                label={m.rules_node_inspector_field_operator()}
                value={expression.predicate.operator}
                options={OPERATOR_OPTIONS}
                onChange={(operator) => patchPredicate({ operator })}
              />
            )}
          </ConfigField>

          <ConfigField label={m.rules_node_inspector_field_pointer()}>
            {(id) => (
              <ConfigTextInput
                id={id}
                label={m.rules_node_inspector_field_pointer()}
                className="font-mono"
                value={expression.predicate.pointer}
                onChange={(pointer) => patchPredicate({ pointer })}
              />
            )}
          </ConfigField>

          {UNARY_OPERATORS.has(expression.predicate.operator) ? null : (
            <ConfigField label={m.rules_node_inspector_field_value()}>
              {(id) => (
                <ConfigTextInput
                  id={id}
                  label={m.rules_node_inspector_field_value()}
                  className="font-mono"
                  value={literalText(expression.predicate.value)}
                  onChange={(value) => patchPredicate({ value: parseLiteral(value) })}
                />
              )}
            </ConfigField>
          )}

          <div className="grid grid-cols-2 gap-2">
            <ConfigField label={m.rules_node_inspector_true_branch()}>
              {(id) => (
                <ConfigSelect
                  id={id}
                  label={m.rules_node_inspector_true_branch()}
                  value={expression.true_branch}
                  options={branchOptions}
                  onChange={(value) =>
                    patchCondition({
                      mode: 'typed',
                      predicate: expression.predicate,
                      true_branch: value,
                      false_branch: expression.false_branch,
                    })
                  }
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
                  onChange={(value) =>
                    patchCondition({
                      mode: 'typed',
                      predicate: expression.predicate,
                      true_branch: expression.true_branch,
                      false_branch: value,
                    })
                  }
                />
              )}
            </ConfigField>
          </div>
        </>
      ) : (
        <ConfigField label={m.rules_node_inspector_field_expression()}>
          {() => (
            <JsCodeEditor
              aria-label={m.rules_node_inspector_field_expression()}
              value={expression.code}
              minLines={12}
              onChange={(code) => onChange({ expression: { mode: 'js', code } })}
            />
          )}
        </ConfigField>
      )}
    </section>
  );
}

function LoopPanel({ config, onChange }: PanelProps) {
  const m = useMessages();
  const selector = collectionSelector(config);

  const setMode = (mode: 'typed' | 'js') => {
    const next: CollectionSelector =
      mode === 'js'
        ? { mode: 'js', code: selector.mode === 'js' ? selector.code : '' }
        : { mode: 'typed', pointer: selector.mode === 'typed' ? selector.pointer : '' };
    onChange(collectionPatch(config, next));
  };

  return (
    <section className="flex flex-col gap-3">
      <ModeToggle
        label={m.rules_node_inspector_collection_mode()}
        value={selector.mode}
        onChange={setMode}
        typedLabel={m.rules_node_inspector_mode_typed()}
        jsLabel={m.rules_node_inspector_mode_js()}
      />

      <ConfigField label={m.rules_node_inspector_field_collection()}>
        {(id) =>
          selector.mode === 'typed' ? (
            <ConfigTextInput
              id={id}
              label={m.rules_node_inspector_field_collection()}
              className="font-mono"
              value={selector.pointer}
              onChange={(pointer) => onChange({ collection: { mode: 'typed', pointer } })}
            />
          ) : (
            <JsCodeEditor
              aria-label={m.rules_node_inspector_field_collection()}
              value={selector.code}
              minLines={10}
              onChange={(code) => onChange({ collection: { mode: 'js', code } })}
            />
          )
        }
      </ConfigField>

      <div className="grid grid-cols-2 gap-2">
        <ConfigField label={m.rules_node_inspector_field_item_binding()}>
          {(id) => (
            <ConfigTextInput
              id={id}
              label={m.rules_node_inspector_field_item_binding()}
              className="font-mono"
              value={stringValue(config.item_binding, 'item')}
              onChange={(binding) => onChange({ item_binding: binding })}
            />
          )}
        </ConfigField>
        <ConfigField label={m.rules_node_inspector_field_index_binding()}>
          {(id) => (
            <ConfigTextInput
              id={id}
              label={m.rules_node_inspector_field_index_binding()}
              className="font-mono"
              value={stringValue(config.index_binding, 'index')}
              onChange={(binding) => onChange({ index_binding: binding })}
            />
          )}
        </ConfigField>
      </div>

      <ConfigField
        label={`${m.rules_node_inspector_field_max_iterations()} (1-${LOOP_MAX_ITERATIONS})`}
      >
        {(id) => (
          <ConfigTextInput
            id={id}
            label={m.rules_node_inspector_field_max_iterations()}
            type="number"
            value={numberValue(config.max_iterations, LOOP_DEFAULT_ITERATIONS)}
            onChange={(value) => onChange({ max_iterations: clampIterations(value) })}
          />
        )}
      </ConfigField>
    </section>
  );
}

const PANELS: Record<FlowNodeKind, (props: PanelProps) => ReactElement> = {
  http: HttpPanel,
  js: JsPanel,
  extract: ExtractPanel,
  mapper: MapperPanel,
  merge: MergePanel,
  condition: ConditionPanel,
  loop: LoopPanel,
};

export function NodeConfigPanel({ kind, config, onChange }: PanelProps & { kind: FlowNodeKind }) {
  const Panel = PANELS[kind];
  return <Panel config={config} onChange={onChange} />;
}
