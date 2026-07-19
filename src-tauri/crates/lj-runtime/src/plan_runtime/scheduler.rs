//! Plan 节点调度与 effect 执行。
//!
//! 此模块只执行已验证的路径。live effect 必须先完成 archive 的 durable 写入并校验收据，
//! 才会发布 `EffectCaptured` 或把输出交给下游；replay 仅读取 archive 并逐项校验归属、
//! fingerprint、输出与 witness，绝不回退到真实网络或 `QuickJS`。

use std::collections::BTreeMap;
use std::sync::Arc;

use lj_media::MediaResourceId;
use lj_rule_model::{
    ControlledMapper, EffectDeclaration, EffectKind, HttpSpec, PlanNode, PlanNodeKind,
    PolicyCapabilities,
};
use tokio::sync::{OwnedSemaphorePermit, mpsc};
use tracing::Instrument;
use uuid::Uuid;

use crate::effect::{
    CancellationHandle, CapturedEffectOutput, DurableCaptureReceipt, EffectArchive,
    EffectCancellation, EffectCapture, EffectError, EffectErrorCode, EffectFailure, EffectHandlers,
    EffectInput, EffectOutput, EffectReplayLookup, EffectWitness, ExtractEffectRequest,
    ExtractOutput, HttpEffectRequest, QuickJsEffectRequest, QuickJsOutput, effect_input_hash,
    effect_output_hash, quickjs_script_hash,
};
use crate::mapper::MapperContext;

use super::api::{
    ExecutionEvent, ExecutionEventKind, ExecutionFailure, ExecutionMode, PlanExecutionRequest,
    RuntimeFailureCode, RuntimeState,
};
use super::validation::{ExecutionPath, effect_fingerprint};

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
    handlers: EffectHandlers,
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
        &handlers,
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
    handlers: &EffectHandlers,
    archive: &dyn EffectArchive,
    cancellation: &CancellationHandle,
    emitter: &mut EventEmitter,
) -> RunOutcome {
    let Some(entry) = request.plan.intent_entries.get(&request.intent) else {
        return failed(
            request,
            RuntimeFailureCode::Internal,
            "运行前已校验的标准意图丢失",
            None,
            None,
        );
    };
    let mut outputs = BTreeMap::<Uuid, Arc<EffectOutput>>::new();

    for node_id in &path.node_ids {
        if cancellation.is_cancelled() {
            return RunOutcome::Cancelled;
        }
        let Some(node) = request.plan.nodes.iter().find(|node| node.id == *node_id) else {
            return failed(
                request,
                RuntimeFailureCode::Internal,
                "运行前已校验的 Plan 节点丢失",
                Some(*node_id),
                None,
            );
        };
        let input = if *node_id == entry.entry_node {
            EffectInput::Intent(request.input.clone())
        } else {
            let Some(parent) = path.predecessors.get(node_id) else {
                return failed(
                    request,
                    RuntimeFailureCode::Internal,
                    "运行前已校验的节点依赖丢失",
                    Some(*node_id),
                    None,
                );
            };
            let Some(output) = outputs.get(parent) else {
                return failed(
                    request,
                    RuntimeFailureCode::Internal,
                    "上游节点没有可用的已确认输出",
                    Some(*node_id),
                    None,
                );
            };
            EffectInput::Output(output.clone())
        };

        match node.kind {
            PlanNodeKind::Http | PlanNodeKind::Js | PlanNodeKind::Extract => {
                let Some(declaration) = request
                    .plan
                    .effects
                    .iter()
                    .find(|effect| effect.node_id == node.id)
                else {
                    return failed(
                        request,
                        RuntimeFailureCode::Internal,
                        "运行前已校验的 effect 声明丢失",
                        Some(node.id),
                        None,
                    );
                };
                let span = tracing::info_span!(
                    "plan_effect",
                    execution_id = %request.execution_id,
                    trace_id = %request.trace_id,
                    source_id = %request.source_id,
                    node_id = %node.id,
                    effect_kind = ?declaration.kind,
                );
                let result = {
                    let mut effect_execution = EffectExecution {
                        state,
                        request,
                        handlers,
                        archive,
                        cancellation,
                        emitter,
                    };
                    execute_effect(&mut effect_execution, node, declaration, input)
                        .instrument(span)
                        .await
                };
                match result {
                    Ok(output) => {
                        outputs.insert(node.id, output);
                    }
                    Err(outcome) => return outcome,
                }
            }
            PlanNodeKind::Mapper => {
                let Ok(mapper) = serde_json::from_value::<ControlledMapper>(node.config.clone())
                else {
                    return failed(
                        request,
                        RuntimeFailureCode::Internal,
                        "运行前已校验的 Mapper 配置无法读取",
                        Some(node.id),
                        None,
                    );
                };
                let Ok(value) = mapper_input(&input) else {
                    return failed(
                        request,
                        RuntimeFailureCode::InputTypeMismatch,
                        "Mapper 需要 JSON 上游输出",
                        Some(node.id),
                        None,
                    );
                };
                let mapper_context = MapperContext::for_plan(
                    source_media_id(&request.source_id),
                    request.base_url.clone(),
                    request.plan.intent_entries.keys().copied().collect(),
                );
                let delta =
                    mapper_context.map_plan_json(&mapper, request.intent, &request.input, &value);
                emitter
                    .emit(ExecutionEventKind::DeltaProduced {
                        node_id: node.id,
                        delta,
                    })
                    .await;
                if cancellation.is_cancelled() {
                    return RunOutcome::Cancelled;
                }
            }
            PlanNodeKind::Merge | PlanNodeKind::Condition | PlanNodeKind::Loop => {
                return failed(
                    request,
                    RuntimeFailureCode::Internal,
                    "运行前已校验的控制流节点不应进入执行",
                    Some(node.id),
                    None,
                );
            }
        }
    }

    RunOutcome::Completed
}

struct EffectExecution<'a> {
    state: &'a RuntimeState,
    request: &'a PlanExecutionRequest,
    handlers: &'a EffectHandlers,
    archive: &'a dyn EffectArchive,
    cancellation: &'a CancellationHandle,
    emitter: &'a mut EventEmitter,
}

async fn execute_effect(
    context: &mut EffectExecution<'_>,
    node: &PlanNode,
    declaration: &EffectDeclaration,
    input: EffectInput,
) -> Result<Arc<EffectOutput>, RunOutcome> {
    if context.cancellation.is_cancelled() {
        return Err(RunOutcome::Cancelled);
    }
    if let Err(message) = enforce_capabilities(declaration, &context.request.capabilities) {
        return Err(failed(
            context.request,
            RuntimeFailureCode::CapabilityDenied,
            message,
            Some(node.id),
            None,
        ));
    }

    let effect_id = Uuid::new_v4();
    let token = context.cancellation.token();
    let Ok(source_semaphore) = context
        .state
        .source_effect_permit(&context.request.source_id)
    else {
        return Err(failed(
            context.request,
            RuntimeFailureCode::Internal,
            "来源级并发状态不可用",
            Some(node.id),
            Some(effect_id),
        ));
    };
    // 先等待来源 permit，避免同一来源排队的 effect 占住全局 permit，阻塞其他来源。
    let _source_permit = acquire_permit(
        source_semaphore,
        token.clone(),
        context.request,
        Some(node.id),
        Some(effect_id),
    )
    .await?;
    let _global_permit = acquire_permit(
        context.state.effect_permits.clone(),
        token,
        context.request,
        Some(node.id),
        Some(effect_id),
    )
    .await?;
    if context.cancellation.is_cancelled() {
        return Err(RunOutcome::Cancelled);
    }
    let Ok(fingerprint) = effect_fingerprint(&context.request.plan, node, declaration, &input)
    else {
        return Err(failed(
            context.request,
            RuntimeFailureCode::Internal,
            "effect fingerprint 计算失败",
            Some(node.id),
            Some(effect_id),
        ));
    };

    match context.request.mode {
        ExecutionMode::Live => {
            execute_live_effect(context, node, declaration, effect_id, input, fingerprint).await
        }
        ExecutionMode::Replay {
            archived_execution_id,
        } => {
            execute_replay_effect(
                context,
                node,
                declaration,
                effect_id,
                &input,
                fingerprint,
                archived_execution_id,
            )
            .await
        }
    }
}

async fn execute_live_effect(
    context: &mut EffectExecution<'_>,
    node: &PlanNode,
    declaration: &EffectDeclaration,
    effect_id: Uuid,
    input: EffectInput,
    fingerprint: String,
) -> Result<Arc<EffectOutput>, RunOutcome> {
    let captured = invoke_live_effect(
        context.request,
        node,
        effect_id,
        input,
        context.handlers,
        context.cancellation.token(),
    )
    .await
    .map_err(|error| effect_error_outcome(context.request, node.id, effect_id, error))?;
    let capture = EffectCapture::from_live(
        context.request.execution_id,
        effect_id,
        node.id,
        fingerprint.clone(),
        captured,
    )
    .map_err(|_| {
        failed(
            context.request,
            RuntimeFailureCode::CaptureWitnessInvalid,
            "effect witness 不符合安全完整性合同",
            Some(node.id),
            Some(effect_id),
        )
    })?;
    if capture.kind != declaration.kind {
        return Err(failed(
            context.request,
            RuntimeFailureCode::CaptureWitnessInvalid,
            "effect 输出类型与 Plan 声明不匹配",
            Some(node.id),
            Some(effect_id),
        ));
    }
    let output = capture.output.clone();
    let output_hash = capture.output_hash.clone();
    let witness_hash = capture.witness_hash.clone();

    // live durable-before-advance：已经发生的外部 effect 必须完成 commit/rollback，
    // 收据匹配前既不能发出 EffectCaptured，也不能让下游读取输出。
    let receipt = context
        .archive
        .persist_durable(capture)
        .await
        .map_err(|_| {
            failed(
                context.request,
                RuntimeFailureCode::CaptureFailed,
                "effect durable capture 失败",
                Some(node.id),
                Some(effect_id),
            )
        })?;
    if !receipt_matches(
        &receipt,
        effect_id,
        &fingerprint,
        &output_hash,
        &witness_hash,
    ) {
        return Err(failed(
            context.request,
            RuntimeFailureCode::CaptureReceiptMismatch,
            "effect durable capture 收据不匹配",
            Some(node.id),
            Some(effect_id),
        ));
    }
    context
        .emitter
        .emit(ExecutionEventKind::EffectCaptured {
            node_id: node.id,
            effect_id,
            kind: declaration.kind.clone(),
            fingerprint,
            output_hash,
            witness_hash,
        })
        .await;
    if context.cancellation.is_cancelled() {
        Err(RunOutcome::Cancelled)
    } else if let Some(message) = captured_output_failure(output.as_ref()) {
        Err(failed(
            context.request,
            RuntimeFailureCode::EffectFailed,
            message,
            Some(node.id),
            Some(effect_id),
        ))
    } else {
        Ok(output)
    }
}

async fn execute_replay_effect(
    context: &mut EffectExecution<'_>,
    node: &PlanNode,
    declaration: &EffectDeclaration,
    effect_id: Uuid,
    input: &EffectInput,
    fingerprint: String,
    archived_execution_id: Uuid,
) -> Result<Arc<EffectOutput>, RunOutcome> {
    let record = context
        .archive
        .load_replay(EffectReplayLookup {
            archived_execution_id,
            node_id: node.id,
            kind: declaration.kind.clone(),
        })
        .await
        .map_err(|_| {
            failed(
                context.request,
                RuntimeFailureCode::ReplayCaptureMissing,
                "replay archive 不可读取",
                Some(node.id),
                Some(effect_id),
            )
        })?;
    let Some(record) = record else {
        return Err(failed(
            context.request,
            RuntimeFailureCode::ReplayCaptureMissing,
            "replay 缺少 effect capture",
            Some(node.id),
            Some(effect_id),
        ));
    };
    if record.execution_id != archived_execution_id
        || record.node_id != node.id
        || record.kind != declaration.kind
        || record.output.kind() != declaration.kind
    {
        return Err(failed(
            context.request,
            RuntimeFailureCode::ReplayRecordMismatch,
            "replay effect capture 归属不匹配",
            Some(node.id),
            Some(record.effect_id),
        ));
    }
    if record.fingerprint != fingerprint {
        return Err(failed(
            context.request,
            RuntimeFailureCode::ReplayFingerprintMismatch,
            "replay effect fingerprint 不匹配",
            Some(node.id),
            Some(record.effect_id),
        ));
    }
    let Ok(actual_output_hash) = effect_output_hash(record.output.as_ref()) else {
        return Err(failed(
            context.request,
            RuntimeFailureCode::ReplayOutputHashMismatch,
            "replay effect 输出无法校验",
            Some(node.id),
            Some(record.effect_id),
        ));
    };
    if record.output_hash != actual_output_hash {
        return Err(failed(
            context.request,
            RuntimeFailureCode::ReplayOutputHashMismatch,
            "replay effect 输出 hash 不匹配",
            Some(node.id),
            Some(record.effect_id),
        ));
    }
    // replay strict integrity：不只校验 archive 自洽，还将 witness 重新绑定当前 Plan 节点
    // 与输入；任何不匹配均为硬失败，绝不调用 live adapter 补救。
    if record.validate_replay_integrity().is_err()
        || !replay_witness_matches(node, input, &record.witness)
    {
        return Err(failed(
            context.request,
            RuntimeFailureCode::ReplayWitnessMismatch,
            "replay effect witness 无效",
            Some(node.id),
            Some(record.effect_id),
        ));
    }
    context
        .emitter
        .emit(ExecutionEventKind::EffectReplayed {
            node_id: node.id,
            effect_id: record.effect_id,
            kind: declaration.kind.clone(),
            fingerprint,
            output_hash: actual_output_hash,
            witness_hash: record.witness_hash.clone(),
        })
        .await;
    if context.cancellation.is_cancelled() {
        Err(RunOutcome::Cancelled)
    } else if let Some(message) = captured_output_failure(record.output.as_ref()) {
        Err(failed(
            context.request,
            RuntimeFailureCode::EffectFailed,
            message,
            Some(node.id),
            Some(record.effect_id),
        ))
    } else {
        Ok(record.output)
    }
}

fn replay_witness_matches(node: &PlanNode, input: &EffectInput, witness: &EffectWitness) -> bool {
    match witness {
        EffectWitness::Http(_) => true,
        EffectWitness::QuickJs(witness) => {
            let Ok(code) = parse_js_code(node) else {
                return false;
            };
            let Ok(input_hash) = effect_input_hash(input) else {
                return false;
            };
            witness.script_hash == quickjs_script_hash(&code) && witness.input_hash == input_hash
        }
        EffectWitness::Extract(witness) => {
            effect_input_hash(input).is_ok_and(|input_hash| witness.input_hash == input_hash)
        }
    }
}

async fn acquire_permit(
    semaphore: Arc<tokio::sync::Semaphore>,
    cancellation: EffectCancellation,
    request: &PlanExecutionRequest,
    node_id: Option<Uuid>,
    effect_id: Option<Uuid>,
) -> Result<OwnedSemaphorePermit, RunOutcome> {
    tokio::select! {
        () = cancellation.cancelled() => Err(RunOutcome::Cancelled),
        permit = semaphore.acquire_owned() => permit.map_err(|_| failed(
            request,
            RuntimeFailureCode::Internal,
            "并发控制器已关闭",
            node_id,
            effect_id,
        )),
    }
}

async fn invoke_live_effect(
    request: &PlanExecutionRequest,
    node: &PlanNode,
    effect_id: Uuid,
    input: EffectInput,
    handlers: &EffectHandlers,
    cancellation: EffectCancellation,
) -> Result<CapturedEffectOutput, EffectError> {
    match node.kind {
        PlanNodeKind::Http => {
            let spec = parse_config::<HttpSpec>(node)?;
            handlers
                .http
                .execute_http(
                    HttpEffectRequest {
                        execution_id: request.execution_id,
                        source_id: request.source_id.clone(),
                        node_id: node.id,
                        effect_id,
                        trace_id: request.trace_id.clone(),
                        spec,
                        input,
                        capabilities: request.capabilities.clone(),
                        base_url: request.base_url.clone(),
                        credentials: request.credentials.clone(),
                    },
                    cancellation,
                )
                .await
        }
        PlanNodeKind::Js => {
            let code = parse_js_code(node)?;
            handlers
                .quickjs
                .execute_quickjs(
                    QuickJsEffectRequest {
                        execution_id: request.execution_id,
                        source_id: request.source_id.clone(),
                        node_id: node.id,
                        effect_id,
                        trace_id: request.trace_id.clone(),
                        code,
                        input,
                        capabilities: request.capabilities.clone(),
                    },
                    cancellation,
                )
                .await
        }
        PlanNodeKind::Extract => {
            let spec = parse_config(node)?;
            handlers
                .extract
                .execute_extract(
                    ExtractEffectRequest {
                        execution_id: request.execution_id,
                        source_id: request.source_id.clone(),
                        node_id: node.id,
                        effect_id,
                        trace_id: request.trace_id.clone(),
                        spec,
                        input,
                        base_url: request.base_url.clone(),
                    },
                    cancellation,
                )
                .await
        }
        PlanNodeKind::Mapper
        | PlanNodeKind::Merge
        | PlanNodeKind::Condition
        | PlanNodeKind::Loop => Err(EffectError::new(
            EffectErrorCode::Internal,
            "非 effect 节点不能调用 effect handler",
        )),
    }
}

fn parse_config<T>(node: &PlanNode) -> Result<T, EffectError>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(node.config.clone())
        .map_err(|_| EffectError::new(EffectErrorCode::Internal, "Plan 节点配置无法读取"))
}

fn parse_js_code(node: &PlanNode) -> Result<String, EffectError> {
    node.config
        .get("code")
        .and_then(serde_json::Value::as_str)
        .filter(|code| !code.trim().is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| EffectError::new(EffectErrorCode::Internal, "Plan JS 配置无法读取"))
}

fn mapper_input(input: &EffectInput) -> Result<serde_json::Value, &'static str> {
    let Some(output) = input.output() else {
        return Err("Mapper 需要 JSON 上游输出");
    };
    match output {
        EffectOutput::QuickJs(QuickJsOutput::Json(value)) => Ok(value.clone()),
        EffectOutput::QuickJs(QuickJsOutput::Raw(_)) => Err("Mapper 收到非 JSON QuickJS 输出"),
        EffectOutput::QuickJs(QuickJsOutput::Error(_)) => Err("Mapper 收到失败的 QuickJS 输出"),
        EffectOutput::Extract(ExtractOutput { records }) => {
            Ok(serde_json::Value::Array(records.clone()))
        }
        EffectOutput::Http(_) => Err("Mapper 不能直接消费 HTTP 响应"),
        EffectOutput::Failure(_) => Err("Mapper 收到失败的 effect 输出"),
    }
}

fn captured_output_failure(output: &EffectOutput) -> Option<&'static str> {
    match output {
        EffectOutput::Failure(EffectFailure::Http { .. }) => Some("HTTP effect 执行失败"),
        EffectOutput::Failure(EffectFailure::QuickJs { .. })
        | EffectOutput::QuickJs(QuickJsOutput::Error(_)) => Some("QuickJS effect 执行失败"),
        EffectOutput::Failure(EffectFailure::Extract) => Some("Extract effect 执行失败"),
        EffectOutput::Http(_) | EffectOutput::QuickJs(_) | EffectOutput::Extract(_) => None,
    }
}

fn source_media_id(source_id: &str) -> MediaResourceId {
    if source_id.starts_with("source:") {
        MediaResourceId(source_id.to_string())
    } else {
        MediaResourceId(format!("source:{source_id}"))
    }
}

fn enforce_capabilities(
    declaration: &EffectDeclaration,
    capabilities: &PolicyCapabilities,
) -> Result<(), &'static str> {
    for capability in &declaration.required_capabilities {
        match capability.as_str() {
            "network" if capabilities.network => {}
            "network" => return Err("安装 grant 未允许 network capability"),
            _ => return Err("Plan 声明了 runtime 不支持的 capability"),
        }
    }
    if matches!(declaration.kind, EffectKind::Http | EffectKind::QuickJs) && !capabilities.network {
        return Err("安装 grant 未允许 network capability");
    }
    Ok(())
}

fn effect_error_outcome(
    request: &PlanExecutionRequest,
    node_id: Uuid,
    effect_id: Uuid,
    error: EffectError,
) -> RunOutcome {
    if error.code == EffectErrorCode::Cancelled {
        RunOutcome::Cancelled
    } else {
        failed(
            request,
            RuntimeFailureCode::EffectFailed,
            error.message,
            Some(node_id),
            Some(effect_id),
        )
    }
}

fn failed(
    request: &PlanExecutionRequest,
    code: RuntimeFailureCode,
    message: impl Into<String>,
    node_id: Option<Uuid>,
    effect_id: Option<Uuid>,
) -> RunOutcome {
    RunOutcome::Failed(ExecutionFailure {
        code,
        execution_id: request.execution_id,
        node_id,
        effect_id,
        trace_id: request.trace_id.clone(),
        message: message.into(),
    })
}

fn receipt_matches(
    receipt: &DurableCaptureReceipt,
    effect_id: Uuid,
    fingerprint: &str,
    output_hash: &str,
    witness_hash: &str,
) -> bool {
    receipt.effect_id == effect_id
        && receipt.fingerprint == fingerprint
        && receipt.output_hash == output_hash
        && receipt.witness_hash == witness_hash
}
