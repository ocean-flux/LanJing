//! 节点预览运行：把当前规则按选定的标准意图跑一次，并给出运行态与取消入口。
//!
//! 目标来源、意图与输入都由作者显式选择：面板不猜节点语义，也不重新解释诊断 ——
//! 事件折叠在 session / `model/execution`，诊断 code 与 message 原样透传。
//!
//! 「节点预览」= 本次运行由当前节点发起、诊断回落到同一诊断栏；`execute` 只跑已安装
//! 来源的已编译 Plan，所以面板如实说明运行的是已安装版本。

import { useEffect, useMemo, useState } from 'react';
import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/ui/spinner';
import { useMessages } from '@/shared/i18n/messages';
import type { IntentInput } from '@/shared/tauri/execution';
import {
  listInstalledSources,
  type InstalledSource,
  type StandardIntent,
} from '@/shared/tauri/sources';
import type { ExecutionRunState } from '../model/execution';
import { selectExecutionRun } from '../model/session';
import { useRuleEditorSession, useRuleEditorSessionStore } from '../use-session';
import { ConfigField, ConfigSelect, ConfigTextInput } from './fields';

/** 输入类型：wire 上的 `IntentInput` 变体名（tag 使用 Rust 变体名，不能自造小写）。 */
const INPUT_KINDS = ['Query', 'ItemId', 'UnitId', 'ActionId', 'Page', 'Opaque', 'None'] as const;

type InputKind = (typeof INPUT_KINDS)[number];

/** 把输入类型 + 文本组成 wire 形态的 `IntentInput`。 */
function toIntentInput(kind: InputKind, value: string): IntentInput {
  switch (kind) {
    case 'None': {
      return { type: 'None' };
    }
    case 'Opaque': {
      // Opaque 载荷是透明 JSON：能解析就用解析值，否则原样当字符串（也是合法 JSON 值）。
      try {
        return { type: 'Opaque', value: JSON.parse(value) as unknown };
      } catch {
        return { type: 'Opaque', value };
      }
    }
    case 'Query':
    case 'ItemId':
    case 'UnitId':
    case 'ActionId':
    case 'Page': {
      return { type: kind, value };
    }
  }
}

function statusLabel(m: ReturnType<typeof useMessages>, run: ExecutionRunState): string {
  switch (run.status) {
    case 'idle': {
      return m.rules_execution_status_idle();
    }
    case 'running': {
      return m.rules_execution_status_running();
    }
    case 'succeeded': {
      return m.rules_execution_status_succeeded();
    }
    case 'failed': {
      return m.rules_execution_status_failed();
    }
    case 'cancelled': {
      return m.rules_execution_status_cancelled();
    }
  }
}

/** 文档声明里的来源身份；wire 上可能是 `{ id }` 或裸字符串。 */
function sourceIdentityId(identity: { id: string } | string): string {
  return typeof identity === 'string' ? identity : identity.id;
}

export function ExecutionPreview() {
  const m = useMessages();
  const store = useRuleEditorSessionStore();
  const run = useRuleEditorSession(selectExecutionRun);
  const definition = useRuleEditorSession((state) => state.core.definition);
  const [sources, setSources] = useState<InstalledSource[] | null>(null);
  const [chosenSourceId, setChosenSourceId] = useState<string | null>(null);
  const [chosenIntent, setChosenIntent] = useState<StandardIntent | null>(null);
  const [inputKind, setInputKind] = useState<InputKind>('Query');
  const [inputValue, setInputValue] = useState('');

  useEffect(() => {
    let cancelled = false;
    void listInstalledSources()
      .then((installed) => {
        if (!cancelled) setSources(installed);
      })
      // 读路径失败退化为「没有已安装来源」，不把面板炸掉。
      .catch(() => {
        if (!cancelled) setSources([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const intents = useMemo(
    () => Object.keys(definition.intent_exports) as StandardIntent[],
    [definition.intent_exports],
  );

  // 默认选中文档来源对应的已安装来源与规则声明的第一个意图；作者的显式选择优先。
  const effectiveSourceId =
    chosenSourceId ??
    sources?.find((source) => source.source_id === sourceIdentityId(definition.source_identity))
      ?.source_id ??
    sources?.[0]?.source_id ??
    null;
  const effectiveIntent = chosenIntent ?? intents[0] ?? null;

  const sourceLabel = m.rules_execution_source();
  const intentLabel = m.rules_execution_intent();
  const inputKindLabel = m.rules_execution_input_kind();
  const inputValueLabel = m.rules_execution_input_value();
  const canRun = effectiveSourceId !== null && effectiveIntent !== null && run.status !== 'running';

  return (
    <section
      aria-label={m.rules_execution_title()}
      className="flex flex-col gap-3 border-t border-hairline pt-3"
    >
      <h3 className="flex items-center gap-2 text-ui-sm font-medium">
        <Icon name="play" />
        <span>{m.rules_execution_title()}</span>
      </h3>
      <p className="text-ui-sm leading-4 text-ink-subtle">{m.rules_execution_hint()}</p>

      {sources !== null && sources.length === 0 ? (
        <p className="text-ui-sm text-warning">{m.rules_execution_no_source()}</p>
      ) : null}
      {sources !== null && sources.length > 0 ? (
        <ConfigField label={sourceLabel}>
          {(id) => (
            <ConfigSelect
              id={id}
              label={sourceLabel}
              value={effectiveSourceId ?? ''}
              options={sources.map((source) => ({
                value: source.source_id,
                label: `${source.profile.title} · ${source.version}`,
              }))}
              onChange={setChosenSourceId}
            />
          )}
        </ConfigField>
      ) : null}

      {intents.length === 0 ? (
        <p className="text-ui-sm text-warning">{m.rules_execution_no_intent()}</p>
      ) : (
        <ConfigField label={intentLabel}>
          {(id) => (
            <ConfigSelect
              id={id}
              label={intentLabel}
              value={effectiveIntent ?? ''}
              options={intents.map((intent) => ({ value: intent, label: intent }))}
              onChange={(next) => setChosenIntent(next as StandardIntent)}
            />
          )}
        </ConfigField>
      )}

      <ConfigField label={inputKindLabel}>
        {(id) => (
          <ConfigSelect
            id={id}
            label={inputKindLabel}
            value={inputKind}
            options={INPUT_KINDS.map((kind) => ({ value: kind, label: kind }))}
            onChange={(next) => setInputKind(next as InputKind)}
          />
        )}
      </ConfigField>
      {inputKind === 'None' ? null : (
        <ConfigField label={inputValueLabel}>
          {(id) => (
            <ConfigTextInput
              id={id}
              label={inputValueLabel}
              value={inputValue}
              onChange={setInputValue}
            />
          )}
        </ConfigField>
      )}

      <div className="flex items-center gap-2">
        <Button
          size="sm"
          disabled={!canRun}
          onClick={() => {
            if (effectiveSourceId === null || effectiveIntent === null) return;
            void store.getState().startPreviewRun({
              sourceId: effectiveSourceId,
              intent: effectiveIntent,
              input: toIntentInput(inputKind, inputValue),
            });
          }}
        >
          {run.status === 'running' ? <Spinner /> : <Icon name="play" />}
          <span>{m.rules_execution_run()}</span>
        </Button>
        {run.status === 'running' ? (
          <Button
            variant="outline"
            size="sm"
            onClick={() => void store.getState().cancelPreviewRun()}
          >
            <Icon name="stop-circle" />
            <span>{m.rules_execution_cancel()}</span>
          </Button>
        ) : null}
      </div>

      <p
        className={run.status === 'failed' ? 'text-ui-sm text-danger' : 'text-ui-sm text-ink-muted'}
      >
        {statusLabel(m, run)}
        {run.failureCode === null ? null : (
          <span className="ml-2 font-mono">
            {m.rules_execution_failure_code({ code: run.failureCode })}
          </span>
        )}
      </p>
    </section>
  );
}
