//! execution wire 测试。
//!
//! 覆盖：3 个 invoke wrapper 的 command 名与 `{ request }` wrapper 约定、
//! DTO 字段名与 Rust 合同（snake_case / 变体 tag）逐字段 parity、
//! 事件形状校验与终态判定。

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  RULE_EXECUTION_EVENT,
  cancelExecution,
  catchUpExecution,
  executeRule,
  executionFailureDiagnostics,
  isExecutionEvent,
  isExecutionEventKind,
  isTerminalExecutionEvent,
  listenRuleExecutionEvents,
  type ExecutionEvent,
} from './execution';
import type { RuleErrorWire } from '@/shared/tauri/rules';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn<typeof invoke>(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn<typeof listen>(),
}));

function makeEvent(kind: ExecutionEvent['kind']): ExecutionEvent {
  return {
    execution_id: 'exec:1',
    sequence: 1,
    trace_id: 'trace:1',
    occurred_at_ms: 1_700_000_000_000,
    kind,
  };
}

describe('execution wire invoke wrappers', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockReset();
  });

  it('executeRule 使用 execute + request wrapper，字段保持 snake_case', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ execution_id: 'exec:1' });

    await expect(
      executeRule({
        source_id: 'source:test',
        intent: 'Search',
        input: { type: 'Query', value: '关键词' },
        mode: { mode: 'live' },
      }),
    ).resolves.toEqual({ execution_id: 'exec:1' });

    expect(invoke).toHaveBeenCalledWith('execute', {
      request: {
        source_id: 'source:test',
        intent: 'Search',
        input: { type: 'Query', value: '关键词' },
        mode: { mode: 'live' },
      },
    });
  });

  it('executeRule 的 replay 模式带 execution_id pin', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ execution_id: 'exec:2' });

    await executeRule({
      source_id: 'source:test',
      intent: 'ResolveItem',
      input: { type: 'None' },
      mode: { mode: 'replay', execution_id: 'exec:archived' },
    });

    expect(invoke).toHaveBeenCalledWith('execute', {
      request: {
        source_id: 'source:test',
        intent: 'ResolveItem',
        input: { type: 'None' },
        mode: { mode: 'replay', execution_id: 'exec:archived' },
      },
    });
  });

  it('cancelExecution 使用 cancel_execution 并回读 changed', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ execution_id: 'exec:1', changed: true });

    await expect(cancelExecution({ execution_id: 'exec:1' })).resolves.toEqual({
      execution_id: 'exec:1',
      changed: true,
    });
    expect(invoke).toHaveBeenCalledWith('cancel_execution', {
      request: { execution_id: 'exec:1' },
    });
  });

  it('catchUpExecution 使用 catch_up_execution 并回读回放进度', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      execution_id: 'exec:1',
      replayed_count: 3,
      delivered_through_sequence: 7,
    });

    await expect(catchUpExecution({ execution_id: 'exec:1', after_sequence: 4 })).resolves.toEqual({
      execution_id: 'exec:1',
      replayed_count: 3,
      delivered_through_sequence: 7,
    });
    expect(invoke).toHaveBeenCalledWith('catch_up_execution', {
      request: { execution_id: 'exec:1', after_sequence: 4 },
    });
  });
});

describe('execution event 订阅', () => {
  beforeEach(() => {
    vi.mocked(listen).mockReset();
  });

  it('订阅 rule-execution-event 并只把满足合同的 payload 交给 handler', async () => {
    let deliver: unknown = null;
    vi.mocked(listen).mockImplementation((_event, handler) => {
      deliver = handler;
      return Promise.resolve(() => undefined);
    });
    const received: ExecutionEvent[] = [];

    await listenRuleExecutionEvents((event) => received.push(event));
    expect(listen).toHaveBeenCalledWith(RULE_EXECUTION_EVENT, expect.any(Function));

    const emit = deliver as (message: { payload: unknown }) => void;
    const event = makeEvent({ kind: 'started' });
    emit({ payload: event });
    // 未知 kind 与缺字段的 payload 都不进 handler：折叠逻辑不必处理半截形状。
    emit({ payload: { ...event, kind: { kind: 'future_event' } } });
    emit({ payload: { execution_id: 'exec:1' } });

    expect(received).toEqual([event]);
  });
});

describe('execution event 判定', () => {
  it('七个已声明 kind 都满足 kind 合同', () => {
    for (const kind of [
      'started',
      'diagnostic',
      'effect_captured',
      'delta_committed',
      'completed',
      'failed',
      'cancelled',
    ]) {
      expect(isExecutionEventKind(kind)).toBe(true);
    }
    expect(isExecutionEventKind('effect_replayed')).toBe(false);
    expect(isExecutionEventKind(undefined)).toBe(false);
  });

  it('isExecutionEvent 逐字段校验，并保留 JS 失败诊断', () => {
    const ruleError: RuleErrorWire = {
      stage: 'execution',
      code: 'js_timeout',
      message: 'JS 执行超时',
      trace_id: 'trace:1',
      retryable: false,
      diagnostics: [],
    };
    const failed = makeEvent({ kind: 'failed', error: ruleError });

    expect(isExecutionEvent(failed)).toBe(true);
    expect(isTerminalExecutionEvent(failed)).toBe(true);
    expect(executionFailureDiagnostics(ruleError)).toEqual([
      { code: 'js_timeout', severity: 'error', message: 'JS 执行超时' },
    ]);
    expect(isExecutionEvent({ ...failed, sequence: '1' })).toBe(false);
    expect(isTerminalExecutionEvent(makeEvent({ kind: 'started' }))).toBe(false);
    expect(isTerminalExecutionEvent(makeEvent({ kind: 'cancelled' }))).toBe(true);
  });

  it('失败错误自带 diagnostics 时原样透传，不重新解释 severity', () => {
    expect(
      executionFailureDiagnostics({
        stage: 'execution',
        code: 'js_output_budget',
        message: 'JS 输出超限',
        trace_id: 'trace:1',
        retryable: false,
        diagnostics: [
          { code: 'js_output_budget', severity: 'warning', message: 'JS 输出超限(超过 4096 字节)' },
        ],
      }),
    ).toEqual([
      { code: 'js_output_budget', severity: 'warning', message: 'JS 输出超限(超过 4096 字节)' },
    ]);
  });
});
