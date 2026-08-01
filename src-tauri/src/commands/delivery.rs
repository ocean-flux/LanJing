//! execution event wire、Tauri delivery 与 cancellation cleanup。

use futures::StreamExt;
use futures::stream::BoxStream;
use lj_rule_system::{ExecutionEvent, ExecutionEventKind, ExecutionId, RuleError, RuleErrorStage};

use super::state::CancellationRegistry;

pub(super) const RULE_EXECUTION_EVENT: &str = "rule-execution-event";

fn is_terminal(event: &ExecutionEvent) -> bool {
    matches!(
        event.kind,
        ExecutionEventKind::Completed
            | ExecutionEventKind::Failed { .. }
            | ExecutionEventKind::Cancelled
    )
}

pub(super) async fn forward_execution_events<F>(
    mut events: BoxStream<'static, ExecutionEvent>,
    execution_id: ExecutionId,
    registry: CancellationRegistry,
    mut emit: F,
) where
    F: FnMut(&ExecutionEvent) -> Result<(), ()> + Send,
{
    while let Some(event) = events.next().await {
        let terminal = is_terminal(&event);
        if emit(&event).is_err() {
            tracing::warn!(
                ?execution_id,
                sequence = event.sequence,
                "rule-execution-event 投递失败"
            );
        }
        if terminal {
            remove_cancellation(&registry, execution_id);
            return;
        }
    }
    tracing::warn!(
        ?execution_id,
        "execution delivery stream 在终态前结束，保留取消注册表"
    );
}

pub(super) fn remove_cancellation(registry: &CancellationRegistry, execution_id: ExecutionId) {
    if let Ok(mut entries) = registry.lock() {
        entries.remove(&execution_id);
    } else {
        tracing::warn!(?execution_id, "execution 终态后无法清理取消注册表");
    }
}

pub(super) fn delivery_error(code: &str, message: &str, trace_id: impl Into<String>) -> RuleError {
    RuleError {
        stage: RuleErrorStage::Internal,
        code: code.to_string(),
        message: message.to_string(),
        trace_id: trace_id.into(),
        retryable: true,
        diagnostics: Vec::new(),
    }
}
