//! effect invocation 的模式分派；live 与 replay 实现位于隔离模块。

use super::{
    Arc, EffectDeclaration, EffectExecution, EffectInput, EffectOutput, ExecutionMode, Instrument,
    InvocationPath, PlanNode, PreparedEffectInvocation, RunOutcome, RuntimeFailureCode, Uuid,
    acquire_permit, effect_fingerprint, enforce_capabilities, execute_live_effect,
    execute_replay_effect, failed,
};

pub(in crate::plan_runtime::scheduler) async fn execute_effect(
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
            execute_replay_effect(
                context.request,
                context.archive,
                context.cancellation,
                context.emitter,
                invocation,
                archived_execution_id,
            )
            .instrument(span)
            .await
        }
    }
}
