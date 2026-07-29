//! live handler 输出、witness 与 durable receipt 校验。

use super::{
    Arc, EffectCapture, EffectExecution, EffectOutput, ExecutionEventKind,
    PreparedEffectInvocation, RunOutcome, RuntimeFailureCode, captured_output_failure,
    effect_error_outcome, failed, invoke_live_effect, js_output_matches_declaration,
    receipt_matches,
};

pub(in crate::plan_runtime::scheduler) async fn execute_live_effect(
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
