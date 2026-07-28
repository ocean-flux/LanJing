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
    FlowPortRef, InvocationPath, JsOutputKind, LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE,
    LOOP_COLLECTION_HANDLE, LOOP_DONE_HANDLE, LoopInvocationSegment, MAX_LOOP_ITERATIONS,
    MERGE_OUTPUT_HANDLE, PlanNode, PlanNodeConfig, PlanNodeKind, PolicyCapabilities,
};
use tokio::sync::{OwnedSemaphorePermit, mpsc};
use tracing::Instrument;
use uuid::Uuid;

use crate::effect::{
    CancellationHandle, CapturedEffectOutput, ControlReplayLookup, ControlTraceCapture,
    DurableCaptureReceipt, EffectArchive, EffectArchiveError, EffectArchiveErrorCode,
    EffectCancellation, EffectCapture, EffectError, EffectErrorCode, EffectFailure, EffectHandlers,
    EffectInput, EffectOutput, EffectReplayLookup, EffectWitness, ExtractEffectRequest,
    HttpEffectRequest, QuickJsEffectRequest, QuickJsOutput, ReplayCompletionLookup,
    effect_input_hash, effect_output_hash, quickjs_script_hash,
};
use crate::mapper::MapperContext;

use super::api::{
    ExecutionEvent, ExecutionEventKind, ExecutionFailure, ExecutionMode, PlanExecutionRequest,
    RuntimeFailureCode, RuntimeState,
};
use super::control::{RuntimeValue, evaluate_condition, merge_values};
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
        handlers,
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

#[derive(Default)]
struct PortState {
    inputs: BTreeMap<FlowPortRef, RuntimeValue>,
    outputs: BTreeMap<FlowPortRef, RuntimeValue>,
}

impl PortState {
    fn seed_input(&mut self, port: FlowPortRef, value: RuntimeValue) {
        self.inputs.insert(port, value);
    }

    fn input(&self, node_id: Uuid, handle: &str) -> Option<RuntimeValue> {
        self.inputs.get(&FlowPortRef::new(node_id, handle)).cloned()
    }

    fn output(&self, port: &FlowPortRef) -> Option<RuntimeValue> {
        self.outputs.get(port).cloned()
    }

    fn route(
        &mut self,
        plan: &lj_rule_model::ExecutionPlan,
        node_id: Uuid,
        handle: &str,
        value: &RuntimeValue,
    ) -> Result<(), &'static str> {
        let source = FlowPortRef::new(node_id, handle);
        self.outputs.insert(source.clone(), (*value).clone());
        for edge in plan.edges().iter().filter(|edge| edge.from == source) {
            if self
                .inputs
                .insert(edge.to.clone(), (*value).clone())
                .is_some()
            {
                return Err("同一 input 收到多个 active producer");
            }
        }
        Ok(())
    }
}

struct InvocationSequence {
    next_ordinal: u64,
}

impl InvocationSequence {
    const fn new() -> Self {
        Self { next_ordinal: 1 }
    }

    fn next(
        &mut self,
        node_id: Uuid,
        loop_iterations: &[LoopInvocationSegment],
    ) -> Result<InvocationPath, ()> {
        let ordinal = self.next_ordinal;
        self.next_ordinal = ordinal.checked_add(1).ok_or(())?;
        InvocationPath::new(node_id, loop_iterations.to_vec(), ordinal).map_err(|_| ())
    }

    const fn observed(&self) -> u64 {
        self.next_ordinal - 1
    }
}

async fn execute_non_loop_node(
    context: &mut EffectExecution<'_>,
    node: &PlanNode,
    ports: &mut PortState,
    invocations: &mut InvocationSequence,
    loop_iterations: &[LoopInvocationSegment],
) -> Result<bool, RunOutcome> {
    match &node.config {
        PlanNodeConfig::Http(_) | PlanNodeConfig::Js(_) | PlanNodeConfig::Extract(_) => {
            let Some(value) = ports.input(node.id, LINEAR_INPUT_HANDLE) else {
                return Ok(false);
            };
            let input = value.into_effect_input().map_err(|message| {
                failed(
                    context.request,
                    RuntimeFailureCode::InputTypeMismatch,
                    message,
                    Some(node.id),
                    None,
                )
            })?;
            let declaration = effect_declaration(context.request, node)?;
            let invocation_path =
                next_invocation(context.request, invocations, node.id, loop_iterations)?;
            let output =
                execute_effect(context, node, declaration, invocation_path, input, None).await?;
            route_value(
                context.request,
                ports,
                node.id,
                LINEAR_OUTPUT_HANDLE,
                &RuntimeValue::Effect(output),
            )?;
            Ok(false)
        }
        PlanNodeConfig::Mapper(mapper) => {
            execute_mapper_node(context, node, mapper, ports, invocations).await
        }
        PlanNodeConfig::Condition(config) => {
            let Some(input) = ports.input(node.id, lj_rule_model::CONDITION_INPUT_HANDLE) else {
                return Ok(false);
            };
            let branch = evaluate_condition_node(
                context,
                node,
                config,
                input.clone(),
                invocations,
                loop_iterations,
            )
            .await?;
            route_value(context.request, ports, node.id, &branch, &input)?;
            Ok(false)
        }
        PlanNodeConfig::Merge(config) => {
            let mut active = Vec::new();
            let mut ordered_inputs = config.inputs.iter().collect::<Vec<_>>();
            ordered_inputs.sort_by_key(|input| input.order);
            for input in ordered_inputs {
                if let Some(value) = ports.input(node.id, &input.handle) {
                    active.push((input.input_id.clone(), value));
                }
            }
            if active.is_empty() {
                return Ok(false);
            }
            let active_inputs = active
                .iter()
                .map(|(input_id, _)| input_id.clone())
                .collect::<Vec<_>>();
            let merged =
                merge_values(config.strategy, RuntimeValue::Many(active)).map_err(|message| {
                    failed(
                        context.request,
                        RuntimeFailureCode::InputTypeMismatch,
                        message,
                        Some(node.id),
                        None,
                    )
                })?;
            let invocation_path =
                next_invocation(context.request, invocations, node.id, loop_iterations)?;
            persist_or_replay_control(
                context,
                invocation_path,
                ControlTrace::Merge { active_inputs },
            )
            .await?;
            route_value(
                context.request,
                ports,
                node.id,
                MERGE_OUTPUT_HANDLE,
                &merged,
            )?;
            Ok(false)
        }
        PlanNodeConfig::Loop(_) => Err(failed(
            context.request,
            RuntimeFailureCode::Internal,
            "nested Loop runtime 未开放",
            Some(node.id),
            None,
        )),
    }
}

async fn execute_mapper_node(
    context: &mut EffectExecution<'_>,
    node: &PlanNode,
    mapper: &lj_rule_model::ControlledMapper,
    ports: &PortState,
    invocations: &InvocationSequence,
) -> Result<bool, RunOutcome> {
    let Some(input) = ports.input(node.id, LINEAR_INPUT_HANDLE) else {
        return Ok(false);
    };
    let value = input.json().map_err(|message| {
        failed(
            context.request,
            RuntimeFailureCode::InputTypeMismatch,
            message,
            Some(node.id),
            None,
        )
    })?;
    if let ExecutionMode::Replay {
        archived_execution_id,
    } = context.request.mode
        && let Err(error) = context
            .archive
            .validate_replay_complete(ReplayCompletionLookup {
                archived_execution_id,
                observed_invocation_count: invocations.observed(),
            })
            .await
    {
        return Err(replay_archive_outcome(
            context.request,
            Some(node.id),
            None,
            &error,
        ));
    }
    let mapper_context = MapperContext::for_plan(
        source_media_id(&context.request.source_id),
        context.request.base_url.clone(),
        context
            .request
            .plan
            .intent_entries()
            .keys()
            .copied()
            .collect(),
    );
    let delta = mapper_context.map_plan_json(
        mapper,
        context.request.intent,
        &context.request.input,
        value.as_ref(),
    );
    context
        .emitter
        .emit(ExecutionEventKind::DeltaProduced {
            node_id: node.id,
            delta,
        })
        .await;
    if context.cancellation.is_cancelled() {
        Err(RunOutcome::Cancelled)
    } else {
        Ok(true)
    }
}

async fn evaluate_condition_node(
    context: &mut EffectExecution<'_>,
    node: &PlanNode,
    config: &lj_rule_model::ConditionConfig,
    input: RuntimeValue,
    invocations: &mut InvocationSequence,
    loop_iterations: &[LoopInvocationSegment],
) -> Result<String, RunOutcome> {
    let json = input.json().map_err(|message| {
        failed(
            context.request,
            RuntimeFailureCode::InputTypeMismatch,
            message,
            Some(node.id),
            None,
        )
    })?;
    let branch = match &config.expression {
        ControlExpression::Typed {
            predicate,
            true_branch,
            false_branch,
        } => {
            if evaluate_condition(predicate, json.as_ref()).map_err(|message| {
                failed(
                    context.request,
                    RuntimeFailureCode::InputTypeMismatch,
                    message,
                    Some(node.id),
                    None,
                )
            })? {
                true_branch.clone()
            } else {
                false_branch.clone()
            }
        }
        ControlExpression::Js { code } => {
            let declaration = effect_declaration(context.request, node)?;
            let invocation_path =
                next_invocation(context.request, invocations, node.id, loop_iterations)?;
            let script = control_script(code);
            let output = execute_effect(
                context,
                node,
                declaration,
                invocation_path,
                EffectInput::Json(json.clone()),
                Some(script.as_str()),
            )
            .await?;
            condition_branch(output.as_ref()).ok_or_else(|| {
                failed(
                    context.request,
                    RuntimeFailureCode::InputTypeMismatch,
                    "Condition JS 必须返回 branch string",
                    Some(node.id),
                    None,
                )
            })?
        }
    };
    let decision = RuntimeValue::Decision(branch);
    let RuntimeValue::Decision(branch) = decision else {
        unreachable!("closed decision variant")
    };
    if !config.branches.contains(&branch) {
        return Err(failed(
            context.request,
            RuntimeFailureCode::InputTypeMismatch,
            "Condition 返回未声明 branch",
            Some(node.id),
            None,
        ));
    }
    let invocation_path = next_invocation(context.request, invocations, node.id, loop_iterations)?;
    persist_or_replay_control(
        context,
        invocation_path,
        ControlTrace::Condition {
            branch: branch.clone(),
        },
    )
    .await?;
    Ok(branch)
}

async fn execute_loop(
    context: &mut EffectExecution<'_>,
    node: &PlanNode,
    program: &super::validation::LoopProgram,
    ports: &mut PortState,
    invocations: &mut InvocationSequence,
    loop_iterations: &[LoopInvocationSegment],
) -> Result<bool, RunOutcome> {
    let PlanNodeConfig::Loop(config) = &node.config else {
        return Err(failed(
            context.request,
            RuntimeFailureCode::Internal,
            "Loop 配置无法读取",
            Some(node.id),
            None,
        ));
    };
    let Some(input) = ports.input(node.id, LOOP_COLLECTION_HANDLE) else {
        return Ok(false);
    };
    let collection_json =
        resolve_loop_collection(context, node, input, invocations, loop_iterations).await?;
    let serde_json::Value::Array(items) = collection_json.as_ref() else {
        return Err(failed(
            context.request,
            RuntimeFailureCode::InputTypeMismatch,
            "Loop collection 必须是 array",
            Some(node.id),
            None,
        ));
    };
    let iteration_count = u32::try_from(items.len()).map_err(|_| {
        failed(
            context.request,
            RuntimeFailureCode::InputTypeMismatch,
            "Loop collection 超过可表示范围",
            Some(node.id),
            None,
        )
    })?;
    if iteration_count > config.max_iterations().get() || iteration_count > MAX_LOOP_ITERATIONS {
        return Err(failed(
            context.request,
            RuntimeFailureCode::InputTypeMismatch,
            "Loop collection 超过 hard max",
            Some(node.id),
            None,
        ));
    }
    let trace_path = next_invocation(context.request, invocations, node.id, loop_iterations)?;
    persist_or_replay_control(context, trace_path, ControlTrace::Loop { iteration_count }).await?;

    let mut collected = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        collected.push(
            execute_loop_iteration(
                context,
                node,
                program,
                item,
                index,
                invocations,
                loop_iterations,
            )
            .await?,
        );
    }
    route_value(
        context.request,
        ports,
        node.id,
        LOOP_DONE_HANDLE,
        &RuntimeValue::Json(Arc::new(serde_json::Value::Array(collected))),
    )?;
    Ok(true)
}

async fn resolve_loop_collection(
    context: &mut EffectExecution<'_>,
    node: &PlanNode,
    input: RuntimeValue,
    invocations: &mut InvocationSequence,
    loop_iterations: &[LoopInvocationSegment],
) -> Result<Arc<serde_json::Value>, RunOutcome> {
    let PlanNodeConfig::Loop(config) = &node.config else {
        return Err(failed(
            context.request,
            RuntimeFailureCode::Internal,
            "Loop 配置无法读取",
            Some(node.id),
            None,
        ));
    };
    match &config.collection {
        CollectionSelector::Typed { pointer } => {
            let json = input.json().map_err(|message| {
                failed(
                    context.request,
                    RuntimeFailureCode::InputTypeMismatch,
                    message,
                    Some(node.id),
                    None,
                )
            })?;
            if pointer.is_empty() {
                Ok(json)
            } else {
                json.pointer(pointer).cloned().map(Arc::new).ok_or_else(|| {
                    failed(
                        context.request,
                        RuntimeFailureCode::InputTypeMismatch,
                        "Loop collection selector 未命中",
                        Some(node.id),
                        None,
                    )
                })
            }
        }
        CollectionSelector::Js { code } => {
            let json_input = input.json().map_err(|message| {
                failed(
                    context.request,
                    RuntimeFailureCode::InputTypeMismatch,
                    message,
                    Some(node.id),
                    None,
                )
            })?;
            let declaration = effect_declaration(context.request, node)?;
            let invocation_path =
                next_invocation(context.request, invocations, node.id, loop_iterations)?;
            let script = control_script(code);
            let output = execute_effect(
                context,
                node,
                declaration,
                invocation_path,
                EffectInput::Json(json_input),
                Some(script.as_str()),
            )
            .await?;
            match output.as_ref() {
                EffectOutput::QuickJs(QuickJsOutput::Json(value)) if value.is_array() => {
                    Ok(Arc::new(value.clone()))
                }
                _ => Err(failed(
                    context.request,
                    RuntimeFailureCode::InputTypeMismatch,
                    "Loop JS selector 必须返回 array",
                    Some(node.id),
                    None,
                )),
            }
        }
    }
}

async fn execute_loop_iteration(
    context: &mut EffectExecution<'_>,
    node: &PlanNode,
    program: &super::validation::LoopProgram,
    item: &serde_json::Value,
    index: usize,
    invocations: &mut InvocationSequence,
    loop_iterations: &[LoopInvocationSegment],
) -> Result<serde_json::Value, RunOutcome> {
    let PlanNodeConfig::Loop(config) = &node.config else {
        return Err(failed(
            context.request,
            RuntimeFailureCode::Internal,
            "Loop 配置无法读取",
            Some(node.id),
            None,
        ));
    };
    if context.cancellation.is_cancelled() {
        return Err(RunOutcome::Cancelled);
    }
    let iteration_index = u32::try_from(index).map_err(|_| {
        failed(
            context.request,
            RuntimeFailureCode::Internal,
            "Loop iteration index 溢出",
            Some(node.id),
            None,
        )
    })?;
    let mut segments = loop_iterations.to_vec();
    segments.push(LoopInvocationSegment {
        loop_id: node.id,
        iteration_index,
    });
    let mut binding = serde_json::Map::new();
    binding.insert(config.item_binding.clone(), item.clone());
    binding.insert(
        config.index_binding.clone(),
        serde_json::Value::Number(index.into()),
    );
    let mut body_ports = PortState::default();
    body_ports.seed_input(
        program.region.body_entry.clone(),
        RuntimeValue::LoopBinding(Arc::new(serde_json::Value::Object(binding))),
    );
    for body_node_id in &program.node_ids {
        let Some(body_node) = context
            .request
            .plan
            .nodes()
            .iter()
            .find(|candidate| candidate.id == *body_node_id)
        else {
            return Err(failed(
                context.request,
                RuntimeFailureCode::Internal,
                "Loop body 节点丢失",
                Some(*body_node_id),
                None,
            ));
        };
        execute_non_loop_node(context, body_node, &mut body_ports, invocations, &segments).await?;
    }
    let yielded = body_ports
        .output(&program.region.yield_source)
        .ok_or_else(|| {
            failed(
                context.request,
                RuntimeFailureCode::InputTypeMismatch,
                "Loop body 未恰好产生一次 yield",
                Some(node.id),
                None,
            )
        })?;
    yielded
        .json()
        .map(|value| value.as_ref().clone())
        .map_err(|message| {
            failed(
                context.request,
                RuntimeFailureCode::InputTypeMismatch,
                message,
                Some(node.id),
                None,
            )
        })
}

fn effect_declaration<'a>(
    request: &'a PlanExecutionRequest,
    node: &PlanNode,
) -> Result<&'a EffectDeclaration, RunOutcome> {
    request
        .plan
        .effects()
        .iter()
        .find(|effect| effect.node_id == node.id)
        .ok_or_else(|| {
            failed(
                request,
                RuntimeFailureCode::Internal,
                "运行前已校验的 effect 声明丢失",
                Some(node.id),
                None,
            )
        })
}

fn next_invocation(
    request: &PlanExecutionRequest,
    invocations: &mut InvocationSequence,
    node_id: Uuid,
    loop_iterations: &[LoopInvocationSegment],
) -> Result<InvocationPath, RunOutcome> {
    invocations.next(node_id, loop_iterations).map_err(|()| {
        failed(
            request,
            RuntimeFailureCode::Internal,
            "invocation identity 无法分配",
            Some(node_id),
            None,
        )
    })
}

fn route_value(
    request: &PlanExecutionRequest,
    ports: &mut PortState,
    node_id: Uuid,
    handle: &str,
    value: &RuntimeValue,
) -> Result<(), RunOutcome> {
    ports
        .route(&request.plan, node_id, handle, value)
        .map_err(|message| {
            failed(
                request,
                RuntimeFailureCode::Internal,
                message,
                Some(node_id),
                None,
            )
        })
}

fn condition_branch(output: &EffectOutput) -> Option<String> {
    match output {
        EffectOutput::QuickJs(
            QuickJsOutput::Json(serde_json::Value::String(branch)) | QuickJsOutput::Raw(branch),
        ) => Some(branch.clone()),
        _ => None,
    }
}

fn control_script(code: &str) -> String {
    let source = serde_json::to_string(code).expect("serializing a Rust string cannot fail");
    format!(
        "(()=>{{const source={source};let evaluate;try{{evaluate=Function('input','\"use strict\";return ('+source+'\\n);');}}catch(error){{if(!(error instanceof SyntaxError))throw error;evaluate=Function('input','\"use strict\";'+source);}}return evaluate(globalThis.input);}})()"
    )
}

async fn persist_or_replay_control(
    context: &mut EffectExecution<'_>,
    invocation_path: InvocationPath,
    trace: ControlTrace,
) -> Result<(), RunOutcome> {
    match context.request.mode {
        ExecutionMode::Live => {
            let capture = ControlTraceCapture::new(
                context.request.execution_id,
                invocation_path.clone(),
                trace,
            )
            .map_err(|_| {
                failed(
                    context.request,
                    RuntimeFailureCode::CaptureFailed,
                    "control trace 无法编码",
                    Some(invocation_path.node_id()),
                    None,
                )
            })?;
            let expected_hash = capture.trace_hash.clone();
            let receipt = context
                .archive
                .persist_control_trace(capture)
                .await
                .map_err(|_| {
                    failed(
                        context.request,
                        RuntimeFailureCode::CaptureFailed,
                        "control trace durable capture 失败",
                        Some(invocation_path.node_id()),
                        None,
                    )
                })?;
            if receipt.invocation_path != invocation_path || receipt.trace_hash != expected_hash {
                return Err(failed(
                    context.request,
                    RuntimeFailureCode::CaptureReceiptMismatch,
                    "control trace durable receipt 不匹配",
                    Some(invocation_path.node_id()),
                    None,
                ));
            }
        }
        ExecutionMode::Replay {
            archived_execution_id,
        } => {
            let archived = context
                .archive
                .load_control_trace(ControlReplayLookup {
                    archived_execution_id,
                    invocation_path: invocation_path.clone(),
                })
                .await
                .map_err(|error| {
                    replay_archive_outcome(
                        context.request,
                        Some(invocation_path.node_id()),
                        None,
                        &error,
                    )
                })?
                .ok_or_else(|| {
                    failed(
                        context.request,
                        RuntimeFailureCode::ReplayCaptureMissing,
                        "replay 缺少 control trace",
                        Some(invocation_path.node_id()),
                        None,
                    )
                })?;
            let expected =
                ControlTraceCapture::new(archived_execution_id, invocation_path.clone(), trace)
                    .map_err(|_| {
                        failed(
                            context.request,
                            RuntimeFailureCode::ReplayRecordMismatch,
                            "replay control trace 无法校验",
                            Some(invocation_path.node_id()),
                            None,
                        )
                    })?;
            if archived.execution_id != archived_execution_id
                || archived.invocation_path != invocation_path
                || archived.trace != expected.trace
                || archived.trace_hash != expected.trace_hash
            {
                return Err(failed(
                    context.request,
                    RuntimeFailureCode::ReplayRecordMismatch,
                    "replay control trace 不匹配",
                    Some(invocation_path.node_id()),
                    None,
                ));
            }
        }
    }
    Ok(())
}

fn replay_archive_outcome(
    request: &PlanExecutionRequest,
    node_id: Option<Uuid>,
    effect_id: Option<Uuid>,
    error: &EffectArchiveError,
) -> RunOutcome {
    let (code, message) = match error.code {
        EffectArchiveErrorCode::LegacyRuleContractUnsupported => (
            RuntimeFailureCode::LegacyRuleContractUnsupported,
            "历史 invocation archive 不受支持",
        ),
        EffectArchiveErrorCode::Integrity => (
            RuntimeFailureCode::ReplayRecordMismatch,
            "replay invocation archive 完整性校验失败",
        ),
        EffectArchiveErrorCode::Unavailable => (
            RuntimeFailureCode::ReplayCaptureMissing,
            "replay invocation archive 不可读取",
        ),
    };
    failed(request, code, message, node_id, effect_id)
}

struct EffectExecution<'a> {
    state: &'a RuntimeState,
    request: &'a PlanExecutionRequest,
    handlers: &'a EffectHandlers,
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

async fn execute_effect(
    context: &mut EffectExecution<'_>,
    node: &PlanNode,
    declaration: &EffectDeclaration,
    invocation_path: InvocationPath,
    input: EffectInput,
    js_code_override: Option<&str>,
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
    let Ok(fingerprint) = effect_fingerprint(
        &context.request.plan,
        node,
        declaration,
        &invocation_path,
        &input,
    ) else {
        return Err(failed(
            context.request,
            RuntimeFailureCode::Internal,
            "effect fingerprint 计算失败",
            Some(node.id),
            Some(effect_id),
        ));
    };
    let span = tracing::info_span!(
        "plan_effect",
        execution_id = %context.request.execution_id,
        trace_id = %context.request.trace_id,
        source_id = %context.request.source_id,
        node_id = %node.id,
        invocation_ordinal = invocation_path.ordinal(),
        effect_kind = ?declaration.kind,
    );

    let invocation = PreparedEffectInvocation {
        node,
        declaration,
        effect_id,
        invocation_path,
        input,
        fingerprint,
        js_code_override,
    };
    match context.request.mode {
        ExecutionMode::Live => {
            execute_live_effect(context, invocation)
                .instrument(span)
                .await
        }
        ExecutionMode::Replay {
            archived_execution_id,
        } => {
            execute_replay_effect(context, invocation, archived_execution_id)
                .instrument(span)
                .await
        }
    }
}

async fn execute_live_effect(
    context: &mut EffectExecution<'_>,
    invocation: PreparedEffectInvocation<'_>,
) -> Result<Arc<EffectOutput>, RunOutcome> {
    let PreparedEffectInvocation {
        node,
        declaration,
        effect_id,
        invocation_path,
        input,
        fingerprint,
        js_code_override,
    } = invocation;
    let captured = invoke_live_effect(
        context.request,
        node,
        effect_id,
        input,
        js_code_override,
        context.handlers,
        context.cancellation.token(),
    )
    .await
    .map_err(|error| effect_error_outcome(context.request, node.id, effect_id, error))?;
    let capture = EffectCapture::from_live(
        context.request.execution_id,
        effect_id,
        invocation_path.clone(),
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
    if capture.kind != declaration.kind
        || !js_output_matches_declaration(node, capture.output.as_ref())
    {
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
        &invocation_path,
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
            invocation_path,
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
    invocation: PreparedEffectInvocation<'_>,
    archived_execution_id: Uuid,
) -> Result<Arc<EffectOutput>, RunOutcome> {
    let PreparedEffectInvocation {
        node,
        declaration,
        effect_id,
        invocation_path,
        input,
        fingerprint,
        js_code_override,
    } = invocation;
    let record = context
        .archive
        .load_replay(EffectReplayLookup {
            archived_execution_id,
            invocation_path: invocation_path.clone(),
            kind: declaration.kind.clone(),
        })
        .await
        .map_err(|error| {
            replay_archive_outcome(context.request, Some(node.id), Some(effect_id), &error)
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
        || record.invocation_path != invocation_path
        || record.invocation_path.node_id() != node.id
        || record.kind != declaration.kind
        || record.output.kind() != declaration.kind
        || !js_output_matches_declaration(node, record.output.as_ref())
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
    // replay strict integrity：不只校验 archive 自洽，还将 witness 重新绑定当前 Plan、exact
    // invocation 与输入；任何不匹配均为硬失败，绝不调用 live adapter 补救。
    if record.validate_replay_integrity().is_err()
        || !replay_witness_matches(node, &input, &record.witness, js_code_override)
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
            invocation_path,
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

fn replay_witness_matches(
    node: &PlanNode,
    input: &EffectInput,
    witness: &EffectWitness,
    js_code_override: Option<&str>,
) -> bool {
    match witness {
        EffectWitness::Http(_) => true,
        EffectWitness::QuickJs(witness) => {
            let Ok(code) = executed_js_code(node, js_code_override) else {
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

fn js_output_matches_declaration(node: &PlanNode, output: &EffectOutput) -> bool {
    match &node.config {
        PlanNodeConfig::Js(config) => match output {
            EffectOutput::QuickJs(QuickJsOutput::Json(_)) => config.output == JsOutputKind::Json,
            EffectOutput::QuickJs(QuickJsOutput::Raw(_)) => config.output == JsOutputKind::Raw,
            EffectOutput::QuickJs(QuickJsOutput::Error(_))
            | EffectOutput::Failure(EffectFailure::QuickJs { .. }) => true,
            _ => false,
        },
        PlanNodeConfig::Condition(lj_rule_model::ConditionConfig {
            expression: ControlExpression::Js { .. },
            ..
        }) => matches!(
            output,
            EffectOutput::QuickJs(
                QuickJsOutput::Json(serde_json::Value::String(_))
                    | QuickJsOutput::Raw(_)
                    | QuickJsOutput::Error(_),
            ) | EffectOutput::Failure(EffectFailure::QuickJs { .. })
        ),
        PlanNodeConfig::Loop(config)
            if matches!(config.collection, CollectionSelector::Js { .. }) =>
        {
            matches!(
                output,
                EffectOutput::QuickJs(QuickJsOutput::Json(_) | QuickJsOutput::Error(_),)
                    | EffectOutput::Failure(EffectFailure::QuickJs { .. })
            )
        }
        PlanNodeConfig::Http(_)
        | PlanNodeConfig::Extract(_)
        | PlanNodeConfig::Mapper(_)
        | PlanNodeConfig::Merge(_)
        | PlanNodeConfig::Condition(_)
        | PlanNodeConfig::Loop(_) => true,
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
    js_code_override: Option<&str>,
    handlers: &EffectHandlers,
    cancellation: EffectCancellation,
) -> Result<CapturedEffectOutput, EffectError> {
    if let Some(code) = js_code_override {
        return handlers
            .quickjs
            .execute_quickjs(
                QuickJsEffectRequest {
                    execution_id: request.execution_id,
                    source_id: request.source_id.clone(),
                    node_id: node.id,
                    effect_id,
                    trace_id: request.trace_id.clone(),
                    code: code.to_string(),
                    input,
                    capabilities: request.capabilities.clone(),
                },
                cancellation,
            )
            .await;
    }
    match &node.config {
        PlanNodeConfig::Http(spec) => {
            handlers
                .http
                .execute_http(
                    HttpEffectRequest {
                        execution_id: request.execution_id,
                        source_id: request.source_id.clone(),
                        node_id: node.id,
                        effect_id,
                        trace_id: request.trace_id.clone(),
                        spec: spec.clone(),
                        input,
                        capabilities: request.capabilities.clone(),
                        base_url: request.base_url.clone(),
                        credentials: request.credentials.clone(),
                    },
                    cancellation,
                )
                .await
        }
        PlanNodeConfig::Js(config) => {
            handlers
                .quickjs
                .execute_quickjs(
                    QuickJsEffectRequest {
                        execution_id: request.execution_id,
                        source_id: request.source_id.clone(),
                        node_id: node.id,
                        effect_id,
                        trace_id: request.trace_id.clone(),
                        code: config.code.clone(),
                        input,
                        capabilities: request.capabilities.clone(),
                    },
                    cancellation,
                )
                .await
        }
        PlanNodeConfig::Extract(spec) => {
            handlers
                .extract
                .execute_extract(
                    ExtractEffectRequest {
                        execution_id: request.execution_id,
                        source_id: request.source_id.clone(),
                        node_id: node.id,
                        effect_id,
                        trace_id: request.trace_id.clone(),
                        spec: spec.clone(),
                        input,
                        base_url: request.base_url.clone(),
                    },
                    cancellation,
                )
                .await
        }
        PlanNodeConfig::Mapper(_)
        | PlanNodeConfig::Merge(_)
        | PlanNodeConfig::Condition(_)
        | PlanNodeConfig::Loop(_) => Err(EffectError::new(
            EffectErrorCode::Internal,
            "非 effect 节点不能调用 effect handler",
        )),
    }
}

fn executed_js_code(
    node: &PlanNode,
    js_code_override: Option<&str>,
) -> Result<String, EffectError> {
    if let Some(code) = js_code_override {
        return (!code.trim().is_empty())
            .then(|| code.to_string())
            .ok_or_else(|| EffectError::new(EffectErrorCode::Internal, "control JS 配置无法读取"));
    }
    let PlanNodeConfig::Js(config) = &node.config else {
        return Err(EffectError::new(
            EffectErrorCode::Internal,
            "Plan JS 配置无法读取",
        ));
    };
    (!config.code.trim().is_empty())
        .then(|| config.code.clone())
        .ok_or_else(|| EffectError::new(EffectErrorCode::Internal, "Plan JS 配置无法读取"))
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
    invocation_path: &InvocationPath,
    fingerprint: &str,
    output_hash: &str,
    witness_hash: &str,
) -> bool {
    receipt.effect_id == effect_id
        && &receipt.invocation_path == invocation_path
        && receipt.fingerprint == fingerprint
        && receipt.output_hash == output_hash
        && receipt.witness_hash == witness_hash
}
