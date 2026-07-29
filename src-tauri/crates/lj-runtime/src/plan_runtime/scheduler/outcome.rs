//! typed effect/archive failure 到既有 runtime outcome 的映射。

use super::{
    DurableCaptureReceipt, EffectError, EffectErrorCode, ExecutionFailure, InvocationPath,
    PlanExecutionRequest, RunOutcome, RuntimeFailureCode, Uuid,
};

pub(in crate::plan_runtime::scheduler) fn effect_error_outcome(
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

pub(in crate::plan_runtime::scheduler) fn failed(
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

pub(in crate::plan_runtime::scheduler) fn receipt_matches(
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
