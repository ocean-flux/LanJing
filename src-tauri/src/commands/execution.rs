//! execution 启动、取消与 durable catch-up command。

use lj_rule_system::{
    ExecuteRequest, ExecutionCancellation, ExecutionEventKind, ExecutionId, RuleError,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use super::delivery::{
    RULE_EXECUTION_EVENT, delivery_error, forward_execution_events, remove_cancellation,
};
use super::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ExecuteResponse {
    pub execution_id: ExecutionId,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct CancelExecutionRequest {
    pub execution_id: ExecutionId,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CancelExecutionResponse {
    pub execution_id: ExecutionId,
    pub changed: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CatchUpExecutionRequest {
    pub execution_id: ExecutionId,
    pub after_sequence: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CatchUpExecutionResponse {
    pub execution_id: ExecutionId,
    pub replayed_count: usize,
    pub delivered_through_sequence: u64,
}

#[tauri::command]
pub(crate) async fn execute(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ExecuteRequest,
) -> Result<ExecuteResponse, RuleError> {
    let session = state.system.execute(request).await?;
    let execution_id = session.id;
    let cancellation = session.cancellation_handle();
    state
        .cancellations
        .lock()
        .map_err(|_| {
            delivery_error(
                "CANCELLATION_REGISTRY_UNAVAILABLE",
                "execution 取消注册表不可用",
                "ipc:execution",
            )
        })?
        .insert(execution_id, cancellation);
    let registry = state.cancellations.clone();
    let events = session.into_events();

    tauri::async_runtime::spawn(async move {
        forward_execution_events(events, execution_id, registry, |payload| {
            app.emit(RULE_EXECUTION_EVENT, payload).map_err(|_| ())
        })
        .await;
    });

    Ok(ExecuteResponse { execution_id })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri command extractor 按值提供 State。
pub(crate) fn cancel_execution(
    state: State<'_, AppState>,
    request: CancelExecutionRequest,
) -> Result<CancelExecutionResponse, RuleError> {
    let registry = state.cancellations.lock().map_err(|_| {
        delivery_error(
            "CANCELLATION_REGISTRY_UNAVAILABLE",
            "execution 取消注册表不可用",
            "ipc:execution",
        )
    })?;
    let changed = registry
        .get(&request.execution_id)
        .is_some_and(ExecutionCancellation::cancel);
    Ok(CancelExecutionResponse {
        execution_id: request.execution_id,
        changed,
    })
}

#[tauri::command]
pub(crate) async fn catch_up_execution(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CatchUpExecutionRequest,
) -> Result<CatchUpExecutionResponse, RuleError> {
    let events = state
        .system
        .catch_up_execution(request.execution_id, request.after_sequence)
        .await?;
    let replayed_count = events.len();
    let delivered_through_sequence = events
        .last()
        .map_or(request.after_sequence, |event| event.sequence);
    let observed_terminal = events.last().is_some_and(|event| {
        matches!(
            &event.kind,
            ExecutionEventKind::Completed
                | ExecutionEventKind::Failed { .. }
                | ExecutionEventKind::Cancelled
        )
    });

    for event in events {
        let trace_id = event.trace_id.clone();
        app.emit(RULE_EXECUTION_EVENT, event).map_err(|_| {
            delivery_error(
                "EXECUTION_EVENT_DELIVERY_FAILED",
                "execution 事件投递失败",
                trace_id,
            )
        })?;
    }
    if observed_terminal {
        remove_cancellation(&state.cancellations, request.execution_id);
    }

    Ok(CatchUpExecutionResponse {
        execution_id: request.execution_id,
        replayed_count,
        delivered_through_sequence,
    })
}
