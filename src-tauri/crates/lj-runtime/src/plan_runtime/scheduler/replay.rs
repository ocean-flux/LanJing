//! archive-only exact replay 校验；本模块不调用 live handler。

use super::{
    Arc, CancellationHandle, CollectionSelector, ControlExpression, EffectArchive, EffectFailure,
    EffectInput, EffectOutput, EffectReplayLookup, EffectWitness, EventEmitter, ExecutionEventKind,
    JsOutputKind, PlanExecutionRequest, PlanNode, PlanNodeConfig, PreparedEffectInvocation,
    QuickJsOutput, RunOutcome, RuntimeFailureCode, Uuid, captured_output_failure,
    effect_input_hash, effect_output_hash, executed_js, failed, quickjs_script_hash,
    replay_archive_outcome,
};

pub(in crate::plan_runtime::scheduler) async fn execute_replay_effect(
    request: &PlanExecutionRequest,
    archive: &dyn EffectArchive,
    cancellation: &CancellationHandle,
    emitter: &mut EventEmitter,
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
    let record = archive
        .load_replay(EffectReplayLookup {
            archived_execution_id,
            invocation_path: invocation_path.clone(),
            kind: declaration.kind.clone(),
        })
        .await
        .map_err(|error| replay_archive_outcome(request, Some(node.id), Some(effect_id), &error))?;
    let Some(record) = record else {
        return Err(failed(
            request,
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
            request,
            RuntimeFailureCode::ReplayRecordMismatch,
            "replay effect capture 归属不匹配",
            Some(node.id),
            Some(record.effect_id),
        ));
    }
    if record.fingerprint != fingerprint {
        return Err(failed(
            request,
            RuntimeFailureCode::ReplayFingerprintMismatch,
            "replay effect fingerprint 不匹配",
            Some(node.id),
            Some(record.effect_id),
        ));
    }
    let Ok(actual_output_hash) = effect_output_hash(record.output.as_ref()) else {
        return Err(failed(
            request,
            RuntimeFailureCode::ReplayOutputHashMismatch,
            "replay effect 输出无法校验",
            Some(node.id),
            Some(record.effect_id),
        ));
    };
    if record.output_hash != actual_output_hash {
        return Err(failed(
            request,
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
            request,
            RuntimeFailureCode::ReplayWitnessMismatch,
            "replay effect witness 无效",
            Some(node.id),
            Some(record.effect_id),
        ));
    }
    emitter
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
    if cancellation.is_cancelled() {
        Err(RunOutcome::Cancelled)
    } else if let Some(message) = captured_output_failure(record.output.as_ref()) {
        Err(failed(
            request,
            RuntimeFailureCode::EffectFailed,
            message,
            Some(node.id),
            Some(record.effect_id),
        ))
    } else {
        Ok(record.output)
    }
}

pub(in crate::plan_runtime::scheduler) fn replay_witness_matches(
    node: &PlanNode,
    input: &EffectInput,
    witness: &EffectWitness,
    js_code_override: Option<&str>,
) -> bool {
    match witness {
        EffectWitness::Http(_) => true,
        EffectWitness::QuickJs(witness) => {
            let Ok(executed) = executed_js(node, js_code_override) else {
                return false;
            };
            let Ok(input_hash) = effect_input_hash(input) else {
                return false;
            };
            witness.script_hash == quickjs_script_hash(&executed.code)
                && witness.input_hash == input_hash
        }
        EffectWitness::Extract(witness) => {
            effect_input_hash(input).is_ok_and(|input_hash| witness.input_hash == input_hash)
        }
    }
}

pub(in crate::plan_runtime::scheduler) fn js_output_matches_declaration(
    node: &PlanNode,
    output: &EffectOutput,
) -> bool {
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
