//! control trace 的 durable capture 与 exact replay。

use super::{
    ControlReplayLookup, ControlTrace, ControlTraceCapture, EffectArchiveError,
    EffectArchiveErrorCode, EffectExecution, ExecutionMode, InvocationPath, PlanExecutionRequest,
    RunOutcome, RuntimeFailureCode, Uuid, failed,
};

pub(in crate::plan_runtime::scheduler) async fn persist_or_replay_control(
    context: &mut EffectExecution<'_>,
    invocation_path: InvocationPath,
    trace: ControlTrace,
) -> Result<(), RunOutcome> {
    match context.request.mode {
        ExecutionMode::Live => persist_live_control(context, invocation_path, trace).await?,
        ExecutionMode::Replay {
            archived_execution_id,
        } => replay_control(context, invocation_path, trace, archived_execution_id).await?,
    }
    Ok(())
}

async fn persist_live_control(
    context: &mut EffectExecution<'_>,
    invocation_path: InvocationPath,
    trace: ControlTrace,
) -> Result<(), RunOutcome> {
    let capture =
        ControlTraceCapture::new(context.request.execution_id, invocation_path.clone(), trace)
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
    Ok(())
}

async fn replay_control(
    context: &mut EffectExecution<'_>,
    invocation_path: InvocationPath,
    trace: ControlTrace,
    archived_execution_id: Uuid,
) -> Result<(), RunOutcome> {
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
    let expected = ControlTraceCapture::new(archived_execution_id, invocation_path.clone(), trace)
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
    Ok(())
}

pub(in crate::plan_runtime::scheduler) fn replay_archive_outcome(
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
