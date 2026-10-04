//! 预览运行的运行态：把 durable execution event 折叠成单一可展示状态。
//!
//! 纯函数（`reduceExecutionRun`）：不碰 IPC、不碰 React、不碰时间。事件顺序、
//! 重复投递、未知 kind 与失败诊断只在这里解释一次，UI 只做渲染。
//!
//! 运行态是「一次运行」的状态，不携带规则语义：谁发起（哪个节点）、跑的是哪一版
//! 规则都由调用方决定，因此通用到任意节点、任意规则。

import {
  executionFailureDiagnostics,
  type ExecutionEvent,
  type ExecutionId,
} from '@/shared/tauri/execution';
import { isRuleErrorWire, type InstallDiagnostic } from '@/shared/tauri/rules';

/** 一次预览运行的状态。 */
export type ExecutionRunStatus = 'idle' | 'running' | 'succeeded' | 'failed' | 'cancelled';

export type ExecutionRunState = {
  status: ExecutionRunStatus;
  /** 本次运行的 execution id；启动请求返回前为 null。 */
  executionId: ExecutionId | null;
  /** 本次运行的 trace id；`Started` 之后才有。 */
  traceId: string | null;
  /** 终态失败的稳定 code；未失败为 null。 */
  failureCode: string | null;
  /**
   * 本次运行产生的诊断。
   *
   * 脚本节点的超时 / 取消 / 能力拒绝 / 资源超限都经由这里，展示层只读 code 与 message，
   * 不重新解释。
   */
  diagnostics: InstallDiagnostic[];
  /** 已折叠到的最大 event sequence；用于丢弃重复投递。 */
  lastSequence: number;
};

/** 未运行的初始状态；`Object.freeze` 防止调用方就地改写共享常量。 */
export const IDLE_EXECUTION_RUN: ExecutionRunState = Object.freeze({
  status: 'idle',
  executionId: null,
  traceId: null,
  failureCode: null,
  diagnostics: [],
  lastSequence: 0,
});

/** 运行已到终态（成功 / 失败 / 取消）。 */
export function isExecutionRunFinished(state: ExecutionRunState): boolean {
  return state.status === 'succeeded' || state.status === 'failed' || state.status === 'cancelled';
}

/** 能否取消：只有进行中的运行有可取消的 execution。 */
export function canCancelExecutionRun(state: ExecutionRunState): boolean {
  return state.status === 'running' && state.executionId !== null;
}

/** 启动一次运行：清空上一次的诊断，进入 running。 */
export function beginExecutionRun(): ExecutionRunState {
  return { ...IDLE_EXECUTION_RUN, status: 'running' };
}

/** 关联启动响应里的 execution id。 */
export function attachExecutionId(
  state: ExecutionRunState,
  executionId: ExecutionId,
): ExecutionRunState {
  return { ...state, executionId };
}

/** 启动请求本身失败（来源未安装、意图不支持、IPC 异常）：进 failed 并收敛诊断。 */
export function failExecutionRun(state: ExecutionRunState, error: unknown): ExecutionRunState {
  const { code, diagnostics } = executionErrorOutcome(error);
  return { ...state, status: 'failed', failureCode: code, diagnostics };
}

/**
 * 把请求错误记成诊断，但不改运行状态。
 *
 * 用于取消请求失败这类「运行本身还在进行」的失败：取消失败不应该把运行改写成 failed。
 */
export function recordExecutionRequestFailure(
  state: ExecutionRunState,
  error: unknown,
): ExecutionRunState {
  const { code, diagnostics } = executionErrorOutcome(error);
  return {
    ...state,
    failureCode: code,
    diagnostics: [...state.diagnostics, ...diagnostics],
  };
}

/**
 * 把请求错误收敛成稳定 code + 诊断。
 *
 * 后端 `RuleError` 自带稳定 code 与已本地化 message，有 diagnostics 时原样透传；
 * 非 IPC 错误（网络、JS 异常）合成 `EXECUTION_FAILED`，仍然是稳定 code，而不是把
 * 任意对象当作后端错误解释。
 */
export function executionErrorOutcome(error: unknown): {
  code: string;
  diagnostics: InstallDiagnostic[];
} {
  if (isRuleErrorWire(error)) {
    return { code: error.code, diagnostics: executionFailureDiagnostics(error) };
  }
  return {
    code: 'EXECUTION_FAILED',
    diagnostics: [
      {
        code: 'EXECUTION_FAILED',
        severity: 'error',
        message: error instanceof Error ? error.message : String(error),
      },
    ],
  };
}

/**
 * 折叠一个 execution event。
 *
 * 不变量：
 * - 只折叠当前 execution 的事件，别人的运行不影响本次状态；
 * - 终态之后的事件一律忽略（catch-up 重投不会复活已结束的运行）；
 * - `sequence` 只增不减，重复投递按 sequence 丢弃；
 * - 终态失败的错误 code 与诊断必须留下来，取消是终态但不是错误。
 *
 * 返回同一引用表示状态未变，调用方可以据此跳过写入。
 */
export function reduceExecutionRun(
  state: ExecutionRunState,
  event: ExecutionEvent,
): ExecutionRunState {
  if (state.executionId === null || event.execution_id !== state.executionId) return state;
  if (isExecutionRunFinished(state)) return state;
  if (event.sequence <= state.lastSequence) return state;

  // 进度事件（effect 捕获、delta 提交）不改状态，但推进 sequence：
  // 丢掉它们会让后续重复投递被误判成新事件。
  const advanced: ExecutionRunState = { ...state, lastSequence: event.sequence };
  const { kind } = event;
  switch (kind.kind) {
    case 'started': {
      return { ...advanced, status: 'running', traceId: event.trace_id };
    }
    case 'diagnostic': {
      return {
        ...advanced,
        // Wire 上 diagnostic 没有 severity；运行期诊断都是「这次跑出来的问题」，
        // 按 error 计入，不猜 warning。
        diagnostics: [
          ...state.diagnostics,
          { code: kind.code, severity: 'error', message: kind.message },
        ],
      };
    }
    case 'effect_captured':
    case 'delta_committed': {
      return advanced;
    }
    case 'completed': {
      return { ...advanced, status: 'succeeded' };
    }
    case 'failed': {
      return {
        ...advanced,
        status: 'failed',
        failureCode: kind.error.code,
        diagnostics: [...state.diagnostics, ...executionFailureDiagnostics(kind.error)],
      };
    }
    case 'cancelled': {
      return { ...advanced, status: 'cancelled' };
    }
  }
}
