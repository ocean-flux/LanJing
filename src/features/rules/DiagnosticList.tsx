//! 诊断栏：校验状态 + 当前诊断列表。
//!
//! 诊断是 compiler 的稳定 code + 已本地化 message（Rust 侧给出），
//! 这里只负责排序、分级配色与点击定位。

import { useMemo } from 'react';
import { Icon } from '@/components/Icon';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/shared/utils';
import { useMessages } from '@/shared/i18n/messages';
import type { InstallDiagnostic } from '@/shared/tauri/rules';
import { executionModeLabel, executionStatusClass, executionStatusLabel } from './labels';
import type { ValidationState } from './model/core';
import { selectExecutionRun } from './model/session';
import { useRuleEditorSession, useRuleEditorSessionStore } from './use-session';

type Severity = InstallDiagnostic['severity'];

/** 错误在前、警告其次、提示最后。 */
const SEVERITY_ORDER: Record<Severity, number> = { error: 0, warning: 1, info: 2 };

function severityClass(severity: Severity): string {
  return severity === 'error'
    ? 'text-danger'
    : severity === 'warning'
      ? 'text-warning'
      : 'text-ink-muted';
}

/** 诊断没有稳定 id，用 code + span + message 合成；完全一致的两条本就是同一件事。 */
function diagnosticKey(diagnostic: InstallDiagnostic): string {
  const { span } = diagnostic;
  return `${diagnostic.code}@${span?.path ?? ''}:${span?.start ?? ''}:${diagnostic.message}`;
}

function statusLabel(m: ReturnType<typeof useMessages>, status: ValidationState['status']) {
  switch (status) {
    case 'unknown': {
      return m.rules_validation_unknown();
    }
    case 'pending': {
      return m.rules_validation_pending();
    }
    case 'valid': {
      return m.rules_validation_valid();
    }
    case 'invalid': {
      return m.rules_validation_invalid();
    }
    case 'stale': {
      return m.rules_validation_stale();
    }
    case 'error': {
      return m.rules_validation_error();
    }
  }
}

export function DiagnosticList() {
  const m = useMessages();
  const store = useRuleEditorSessionStore();
  const validation = useRuleEditorSession((state) => state.core.validation);
  const execution = useRuleEditorSession(selectExecutionRun);

  const sorted = useMemo(
    () =>
      [...validation.diagnostics].sort(
        (left, right) => SEVERITY_ORDER[left.severity] - SEVERITY_ORDER[right.severity],
      ),
    [validation.diagnostics],
  );

  const counts = useMemo(() => {
    const result = { error: 0, warning: 0, info: 0 };
    for (const diagnostic of validation.diagnostics) result[diagnostic.severity] += 1;
    return result;
  }, [validation.diagnostics]);

  return (
    <section
      aria-label={m.rules_diagnostics()}
      className="flex max-h-52 shrink-0 flex-col border-t border-hairline bg-surface-1"
    >
      <header className="flex h-(--density-row) shrink-0 items-center gap-2 border-b border-hairline px-(--page-gutter)">
        <h2 className="font-medium">{m.rules_diagnostics()}</h2>
        <Badge variant={validation.status === 'valid' ? 'secondary' : 'outline'}>
          {statusLabel(m, validation.status)}
        </Badge>
        {counts.error > 0 ? (
          <span className="text-ui-sm text-danger">
            {m.rules_diagnostics_error({ count: counts.error })}
          </span>
        ) : null}
        {counts.warning > 0 ? (
          <span className="text-ui-sm text-warning">
            {m.rules_diagnostics_warning({ count: counts.warning })}
          </span>
        ) : null}
        {typeof validation.revision === 'number' ? (
          <span className="ml-auto shrink-0 font-mono text-ui-sm text-ink-subtle">
            rev {validation.revision}
          </span>
        ) : null}
      </header>

      {sorted.length === 0 ? (
        <p className="px-(--page-gutter) py-2 text-ui-sm text-ink-subtle">
          {m.rules_diagnostics_empty()}
        </p>
      ) : (
        <ul className="min-h-0 flex-1 overflow-y-auto">
          {sorted.map((diagnostic) => (
            <DiagnosticRow
              key={diagnosticKey(diagnostic)}
              diagnostic={diagnostic}
              onReveal={(nodeId) => store.getState().revealNode(nodeId)}
            />
          ))}
        </ul>
      )}

      {/* 运行诊断与校验诊断分组展示：校验状态只是「文档能不能编译」，
          一次运行失败不代表文档无效，混在同一列表里会让人误读状态徽章。 */}
      {execution.diagnostics.length === 0 ? null : (
        <>
          <header className="flex h-(--density-row) shrink-0 items-center gap-2 border-y border-hairline px-(--page-gutter)">
            <h3 className="text-ui-sm font-medium text-ink-muted">
              {m.rules_execution_diagnostics()}
            </h3>
            {/* 运行诊断可能属于一次重放：分组标题处就标出模式与结局，
                不让一段重放的失败看起来像一次实时运行的失败。 */}
            <span className={cn('text-ui-sm', executionStatusClass(execution.status))}>
              {executionModeLabel(m, execution.mode)} · {executionStatusLabel(m, execution.status)}
            </span>
          </header>
          <ul className="max-h-32 shrink-0 overflow-y-auto">
            {execution.diagnostics.map((diagnostic, index) => (
              <DiagnosticRow
                key={`run:${String(index)}:${diagnosticKey(diagnostic)}`}
                diagnostic={diagnostic}
                onReveal={(nodeId) => store.getState().revealNode(nodeId)}
              />
            ))}
          </ul>
        </>
      )}
    </section>
  );
}

/** 一行诊断：稳定 code + 已本地化 message，点击把画布带到诊断指向的节点。 */
function DiagnosticRow({
  diagnostic,
  onReveal,
}: {
  diagnostic: InstallDiagnostic;
  onReveal: (nodeId: string) => void;
}) {
  return (
    <li>
      <button
        type="button"
        className="flex w-full items-start gap-2 px-(--page-gutter) py-1.5 text-left text-ui-sm hover:bg-surface-2 focus-visible:bg-surface-2 focus-visible:outline-none"
        onClick={() => {
          // 诊断的 span.path 目前对应节点 id，用于把画布焦点带过去。
          const nodeId = diagnostic.span?.path;
          if (nodeId) onReveal(nodeId);
        }}
      >
        <Icon
          name={diagnostic.severity === 'info' ? 'file-text' : 'warning-circle'}
          className={cn('mt-0.5 shrink-0', severityClass(diagnostic.severity))}
        />
        <span className="min-w-0 flex-1">{diagnostic.message}</span>
        <span className="shrink-0 font-mono text-[10px] text-ink-subtle">{diagnostic.code}</span>
      </button>
    </li>
  );
}
