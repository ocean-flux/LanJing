//! Plan 节点调度与 effect 执行。
//!
//! 此模块只执行已验证的路径。live effect 必须先完成 archive 的 durable 写入并校验收据，
//! 才会发布 `EffectCaptured` 或把输出交给下游；replay 仅读取 archive 并逐项校验归属、
//! fingerprint、输出与 witness，绝不回退到真实网络或 `QuickJS`。

use std::collections::BTreeMap;
use std::sync::Arc;

use lj_media::MediaResourceId;
use lj_rule_model::{
    CollectionSelector, ControlExpression, ControlTrace, EffectDeclaration, EffectKind,
    FlowPortRef, InvocationPath, JsBudget, JsOutputKind, LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE,
    LOOP_COLLECTION_HANDLE, LOOP_DONE_HANDLE, LoopInvocationSegment, MAX_LOOP_ITERATIONS,
    MERGE_OUTPUT_HANDLE, PlanNode, PlanNodeConfig, PlanNodeKind, PolicyCapabilities,
};
use tokio::sync::{OwnedSemaphorePermit, mpsc};
use tracing::Instrument;
use uuid::Uuid;

use crate::capability::effective_js_budget;
use crate::effect::{
    CancellationHandle, CapturedEffectOutput, ControlReplayLookup, ControlTraceCapture,
    DurableCaptureReceipt, EffectArchive, EffectArchiveError, EffectArchiveErrorCode,
    EffectCancellation, EffectCapture, EffectError, EffectErrorCode, EffectFailure, EffectInput,
    EffectOutput, EffectReplayLookup, EffectWitness, ExtractEffectRequest, HttpEffectRequest,
    QuickJsEffectRequest, QuickJsOutput, ReplayCompletionLookup, effect_input_hash,
    effect_output_hash, quickjs_script_hash,
};
use crate::effect_registry::{EffectHandler, FrozenEffectRegistry};
use crate::mapper::MapperContext;

use super::api::{
    ExecutionEvent, ExecutionEventKind, ExecutionFailure, ExecutionMode, PlanExecutionRequest,
    RuntimeFailureCode, RuntimeState,
};
use super::control::{RuntimeValue, evaluate_condition, merge_values};
use super::validation::{ExecutionPath, effect_fingerprint};
mod control_archive;
mod dispatch;
mod effect_execution;
mod live_capture;
mod loop_execution;
mod outcome;
mod replay;
mod routing;
mod state;

use control_archive::{persist_or_replay_control, replay_archive_outcome};
use dispatch::{
    acquire_permit, captured_output_failure, enforce_capabilities, executed_js, invoke_live_effect,
    source_media_id,
};
use effect_execution::execute_effect;
use live_capture::execute_live_effect;
use loop_execution::{
    condition_branch, control_script, effect_declaration, execute_loop, next_invocation,
    route_value,
};
use outcome::{effect_error_outcome, failed, receipt_matches};
use replay::{execute_replay_effect, js_output_matches_declaration};
use routing::execute_non_loop_node;
use state::{InvocationSequence, PortState};

#[derive(Debug)]
enum RunOutcome {
    Completed,
    Cancelled,
    Failed(ExecutionFailure),
}

struct EventEmitter {
    execution_id: Uuid,
    trace_id: String,
    sender: mpsc::Sender<ExecutionEvent>,
    next_sequence: u64,
    terminal_sent: bool,
    receiver_gone: bool,
}

impl EventEmitter {
    fn new(execution_id: Uuid, trace_id: String, sender: mpsc::Sender<ExecutionEvent>) -> Self {
        Self {
            execution_id,
            trace_id,
            sender,
            next_sequence: 1,
            terminal_sent: false,
            receiver_gone: false,
        }
    }

    async fn emit(&mut self, kind: ExecutionEventKind) {
        if self.receiver_gone {
            return;
        }
        let event = ExecutionEvent {
            execution_id: self.execution_id,
            sequence: self.next_sequence,
            trace_id: self.trace_id.clone(),
            kind,
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        if self.sender.send(event).await.is_err() {
            // 丢弃 delivery stream 不隐式取消 execution；仅停止继续投递事件。
            self.receiver_gone = true;
        }
    }

    async fn emit_terminal(&mut self, outcome: RunOutcome) {
        if self.terminal_sent {
            return;
        }
        self.terminal_sent = true;
        match outcome {
            RunOutcome::Completed => self.emit(ExecutionEventKind::Completed).await,
            RunOutcome::Cancelled => self.emit(ExecutionEventKind::Cancelled).await,
            RunOutcome::Failed(failure) => self.emit(ExecutionEventKind::Failed { failure }).await,
        }
    }
}

/// 运行一条已验证 Plan 路径，并保证 session 只发射一个终态。
pub(super) async fn run_execution(
    state: Arc<RuntimeState>,
    request: PlanExecutionRequest,
    path: ExecutionPath,
    registry: Arc<FrozenEffectRegistry>,
    archive: Arc<dyn EffectArchive>,
    cancellation: CancellationHandle,
    sender: mpsc::Sender<ExecutionEvent>,
) {
    let mut emitter = EventEmitter::new(request.execution_id, request.trace_id.clone(), sender);
    emitter.emit(ExecutionEventKind::Started).await;

    let execution_permit = match acquire_permit(
        state.execution_permits.clone(),
        cancellation.token(),
        &request,
        None,
        None,
    )
    .await
    {
        Ok(permit) => permit,
        Err(outcome) => {
            emitter.emit_terminal(outcome).await;
            return;
        }
    };

    let outcome = execute_path(
        &state,
        &request,
        &path,
        &registry,
        archive.as_ref(),
        &cancellation,
        &mut emitter,
    )
    .await;
    drop(execution_permit);
    emitter.emit_terminal(outcome).await;
}

async fn execute_path(
    state: &RuntimeState,
    request: &PlanExecutionRequest,
    path: &ExecutionPath,
    registry: &FrozenEffectRegistry,
    archive: &dyn EffectArchive,
    cancellation: &CancellationHandle,
    emitter: &mut EventEmitter,
) -> RunOutcome {
    let mut ports = PortState::default();
    ports.seed_input(
        FlowPortRef::new(path.entry_node, LINEAR_INPUT_HANDLE),
        RuntimeValue::Intent(request.input.clone()),
    );
    let mut invocations = InvocationSequence::new();
    let mut mapper_emitted = false;
    let mut context = EffectExecution {
        state,
        request,
        registry,
        archive,
        cancellation,
        emitter,
    };

    for node_id in &path.node_ids {
        if cancellation.is_cancelled() {
            return RunOutcome::Cancelled;
        }
        let Some(node) = request.plan.nodes().iter().find(|node| node.id == *node_id) else {
            return failed(
                request,
                RuntimeFailureCode::Internal,
                "运行前已校验的 Plan 节点丢失",
                Some(*node_id),
                None,
            );
        };
        let result = if node.kind() == PlanNodeKind::Loop {
            let Some(program) = path.loops.get(&node.id) else {
                return failed(
                    request,
                    RuntimeFailureCode::Internal,
                    "Loop control program 丢失",
                    Some(node.id),
                    None,
                );
            };
            execute_loop(
                &mut context,
                node,
                program,
                &mut ports,
                &mut invocations,
                &[],
            )
            .await
            .map(|_| false)
        } else {
            execute_non_loop_node(&mut context, node, &mut ports, &mut invocations, &[]).await
        };
        match result {
            Ok(produced_delta) => {
                mapper_emitted |= produced_delta && node.id == path.mapper_output;
            }
            Err(outcome) => return outcome,
        }
    }

    if !mapper_emitted {
        return failed(
            request,
            RuntimeFailureCode::InputTypeMismatch,
            "当前控制路径未到达 Mapper",
            Some(path.mapper_output),
            None,
        );
    }
    RunOutcome::Completed
}

struct EffectExecution<'a> {
    state: &'a RuntimeState,
    request: &'a PlanExecutionRequest,
    registry: &'a FrozenEffectRegistry,
    archive: &'a dyn EffectArchive,
    cancellation: &'a CancellationHandle,
    emitter: &'a mut EventEmitter,
}

struct PreparedEffectInvocation<'a> {
    node: &'a PlanNode,
    declaration: &'a EffectDeclaration,
    effect_id: Uuid,
    invocation_path: InvocationPath,
    input: EffectInput,
    fingerprint: String,
    js_code_override: Option<&'a str>,
}
