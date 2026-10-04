//! execution IPC 边界：TS 类型镜像 + invoke wrapper + durable 事件订阅。
//!
//! 只镜像 `lj-rule-system` 的安全 DTO（snake_case，`request` wrapper 与既有 wire 一致）。
//! 事件流是已 durable 的 execution event 的 delivery 副本：前端只折叠状态与诊断，
//! 不解释 Plan、不重放、不落地任何 archive。
//!
//! 这里只做「跑一条规则」的最小通用面（execute / cancel / catch-up / 事件），
//! 投影、artifact 与 replay 的字段级镜像由消费方按需扩展，不在本模块假装拥有。

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { RuleErrorWire } from '@/shared/tauri/rules';
import type { InstallDiagnostic, StandardIntent } from '@/shared/tauri/sources';

/** Execution event 的 Tauri 事件名（`RULE_EXECUTION_EVENT`）。 */
export const RULE_EXECUTION_EVENT = 'rule-execution-event';

/** Execution 的 stable opaque ID（Rust `ExecutionId(Uuid)`，serde transparent）。 */
export type ExecutionId = string;

/** Execution 请求模式（`ExecutionMode`，内部 tag `mode`）。 */
export type ExecutionMode = { mode: 'live' } | { mode: 'replay'; execution_id: ExecutionId };

/**
 * 标准意图输入（`IntentInput`）。
 *
 * tagged 形态是 `{ type, value }`，但 tag 用 Rust 变体名（PascalCase），
 * 与 `StandardIntent` 的既有镜像一致；unit 变体 `none` 没有 `value`。
 */
export type IntentInput =
  | { type: 'Query'; value: string }
  | { type: 'ItemId'; value: string }
  | { type: 'UnitId'; value: string }
  | { type: 'ActionId'; value: string }
  | { type: 'Opaque'; value: unknown }
  | { type: 'Page'; value: string }
  | { type: 'None' };

/** 启动标准意图 execution 的请求（`ExecuteRequest`）。 */
export interface ExecuteRequest {
  source_id: string;
  intent: StandardIntent;
  input: IntentInput;
  mode: ExecutionMode;
}

export interface ExecuteResponse {
  execution_id: ExecutionId;
}

export interface CancelExecutionRequest {
  execution_id: ExecutionId;
}

export interface CancelExecutionResponse {
  execution_id: ExecutionId;
  changed: boolean;
}

export interface CatchUpExecutionRequest {
  execution_id: ExecutionId;
  after_sequence: number;
}

export interface CatchUpExecutionResponse {
  execution_id: ExecutionId;
  replayed_count: number;
  delivered_through_sequence: number;
}

/** 明文/压缩 body 的内容寻址引用（`ArtifactRef`）。 */
export interface ArtifactRef {
  hash: string;
  codec: string;
}

/**
 * 标准媒体增量（`MediaGraphDelta`）。
 *
 * 只保留数组形状：单条资源的字段级镜像属于投影 wire，这里不消费也不重声明。
 */
export interface MediaGraphDeltaWire {
  sources: unknown[];
  items: unknown[];
  collections: unknown[];
  units: unknown[];
  assets: unknown[];
  relations: unknown[];
  actions: unknown[];
  hints: unknown[];
}

/** Execution 对外状态转换（`ExecutionEventKind`，内部 tag `kind`）。 */
export type ExecutionEventKind =
  | { kind: 'started' }
  | { kind: 'diagnostic'; code: string; message: string }
  | {
      kind: 'effect_captured';
      effect_id: string;
      artifact_refs: ArtifactRef[];
      output_hash: string;
    }
  | {
      kind: 'delta_committed';
      global_revision: number;
      source_revision: number;
      delta: MediaGraphDeltaWire;
    }
  | { kind: 'completed' }
  | { kind: 'failed'; error: RuleErrorWire }
  | { kind: 'cancelled' };

/** 已 durable、可 delivery 的 execution event（`ExecutionEvent`）。 */
export interface ExecutionEvent {
  execution_id: ExecutionId;
  /** Stream sequence，从 1 开始。 */
  sequence: number;
  trace_id: string;
  occurred_at_ms: number;
  kind: ExecutionEventKind;
}

/** 终态事件；delivery 到终态即结束，取消注册表同时被清理。 */
export function isTerminalExecutionEvent(event: ExecutionEvent): boolean {
  return (
    event.kind.kind === 'completed' ||
    event.kind.kind === 'failed' ||
    event.kind.kind === 'cancelled'
  );
}

/**
 * 已声明的 event kind。
 *
 * 用 `Record` 而不是数组：新增 union 成员却不在这里登记是类型错误，穷尽性交给编译器看着。
 */
const EXECUTION_EVENT_KINDS: Record<ExecutionEventKind['kind'], true> = {
  started: true,
  diagnostic: true,
  effect_captured: true,
  delta_committed: true,
  completed: true,
  failed: true,
  cancelled: true,
};

/**
 * 事件 kind 是否属于当前合同。
 *
 * `kind` 是 tag 字符串而不是闭集类型：将来新增的 event kind 在这里被判定为未知并被
 * 调用方忽略，而不是让折叠逻辑把 `undefined` 当分支处理。
 */
export function isExecutionEventKind(value: unknown): value is ExecutionEventKind['kind'] {
  return typeof value === 'string' && Object.hasOwn(EXECUTION_EVENT_KINDS, value);
}

/** Event 是否满足可折叠的 execution event 合同（IPC 边界形状校验）。 */
export function isExecutionEvent(value: unknown): value is ExecutionEvent {
  if (typeof value !== 'object' || value === null) return false;
  const candidate = value as Partial<ExecutionEvent>;
  return (
    typeof candidate.execution_id === 'string' &&
    typeof candidate.sequence === 'number' &&
    typeof candidate.trace_id === 'string' &&
    typeof candidate.occurred_at_ms === 'number' &&
    typeof candidate.kind === 'object' &&
    candidate.kind !== null &&
    isExecutionEventKind((candidate.kind as { kind?: unknown }).kind)
  );
}

// ---------------------------------------------------------------------------
// Invoke wrapper（command 名与 Tauri 注册表一致；统一 `{ request }` wrapper）
// ---------------------------------------------------------------------------

/**
 * 启动 execution；返回的 `execution_id` 是后续事件与取消的关联键。
 *
 * 事件不随响应返回：订阅 `rule-execution-event` 才拿得到进度与终态。
 */
export function executeRule(request: ExecuteRequest): Promise<ExecuteResponse> {
  return invoke<ExecuteResponse>('execute', { request });
}

/** 请求取消 execution；`changed` 为 false 表示已经取消过或已不在注册表。 */
export function cancelExecution(request: CancelExecutionRequest): Promise<CancelExecutionResponse> {
  return invoke<CancelExecutionResponse>('cancel_execution', { request });
}

/** 补读 `after_sequence` 之后的 durable 事件（按原顺序重新投递到事件流）。 */
export function catchUpExecution(
  request: CatchUpExecutionRequest,
): Promise<CatchUpExecutionResponse> {
  return invoke<CatchUpExecutionResponse>('catch_up_execution', { request });
}

/**
 * 订阅 execution event 流。
 *
 * 一条订阅覆盖所有 execution：调用方按 `execution_id` 过滤自己关心的那一次。
 * 在 `execute` 之前就订阅可以避免错过启动阶段的事件。
 */
export function listenRuleExecutionEvents(
  handler: (event: ExecutionEvent) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(RULE_EXECUTION_EVENT, (message) => {
    if (isExecutionEvent(message.payload)) handler(message.payload);
  });
}

// ---------------------------------------------------------------------------
// 诊断收敛
// ---------------------------------------------------------------------------

/** 失败错误的诊断：后端自带时原样透传，否则用稳定 code + message 合成一条。 */
export function executionFailureDiagnostics(error: RuleErrorWire): InstallDiagnostic[] {
  if (error.diagnostics.length > 0) return error.diagnostics;
  return [{ code: error.code, severity: 'error', message: error.message }];
}
