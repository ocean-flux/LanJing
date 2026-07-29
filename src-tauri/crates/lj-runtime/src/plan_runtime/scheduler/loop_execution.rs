//! bounded for-each 的顺序执行与 iteration path。

use super::{
    Arc, CollectionSelector, ControlTrace, EffectDeclaration, EffectExecution, EffectInput,
    EffectOutput, InvocationPath, InvocationSequence, LOOP_COLLECTION_HANDLE, LOOP_DONE_HANDLE,
    LoopInvocationSegment, MAX_LOOP_ITERATIONS, PlanExecutionRequest, PlanNode, PlanNodeConfig,
    PortState, QuickJsOutput, RunOutcome, RuntimeFailureCode, RuntimeValue, Uuid, execute_effect,
    execute_non_loop_node, failed, persist_or_replay_control,
};

pub(in crate::plan_runtime::scheduler) async fn execute_loop(
    context: &mut EffectExecution<'_>,
    node: &PlanNode,
    program: &super::super::validation::LoopProgram,
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

pub(in crate::plan_runtime::scheduler) async fn resolve_loop_collection(
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

pub(in crate::plan_runtime::scheduler) async fn execute_loop_iteration(
    context: &mut EffectExecution<'_>,
    node: &PlanNode,
    program: &super::super::validation::LoopProgram,
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

pub(in crate::plan_runtime::scheduler) fn effect_declaration<'a>(
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

pub(in crate::plan_runtime::scheduler) fn next_invocation(
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

pub(in crate::plan_runtime::scheduler) fn route_value(
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

pub(in crate::plan_runtime::scheduler) fn condition_branch(
    output: &EffectOutput,
) -> Option<String> {
    match output {
        EffectOutput::QuickJs(
            QuickJsOutput::Json(serde_json::Value::String(branch)) | QuickJsOutput::Raw(branch),
        ) => Some(branch.clone()),
        _ => None,
    }
}

pub(in crate::plan_runtime::scheduler) fn control_script(code: &str) -> String {
    let source = serde_json::to_string(code).expect("serializing a Rust string cannot fail");
    format!(
        "(()=>{{const source={source};let evaluate;try{{evaluate=Function('input','\"use strict\";return ('+source+'\\n);');}}catch(error){{if(!(error instanceof SyntaxError))throw error;evaluate=Function('input','\"use strict\";'+source);}}return evaluate(globalThis.input);}})()"
    )
}
