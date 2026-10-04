//! 预览运行态折叠测试。
//!
//! 覆盖：event → 状态的完整映射、重复/乱序/他人运行的丢弃、终态不可复活、
//! 请求失败（含非 IPC 错误）的稳定 code，以及脚本节点诊断的透传。

import { describe, expect, it } from 'vitest';

import type { ExecutionEvent, ExecutionEventKind } from '@/shared/tauri/execution';
import {
  attachExecutionId,
  beginExecutionRun,
  canCancelExecutionRun,
  failExecutionRun,
  IDLE_EXECUTION_RUN,
  isExecutionRunFinished,
  recordExecutionRequestFailure,
  reduceExecutionRun,
  type ExecutionRunState,
} from './execution';

const EXECUTION_ID = 'exec:1';

/** 关联到本次运行的运行态（启动响应返回之后的状态）。 */
function runningRun(): ExecutionRunState {
  return attachExecutionId(beginExecutionRun(), EXECUTION_ID);
}

function event(sequence: number, kind: ExecutionEventKind, executionId = EXECUTION_ID) {
  return {
    execution_id: executionId,
    sequence,
    trace_id: 'trace:1',
    occurred_at_ms: 1_700_000_000_000,
    kind,
  } satisfies ExecutionEvent;
}

describe('reduceExecutionRun', () => {
  it('started 记录 trace 并保持 running', () => {
    const next = reduceExecutionRun(runningRun(), event(1, { kind: 'started' }));

    expect(next.status).toBe('running');
    expect(next.traceId).toBe('trace:1');
    expect(next.lastSequence).toBe(1);
    expect(canCancelExecutionRun(next)).toBe(true);
  });

  it('未关联 execution id 时事件一律不折叠（同一引用）', () => {
    const state = beginExecutionRun();

    expect(reduceExecutionRun(state, event(1, { kind: 'started' }))).toBe(state);
  });

  it('忽略其他 execution 的事件', () => {
    const state = runningRun();

    expect(reduceExecutionRun(state, event(1, { kind: 'started' }, 'exec:other'))).toBe(state);
  });

  it('诊断事件按到达顺序累积，稳定 code 原样保留', () => {
    let state = runningRun();
    state = reduceExecutionRun(state, event(1, { kind: 'started' }));
    state = reduceExecutionRun(
      state,
      event(2, { kind: 'diagnostic', code: 'js_timeout', message: 'JS 执行超时(超过 50ms)' }),
    );
    state = reduceExecutionRun(
      state,
      event(3, { kind: 'diagnostic', code: 'js_memory_limit', message: 'JS 内存超限' }),
    );

    expect(state.status).toBe('running');
    expect(state.diagnostics).toEqual([
      { code: 'js_timeout', severity: 'error', message: 'JS 执行超时(超过 50ms)' },
      { code: 'js_memory_limit', severity: 'error', message: 'JS 内存超限' },
    ]);
  });

  it('failed 终态保留错误 code 与自带 diagnostics（含 severity）', () => {
    let state = runningRun();
    state = reduceExecutionRun(state, event(1, { kind: 'started' }));
    state = reduceExecutionRun(
      state,
      event(2, {
        kind: 'failed',
        error: {
          stage: 'execution',
          code: 'js_output_budget',
          message: 'JS 输出超限',
          trace_id: 'trace:1',
          retryable: false,
          diagnostics: [
            {
              code: 'js_output_budget',
              severity: 'warning',
              message: 'JS 输出超限(超过 4096 字节)',
            },
          ],
        },
      }),
    );

    expect(state.status).toBe('failed');
    expect(state.failureCode).toBe('js_output_budget');
    expect(state.diagnostics).toEqual([
      { code: 'js_output_budget', severity: 'warning', message: 'JS 输出超限(超过 4096 字节)' },
    ]);
    expect(isExecutionRunFinished(state)).toBe(true);
    expect(canCancelExecutionRun(state)).toBe(false);
  });

  it('取消是终态但不是失败', () => {
    let state = runningRun();
    state = reduceExecutionRun(state, event(1, { kind: 'started' }));
    state = reduceExecutionRun(state, event(2, { kind: 'cancelled' }));

    expect(state.status).toBe('cancelled');
    expect(state.failureCode).toBeNull();
    expect(state.diagnostics).toEqual([]);
  });

  it('completed 进 succeeded，进度事件只推进 sequence', () => {
    let state = runningRun();
    state = reduceExecutionRun(state, event(1, { kind: 'started' }));
    const progressed = reduceExecutionRun(
      state,
      event(2, { kind: 'effect_captured', effect_id: 'e1', artifact_refs: [], output_hash: 'h' }),
    );

    expect(progressed.status).toBe('running');
    expect(progressed.lastSequence).toBe(2);
    expect(reduceExecutionRun(progressed, event(3, { kind: 'completed' })).status).toBe(
      'succeeded',
    );
  });

  it('重复投递与终态之后的事件都不再改变状态', () => {
    let state = runningRun();
    state = reduceExecutionRun(state, event(1, { kind: 'started' }));
    state = reduceExecutionRun(state, event(2, { kind: 'completed' }));

    // 同一 sequence 重投：忽略。
    expect(reduceExecutionRun(state, event(2, { kind: 'completed' }))).toBe(state);
    // 终态之后到来的 diagnostic（catch-up 重放）：不得复活运行，也不得追加诊断。
    expect(
      reduceExecutionRun(state, event(3, { kind: 'diagnostic', code: 'js_timeout', message: 'x' })),
    ).toBe(state);
  });

  it('beginExecutionRun 清空上一次的诊断与 trace', () => {
    const previous: ExecutionRunState = {
      ...IDLE_EXECUTION_RUN,
      status: 'failed',
      executionId: EXECUTION_ID,
      traceId: 'trace:1',
      failureCode: 'js_timeout',
      diagnostics: [{ code: 'js_timeout', severity: 'error', message: 'JS 执行超时' }],
    };

    expect(beginExecutionRun()).toEqual({ ...IDLE_EXECUTION_RUN, status: 'running' });
    expect(previous.status).toBe('failed');
  });
});

describe('预览请求失败', () => {
  it('RuleError 走稳定 code 与透传诊断', () => {
    const state = failExecutionRun(runningRun(), {
      stage: 'execution',
      code: 'source_not_installed',
      message: '来源未安装',
      trace_id: 'trace:1',
      retryable: false,
      diagnostics: [],
    });

    expect(state.status).toBe('failed');
    expect(state.failureCode).toBe('source_not_installed');
    expect(state.diagnostics).toEqual([
      { code: 'source_not_installed', severity: 'error', message: '来源未安装' },
    ]);
  });

  it('非 IPC 错误退化为 EXECUTION_FAILED，不把任意对象当后端错误', () => {
    const state = failExecutionRun(runningRun(), new Error('invoke 崩了'));

    expect(state.failureCode).toBe('EXECUTION_FAILED');
    expect(state.diagnostics).toEqual([
      { code: 'EXECUTION_FAILED', severity: 'error', message: 'invoke 崩了' },
    ]);
  });

  it('取消失败只追加诊断，不改写运行状态', () => {
    const running = reduceExecutionRun(runningRun(), event(1, { kind: 'started' }));
    const state = recordExecutionRequestFailure(running, new Error('registry 不可用'));

    expect(state.status).toBe('running');
    expect(state.failureCode).toBe('EXECUTION_FAILED');
    expect(state.diagnostics).toEqual([
      { code: 'EXECUTION_FAILED', severity: 'error', message: 'registry 不可用' },
    ]);
  });
});
