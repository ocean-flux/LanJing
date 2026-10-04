//! 预览运行态折叠测试。
//!
//! 覆盖：event → 状态的完整映射、重复/乱序/他人运行的丢弃、终态不可复活、
//! 请求失败（含非 IPC 错误）的稳定 code，以及脚本节点诊断的透传。

import { describe, expect, it } from 'vitest';

import type { ExecutionEvent, ExecutionEventKind } from '@/shared/tauri/execution';
import type { RuleErrorWire } from '@/shared/tauri/rules';
import {
  attachExecutionId,
  beginExecutionRun,
  canCancelExecutionRun,
  failExecutionRun,
  IDLE_EXECUTION_RUN,
  isExecutionRunFinished,
  isReplayRun,
  recordExecutionRequestFailure,
  reduceExecutionRun,
  replayFailureReason,
  replayFailureReasonOf,
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

// ---------------------------------------------------------------------------
// Capture 折叠与运行模式
// ---------------------------------------------------------------------------

describe('run 模式与 capture 折叠', () => {
  it('默认启动的是 live 运行，没有固定历史 execution', () => {
    const state = beginExecutionRun();

    expect(state.mode).toBe('live');
    expect(state.replayOf).toBeNull();
    expect(isReplayRun(state)).toBe(false);
    expect(state.captures).toEqual([]);
  });

  it('effect_captured 按捕获顺序累积摘要，且不改变运行状态', () => {
    let state = attachExecutionId(beginExecutionRun(), EXECUTION_ID);
    state = reduceExecutionRun(state, event(1, { kind: 'started' }));
    state = reduceExecutionRun(
      state,
      event(2, {
        kind: 'effect_captured',
        effect_id: 'fx:1',
        artifact_refs: [{ hash: 'blake3:a', codec: 'identity' }],
        output_hash: 'blake3:out1',
      }),
    );
    state = reduceExecutionRun(
      state,
      event(3, {
        kind: 'effect_captured',
        effect_id: 'fx:2',
        artifact_refs: [
          { hash: 'blake3:b', codec: 'identity' },
          { hash: 'blake3:c', codec: 'gzip' },
        ],
        output_hash: 'blake3:out2',
      }),
    );

    expect(state.status).toBe('running');
    expect(state.captures).toEqual([
      { effectId: 'fx:1', outputHash: 'blake3:out1', artifactCount: 1 },
      { effectId: 'fx:2', outputHash: 'blake3:out2', artifactCount: 2 },
    ]);
  });

  it('replay 运行在整个生命周期里都保持 replay，不会被终态改写', () => {
    let state = attachExecutionId(beginExecutionRun('exec:history'), EXECUTION_ID);
    state = reduceExecutionRun(state, event(1, { kind: 'started' }));

    expect(isReplayRun(state)).toBe(true);
    expect(state.mode).toBe('replay');
    expect(state.replayOf).toBe('exec:history');

    state = reduceExecutionRun(state, event(2, { kind: 'completed' }));
    expect(isReplayRun(state)).toBe(true);
    expect(state.replayOf).toBe('exec:history');
  });
});

// ---------------------------------------------------------------------------
// 重放失败归类
//
// Code 是 Rust 侧写死的稳定 code；这里逐条 pin 住归类，新增归类必须显式落在这张表里。
// ---------------------------------------------------------------------------

describe('replay 失败归类', () => {
  it('缺 capture 有自己的结局', () => {
    expect(replayFailureReason('replay_capture_missing')).toBe('capture_missing');
  });

  it('历史输入输出校验不一致归为 history_mismatch', () => {
    for (const code of [
      'replay_record_mismatch',
      'replay_fingerprint_mismatch',
      'replay_output_hash_mismatch',
      'replay_witness_mismatch',
      'replay_delta_invalid',
    ]) {
      expect(replayFailureReason(code)).toBe('history_mismatch');
    }
  });

  it('历史无法固定为可重放基线归为 history_unavailable', () => {
    for (const code of [
      'replay_execution_missing',
      'replay_execution_not_completed',
      'replay_revision_unavailable',
      'replay_pin_unavailable',
      'replay_delta_missing',
      'replay_continue_action_missing',
      'replay_source_mismatch',
      'replay_mode_mismatch',
      'replay_source_snapshot_invalid',
      'unsupported_pinned_intent',
      'LEGACY_RULE_CONTRACT_UNSUPPORTED',
    ]) {
      expect(replayFailureReason(code)).toBe('history_unavailable');
    }
  });

  it('非 replay 专属失败不给 replay 归类，不冒充 replay 结局', () => {
    expect(replayFailureReason('js_timeout')).toBeNull();
    expect(replayFailureReason('EXECUTION_FAILED')).toBeNull();
    expect(replayFailureReason('effect_failed')).toBeNull();
    expect(replayFailureReason('constructor')).toBeNull();
  });

  it('只有 replay 运行的失败才带 replay 归类', () => {
    const captureMissing: RuleErrorWire = {
      stage: 'replay',
      code: 'replay_capture_missing',
      message: '历史 execution 缺少 invocation archive',
      trace_id: 'trace:1',
      retryable: false,
      diagnostics: [],
    };

    const replayFailed = failExecutionRun(beginExecutionRun('exec:history'), captureMissing);
    const liveFailed = failExecutionRun(runningRun(), captureMissing);

    expect(replayFailureReasonOf(replayFailed)).toBe('capture_missing');
    // Live 运行带同样的 code 也不给重放归类：这里没有可解释的重放历史。
    expect(replayFailureReasonOf(liveFailed)).toBeNull();
  });
});
