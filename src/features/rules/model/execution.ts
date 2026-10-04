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

/**
 * 本次运行的模式。
 *
 * 启动时固定，之后不变：replay 失败的运行也可能在界面上明确写着「重放」，
 * 而不会被任何路径改写成一次看起来成功的 live 运行。
 */
export type ExecutionRunMode = 'live' | 'replay';

/** 一次已捕获 effect 的可展示摘要（`effect_captured` 事件切片）。 */
export type ExecutionCapture = {
  effectId: string;
  outputHash: string;
  /** 该 capture 的 durable 内容寻址引用数量。 */
  artifactCount: number;
};

export type ExecutionRunState = {
  status: ExecutionRunStatus;
  /** 本次运行的模式。 */
  mode: ExecutionRunMode;
  /** 重放固定的历史 execution id；live 为 null。 */
  replayOf: ExecutionId | null;
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
  /** 已捕获的 effect 摘要，按捕获顺序。replay 不产生新捕获，因此恒为空。 */
  captures: ExecutionCapture[];
  /** 已折叠到的最大 event sequence；用于丢弃重复投递。 */
  lastSequence: number;
};

/** 未运行的初始状态；`Object.freeze` 防止调用方就地改写共享常量。 */
export const IDLE_EXECUTION_RUN: ExecutionRunState = Object.freeze({
  status: 'idle',
  mode: 'live',
  replayOf: null,
  executionId: null,
  traceId: null,
  failureCode: null,
  diagnostics: [],
  captures: [],
  lastSequence: 0,
});

/** 运行已到终态（成功 / 失败 / 取消）。 */
export function isExecutionRunFinished(state: ExecutionRunState): boolean {
  return state.status === 'succeeded' || state.status === 'failed' || state.status === 'cancelled';
}

/** 本次运行是否 replay；模式在启动时固定，终态也不改。 */
export function isReplayRun(state: ExecutionRunState): boolean {
  return state.mode === 'replay';
}

/** 能否取消：只有进行中的运行有可取消的 execution。 */
export function canCancelExecutionRun(state: ExecutionRunState): boolean {
  return state.status === 'running' && state.executionId !== null;
}

/**
 * 启动一次运行：清空上一次的诊断与捕获，进入 running。
 *
 * `replayOf` 非空即 replay：被重放的历史 execution 只在这里记录一次，之后不变。
 */
export function beginExecutionRun(replayOf: ExecutionId | null = null): ExecutionRunState {
  return {
    ...IDLE_EXECUTION_RUN,
    status: 'running',
    mode: replayOf === null ? 'live' : 'replay',
    replayOf,
  };
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
 * 重放终态失败的稳定归类。
 *
 * 一次 replay 不只可能「失败」：它可能缺一段没被捕获的 effect，也可能在历史
 * 输入/输出校验上不一致，还可能压根固定不出一份可重放的历史。这三件事对作者意味着
 * 完全不同的下一步，因此在这里分开，而不是都长成一句「运行失败」。
 */
export type ReplayFailureReason = 'capture_missing' | 'history_mismatch' | 'history_unavailable';

/**
 * 重放专属稳定 code → 归类的唯一映射表。
 *
 * code 由 Rust 侧写死（见 `lj-rule-system` 的 error_mapping / session_delivery /
 * lifecycle）：
 * - 缺 archive / 缺一段 capture：`replay_capture_missing`；
 * - 历史输入、输出、witness 或收据校验不一致：`replay_record_mismatch` 一族；
 * - 历史固定不出来（不存在、未成功、被 GC、pin 与请求或快照不符）：其余。
 *
 * 未登记的 code 一律不归类：宁可退回普通的运行失败，也不把未知失败说成某种重放结局。
 */
const REPLAY_FAILURE_REASONS: Record<string, ReplayFailureReason> = {
  replay_capture_missing: 'capture_missing',
  replay_record_mismatch: 'history_mismatch',
  replay_fingerprint_mismatch: 'history_mismatch',
  replay_output_hash_mismatch: 'history_mismatch',
  replay_witness_mismatch: 'history_mismatch',
  replay_delta_invalid: 'history_mismatch',
  replay_execution_missing: 'history_unavailable',
  replay_execution_not_completed: 'history_unavailable',
  replay_revision_unavailable: 'history_unavailable',
  replay_pin_unavailable: 'history_unavailable',
  replay_delta_missing: 'history_unavailable',
  replay_continue_action_missing: 'history_unavailable',
  replay_source_mismatch: 'history_unavailable',
  replay_mode_mismatch: 'history_unavailable',
  replay_source_snapshot_invalid: 'history_unavailable',
  unsupported_pinned_intent: 'history_unavailable',
  LEGACY_RULE_CONTRACT_UNSUPPORTED: 'history_unavailable',
};

/** 稳定 code 对应的 replay 失败归类；不是 replay 专属失败时为 null。 */
export function replayFailureReason(code: string): ReplayFailureReason | null {
  if (!Object.hasOwn(REPLAY_FAILURE_REASONS, code)) return null;
  return REPLAY_FAILURE_REASONS[code];
}

/**
 * 本次运行的 replay 失败归类；live 运行与非 replay 专属失败都是 null。
 *
 * 归类只在 replay 运行上成立：同样一个 code 出现在 live 运行里时，没有对应的历史
 * 可解释，因此不当成 replay 结局。
 */
export function replayFailureReasonOf(state: ExecutionRunState): ReplayFailureReason | null {
  if (!isReplayRun(state) || state.failureCode === null) return null;
  return replayFailureReason(state.failureCode);
}

/**
 * 折叠一个 execution event。
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
    case 'effect_captured': {
      return {
        ...advanced,
        captures: [
          ...state.captures,
          {
            effectId: kind.effect_id,
            outputHash: kind.output_hash,
            artifactCount: kind.artifact_refs.length,
          },
        ],
      };
    }
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
