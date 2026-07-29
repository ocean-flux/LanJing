//! 普通节点、Mapper 与 Condition 路由。

use super::{
    ControlExpression, ControlTrace, EffectExecution, EffectInput, ExecutionEventKind,
    ExecutionMode, InvocationSequence, LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE,
    LoopInvocationSegment, MERGE_OUTPUT_HANDLE, MapperContext, PlanNode, PlanNodeConfig, PortState,
    ReplayCompletionLookup, RunOutcome, RuntimeFailureCode, RuntimeValue, condition_branch,
    control_script, effect_declaration, evaluate_condition, execute_effect, failed, merge_values,
    next_invocation, persist_or_replay_control, replay_archive_outcome, route_value,
    source_media_id,
};

pub(in crate::plan_runtime::scheduler) async fn execute_non_loop_node(
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

pub(in crate::plan_runtime::scheduler) async fn execute_mapper_node(
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

pub(in crate::plan_runtime::scheduler) async fn evaluate_condition_node(
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
