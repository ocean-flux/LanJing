//! 检查器里复用的字段控件。
//!
//! 面板只描述「哪些字段、什么类型」，控件的密度、标签与无障碍名统一在这里。
//! Select 走 NativeSelect 而不是弹层 Select：检查器一屏十几个选择器，
//! 原生控件更紧凑、键盘可达、不产生 portal。

import { useId, useState, type ReactNode } from 'react';
import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select';
import { cn } from '@/shared/utils';

export type SelectOption = { value: string; label: string };

/** 竖排字段：标签在上、控件在下。 */
export function ConfigField({
  label,
  children,
  className,
}: {
  label: string;
  children: (id: string) => ReactNode;
  className?: string;
}) {
  const id = useId();
  return (
    <div className={cn('flex flex-col gap-1', className)}>
      <label htmlFor={id} className="text-ui-sm text-ink-muted">
        {label}
      </label>
      {children(id)}
    </div>
  );
}

export function ConfigSelect({
  id,
  label,
  value,
  options,
  onChange,
  className,
}: {
  id?: string;
  label: string;
  value: string;
  options: readonly SelectOption[];
  onChange: (value: string) => void;
  className?: string;
}) {
  // 当前值不在选项里时补一个，避免 <select> 静默落到第一项而错报配置。
  const known = options.some((option) => option.value === value);
  return (
    <NativeSelect
      id={id}
      size="sm"
      aria-label={label}
      className={cn('w-full', className)}
      value={value}
      onChange={(event) => onChange(event.target.value)}
    >
      {known ? null : <NativeSelectOption value={value}>{value}</NativeSelectOption>}
      {options.map((option) => (
        <NativeSelectOption key={option.value} value={option.value}>
          {option.label}
        </NativeSelectOption>
      ))}
    </NativeSelect>
  );
}

export function ConfigTextInput({
  id,
  label,
  value,
  onChange,
  placeholder,
  type,
  className,
}: {
  id?: string;
  label: string;
  value: string | number;
  onChange: (value: string) => void;
  placeholder?: string;
  type?: 'text' | 'number';
  className?: string;
}) {
  return (
    <Input
      id={id}
      aria-label={label}
      type={type}
      placeholder={placeholder}
      className={cn('h-(--density-control-sm) text-ui-sm', className)}
      value={value}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}

/** 小节标题 + 右侧「添加」按钮。 */
export function ConfigListHeader({
  title,
  addLabel,
  onAdd,
}: {
  title: string;
  addLabel: string;
  onAdd: () => void;
}) {
  return (
    <div className="flex items-center justify-between gap-2">
      <p className="text-ui-sm font-medium text-ink-muted">{title}</p>
      <Button variant="ghost" size="icon-xs" aria-label={addLabel} title={addLabel} onClick={onAdd}>
        <Icon name="plus" />
      </Button>
    </div>
  );
}

/** 字符串列表编辑器（身份字段、条件分支）。 */
export function StringListEditor({
  title,
  values,
  itemLabel,
  addLabel,
  removeLabel,
  minLength = 0,
  onChange,
  makeNewValue,
}: {
  title: string;
  values: readonly string[];
  itemLabel: string;
  addLabel: string;
  removeLabel: string;
  /** 少于该长度时禁止删除。 */
  minLength?: number;
  onChange: (next: string[]) => void;
  makeNewValue?: (count: number) => string;
}) {
  return (
    <div className="flex flex-col gap-2">
      <ConfigListHeader
        title={title}
        addLabel={addLabel}
        onAdd={() => onChange([...values, makeNewValue?.(values.length) ?? ''])}
      />
      {values.map((value, index) => (
        // 列表项没有稳定 id，值本身可重复也可为空，下标是这里唯一可用的 key。
        // eslint-disable-next-line react/no-array-index-key
        <div key={index} className="flex gap-1.5">
          <ConfigTextInput
            label={itemLabel}
            className="min-w-0 flex-1"
            value={value}
            onChange={(next) => onChange(values.map((item, i) => (i === index ? next : item)))}
          />
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={removeLabel}
            title={removeLabel}
            disabled={values.length <= minLength}
            onClick={() => onChange(values.filter((_, i) => i !== index))}
          >
            <Icon name="trash" className="text-ink-subtle" />
          </Button>
        </div>
      ))}
    </div>
  );
}

/**
 * 键值对编辑器（HTTP headers）。
 *
 * headers 在 config 里是 Record，空键无法表示，所以「新增」得到的空行只能
 * 先留在组件本地，等键名填上再提交上去。legacy 直接提交空行然后被过滤掉，
 * 结果是加号按钮点了没反应。
 */
export function PairListEditor({
  title,
  pairs,
  keyLabel,
  valueLabel,
  addLabel,
  removeLabel,
  onChange,
}: {
  title: string;
  pairs: readonly (readonly [string, string])[];
  keyLabel: string;
  valueLabel: string;
  addLabel: string;
  removeLabel: string;
  onChange: (next: [string, string][]) => void;
}) {
  /** 尚未提交的空键行；键一填上就并进 pairs 并从这里移除。 */
  const [drafts, setDrafts] = useState<[string, string][]>([]);

  const committed = pairs.map(([key, value]): [string, string] => [key, value]);

  const editCommitted = (index: number, pair: [string, string]) =>
    onChange(committed.map((item, i) => (i === index ? pair : item)));

  const editDraft = (index: number, pair: [string, string]) => {
    if (pair[0].trim().length === 0) {
      setDrafts(drafts.map((item, i) => (i === index ? pair : item)));
      return;
    }
    setDrafts(drafts.filter((_, i) => i !== index));
    onChange([...committed, pair]);
  };

  const rows: [string, string][] = [...committed, ...drafts];

  return (
    <div className="flex flex-col gap-2">
      <ConfigListHeader
        title={title}
        addLabel={addLabel}
        onAdd={() => setDrafts([...drafts, ['', '']])}
      />
      {rows.map(([key, value], index) => {
        const isDraft = index >= committed.length;
        const draftIndex = index - committed.length;
        return (
          // 键可以为空也可以临时重复（正在输入），下标是这里唯一稳定的 key。
          // eslint-disable-next-line react/no-array-index-key
          <div key={index} className="flex gap-1.5">
            <ConfigTextInput
              label={keyLabel}
              className="min-w-0 flex-1"
              value={key}
              onChange={(next) =>
                isDraft ? editDraft(draftIndex, [next, value]) : editCommitted(index, [next, value])
              }
            />
            <ConfigTextInput
              label={valueLabel}
              className="min-w-0 flex-1"
              value={value}
              onChange={(next) =>
                isDraft ? editDraft(draftIndex, [key, next]) : editCommitted(index, [key, next])
              }
            />
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={removeLabel}
              title={removeLabel}
              onClick={() => {
                if (isDraft) {
                  setDrafts(drafts.filter((_, i) => i !== draftIndex));
                  return;
                }
                onChange(committed.filter((_, i) => i !== index));
              }}
            >
              <Icon name="trash" className="text-ink-subtle" />
            </Button>
          </div>
        );
      })}
    </div>
  );
}

/** Typed / JS 两态切换。 */
export function ModeToggle({
  label,
  value,
  onChange,
  typedLabel,
  jsLabel,
}: {
  label: string;
  value: 'typed' | 'js';
  onChange: (mode: 'typed' | 'js') => void;
  typedLabel: string;
  jsLabel: string;
}) {
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className="flex items-center gap-1 border border-hairline p-1"
    >
      {(
        [
          ['typed', typedLabel],
          ['js', jsLabel],
        ] as const
      ).map(([mode, text]) => (
        <button
          key={mode}
          type="button"
          role="radio"
          aria-checked={value === mode}
          className={cn(
            'flex h-(--density-control-sm) min-w-0 flex-1 items-center justify-center text-ui-sm',
            'focus-visible:outline focus-visible:outline-2 focus-visible:outline-ring',
            value === mode ? 'bg-surface-2 text-ink' : 'text-ink-muted hover:text-ink',
          )}
          onClick={() => onChange(mode)}
        >
          {text}
        </button>
      ))}
    </div>
  );
}
