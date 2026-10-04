//! 规则预览运行：把当前规则按选定的标准意图跑一次，并给出运行态、捕获与取消入口。
//!
//! 目标来源、意图与输入都由作者显式选择：面板不猜节点语义，也不重新解释诊断 ——
//! 事件折叠在 session / `model/execution`，诊断 code 与 message 原样透传。
//!
//! 「预览运行」是正在编辑的这条规则的能力，不是某个节点的专属行为：面板同样挂在
//! 编辑器工具栏上；从节点检查器进来时，本次运行由那个节点发起、诊断回落到同一诊断栏。
//!
//! 重放走同一条折叠路径，只有请求模式不同。界面上重放有独立标识：它只读固定历史
//! archive，绝不因为重放失败就改打一次实时请求，作者看到的失败也必须是重放自己的结局。

import { useEffect, useMemo, useState } from 'react';
import { Icon } from '@/components/Icon';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/ui/spinner';
import { useMessages } from '@/shared/i18n/messages';
import type { IntentInput } from '@/shared/tauri/execution';
import {
  listInstalledSources,
  type InstalledSource,
  type StandardIntent,
} from '@/shared/tauri/sources';
import {
  executionModeLabel,
  executionStatusClass,
  executionStatusLabel,
  replayFailureText,
} from '../labels';
import {
  replayFailureReasonOf,
  type ExecutionCapture,
  type ExecutionRunState,
} from '../model/execution';
import { selectExecutionRun, selectReplaySource } from '../model/session';
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

/** 文档声明里的来源身份；wire 上可能是 `{ id }` 或裸字符串。 */
function sourceIdentityId(identity: { id: string } | string): string {
  return typeof identity === 'string' ? identity : identity.id;
}

/**
 * 运行结局：状态 + 稳定 code，重放失败额外给出归类文案。
 *
 * 重放失败的归类文案说的是「这段重放缺什么」，与同一 code 出现在实时运行里时不同：
 * 只有 `mode === 'replay'` 的运行才有可解释的重放历史。
 */
function RunOutcome({ run }: { run: ExecutionRunState }) {
  const m = useMessages();
  const reason = replayFailureReasonOf(run);

  return (
    <>
      <p className={`text-ui-sm ${executionStatusClass(run.status)}`}>
        {executionStatusLabel(m, run.status)}
        {run.failureCode === null ? null : (
          <span className="ml-2 font-mono">
            {m.rules_execution_failure_code({ code: run.failureCode })}
          </span>
        )}
      </p>
      {reason === null ? null : (
        <p className="text-ui-sm text-danger">{replayFailureText(m, reason)}</p>
      )}
    </>
  );
}

/** 一次 live 运行的捕获清单：作者据此确认这次跑真的拿到了哪些 effect。 */
function CaptureList({ captures }: { captures: ExecutionCapture[] }) {
  const m = useMessages();

  return (
    <div aria-label={m.rules_execution_captures_title()} className="flex flex-col gap-1">
      <h4 className="flex items-center gap-2 text-ui-sm font-medium">
        <Icon name="push-pin" />
        <span>{m.rules_execution_captures_title()}</span>
        {captures.length === 0 ? null : (
          <span className="text-ink-subtle">
            {m.rules_execution_captures_count({ count: captures.length })}
          </span>
        )}
      </h4>
      {captures.length === 0 ? (
        <p className="text-ui-sm text-ink-subtle">{m.rules_execution_captures_empty()}</p>
      ) : (
        <ul className="flex flex-col gap-1">
          {captures.map((capture) => (
            <li key={capture.effectId} className="flex items-center gap-2 text-ui-sm">
              <code className="min-w-0 shrink truncate font-mono text-ink-muted">
                {capture.effectId}
              </code>
              <code className="min-w-0 flex-1 truncate font-mono text-ink-subtle">
                {capture.outputHash}
              </code>
              <span className="shrink-0 text-ink-subtle">
                {m.rules_execution_capture_artifacts({ count: capture.artifactCount })}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export function ExecutionPreview() {
  const m = useMessages();
  const store = useRuleEditorSessionStore();
  const run = useRuleEditorSession(selectExecutionRun);
  const replaySource = useRuleEditorSession(selectReplaySource);
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
  const isRunning = run.status === 'running';
  const canRun = effectiveSourceId !== null && effectiveIntent !== null && !isRunning;
  const canReplay = replaySource !== null && !isRunning;

  /** 两个运行入口共用同一组目标与输入；重放只多带一个历史 execution id。 */
  const request = () => {
    if (effectiveSourceId === null || effectiveIntent === null) return null;
    return {
      sourceId: effectiveSourceId,
      intent: effectiveIntent,
      input: toIntentInput(inputKind, inputValue),
    };
  };

  return (
    <section
      aria-label={m.rules_execution_title()}
      className="flex flex-col gap-3 border-t border-hairline pt-3"
    >
      <h3 className="flex items-center gap-2 text-ui-sm font-medium">
        <Icon name="play" />
        <span>{m.rules_execution_title()}</span>
        {run.status === 'idle' ? null : (
          <Badge variant="outline" className={run.mode === 'replay' ? 'text-warning' : undefined}>
            {executionModeLabel(m, run.mode)}
          </Badge>
        )}
        {run.replayOf === null ? null : (
          <code className="min-w-0 truncate font-mono text-ui-sm text-ink-subtle">
            {m.rules_execution_replay_of({ executionId: run.replayOf })}
          </code>
        )}
      </h3>
      <p className="text-ui-sm leading-4 text-ink-subtle">{m.rules_execution_hint()}</p>
      {run.mode === 'replay' ? (
        <p className="text-ui-sm leading-4 text-warning">
          {m.rules_execution_replay_no_fallback()}
        </p>
      ) : null}

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

      <div className="flex flex-wrap items-center gap-2">
        <Button
          size="sm"
          disabled={!canRun}
          onClick={() => {
            const next = request();
            if (next) void store.getState().startPreviewRun(next);
          }}
        >
          {isRunning ? <Spinner /> : <Icon name="play" />}
          <span>{m.rules_execution_run()}</span>
        </Button>
        {isRunning ? (
          <Button
            variant="outline"
            size="sm"
            onClick={() => void store.getState().cancelPreviewRun()}
          >
            <Icon name="stop-circle" />
            <span>{m.rules_execution_cancel()}</span>
          </Button>
        ) : null}
        <Button
          variant="outline"
          size="sm"
          disabled={!canReplay}
          onClick={() => {
            const next = request();
            if (next && replaySource !== null) {
              void store.getState().startReplayRun({ ...next, archivedExecutionId: replaySource });
            }
          }}
        >
          <Icon name="arrow-counter-clockwise" />
          <span>{m.rules_execution_replay_start()}</span>
        </Button>
      </div>

      {replaySource === null && run.status !== 'idle' && !isRunning ? (
        <p className="text-ui-sm text-ink-subtle">{m.rules_execution_replay_unavailable()}</p>
      ) : null}

      <RunOutcome run={run} />

      {/* 捕获是 live 运行的事实；重放不产生新捕获，因此不给一份会误读成「零捕获」的清单。 */}
      {run.mode === 'replay' ? (
        <p className="text-ui-sm leading-4 text-ink-subtle">
          {m.rules_execution_replay_captures_note()}
        </p>
      ) : (
        <CaptureList captures={run.captures} />
      )}
    </section>
  );
}
