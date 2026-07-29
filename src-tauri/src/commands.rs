//! Tauri IPC 的 `RuleSystem` delivery、查询、取消与导入 payload 拉取边界。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::stream::BoxStream;
use lj_rule_system::{
    CandidateId, CapabilityGrant, ExecuteRequest, ExecutionCancellation, ExecutionEvent,
    ExecutionEventKind, ExecutionId, InstalledSource, LibraryEntryUpdate, LibraryProjection,
    LibraryUpdateReceipt, MediaAssetPage, MediaItem, MediaUnitPage, RuleError, RuleInput,
    RuleSystem,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

const RULE_EXECUTION_EVENT: &str = "rule-execution-event";

type CancellationRegistry = Arc<Mutex<HashMap<ExecutionId, ExecutionCancellation>>>;

/// Tauri 共享状态；业务编排全部封装在 `RuleSystem` 内。
pub struct AppState {
    system: Arc<RuleSystem>,
    cancellations: CancellationRegistry,
}

impl AppState {
    #[must_use]
    pub(crate) fn new(system: Arc<RuleSystem>) -> Self {
        Self {
            system,
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FetchImportSrcRequest {
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallRequest {
    pub candidate_id: CandidateId,
    pub grant: CapabilityGrantPreset,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityGrantPreset {
    None,
    NetworkOnly,
}

impl CapabilityGrantPreset {
    fn into_grant(self) -> CapabilityGrant {
        match self {
            Self::None => CapabilityGrant::none(),
            Self::NetworkOnly => CapabilityGrant::network_only(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecuteResponse {
    pub execution_id: ExecutionId,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct CancelExecutionRequest {
    pub execution_id: ExecutionId,
}

#[derive(Debug, Clone, Serialize)]
pub struct CancelExecutionResponse {
    pub execution_id: ExecutionId,
    pub changed: bool,
}

#[derive(Debug, Deserialize)]
pub struct CatchUpExecutionRequest {
    pub execution_id: ExecutionId,
    pub after_sequence: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatchUpExecutionResponse {
    pub execution_id: ExecutionId,
    pub replayed_count: usize,
    pub delivered_through_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleExecutionEvent {
    pub execution_id: ExecutionId,
    pub sequence: u64,
    pub trace_id: String,
    pub occurred_at_ms: i64,
    pub kind: ExecutionEventKind,
}

impl From<ExecutionEvent> for RuleExecutionEvent {
    fn from(event: ExecutionEvent) -> Self {
        Self {
            execution_id: event.execution_id,
            sequence: event.sequence,
            trace_id: event.trace_id,
            occurred_at_ms: event.occurred_at_ms,
            kind: event.kind,
        }
    }
}

impl RuleExecutionEvent {
    fn is_terminal(&self) -> bool {
        matches!(
            self.kind,
            ExecutionEventKind::Completed
                | ExecutionEventKind::Failed { .. }
                | ExecutionEventKind::Cancelled
        )
    }
}

#[tauri::command]
pub async fn fetch_import_src(request: FetchImportSrcRequest) -> Result<String, String> {
    RuleSystem::fetch_import_source(&request.url)
        .await
        .map_err(|error| map_rule_error(&error))
}

#[tauri::command]
pub async fn prepare_install(
    state: State<'_, AppState>,
    request: RuleInput,
) -> Result<lj_rule_system::InstallCandidate, String> {
    state
        .system
        .prepare_install(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

#[tauri::command]
pub async fn install(
    state: State<'_, AppState>,
    request: InstallRequest,
) -> Result<InstalledSource, String> {
    state
        .system
        .install(request.candidate_id, request.grant.into_grant())
        .await
        .map_err(|error| map_rule_error(&error))
}

#[tauri::command]
pub async fn execute(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ExecuteRequest,
) -> Result<ExecuteResponse, String> {
    let session = state
        .system
        .execute(request)
        .await
        .map_err(|error| map_rule_error(&error))?;
    let execution_id = session.id;
    let cancellation = session.cancellation_handle();
    state
        .cancellations
        .lock()
        .map_err(|_| "execution 取消注册表不可用".to_string())?
        .insert(execution_id, cancellation);
    let registry = state.cancellations.clone();
    let events = session.into_events();

    tauri::async_runtime::spawn(async move {
        forward_execution_events(events, execution_id, registry, |payload| {
            app.emit(RULE_EXECUTION_EVENT, payload)
                .map_err(|_| "execution 事件投递失败".to_string())
        })
        .await;
    });

    Ok(ExecuteResponse { execution_id })
}

#[tauri::command]
pub fn cancel_execution(
    state: State<'_, AppState>,
    request: CancelExecutionRequest,
) -> Result<CancelExecutionResponse, String> {
    let app_state = {
        let owned_state = state;
        owned_state.inner()
    };
    let registry = app_state
        .cancellations
        .lock()
        .map_err(|_| "execution 取消注册表不可用".to_string())?;
    let changed = registry
        .get(&request.execution_id)
        .is_some_and(ExecutionCancellation::cancel);
    Ok(CancelExecutionResponse {
        execution_id: request.execution_id,
        changed,
    })
}

#[tauri::command]
pub async fn catch_up_execution(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CatchUpExecutionRequest,
) -> Result<CatchUpExecutionResponse, String> {
    let events = state
        .system
        .catch_up_execution(request.execution_id, request.after_sequence)
        .await
        .map_err(|error| map_rule_error(&error))?;
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
        app.emit(RULE_EXECUTION_EVENT, RuleExecutionEvent::from(event))
            .map_err(|_| "execution 事件投递失败".to_string())?;
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

#[tauri::command]
pub async fn list_installed_sources(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledSource>, String> {
    state
        .system
        .list_installed_sources()
        .await
        .map_err(|error| map_rule_error(&error))
}

#[tauri::command]
pub async fn get_library_projection(
    state: State<'_, AppState>,
) -> Result<LibraryProjection, String> {
    state
        .system
        .get_library_projection()
        .await
        .map_err(|error| map_rule_error(&error))
}

#[tauri::command]
pub async fn update_library_entry(
    state: State<'_, AppState>,
    request: LibraryEntryUpdate,
) -> Result<LibraryUpdateReceipt, String> {
    state
        .system
        .update_library_entry(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

#[derive(Debug, Deserialize)]
pub struct GetMediaItemRequest {
    pub resource_id: String,
}

#[derive(Debug, Deserialize)]
pub struct GetMediaItemsRequest {
    pub resource_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListMediaUnitsRequest {
    pub item_id: String,
    #[serde(default)]
    pub offset: u32,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ListMediaAssetsRequest {
    pub unit_id: String,
    #[serde(default)]
    pub offset: u32,
    pub limit: Option<u32>,
}

#[tauri::command]
pub async fn get_media_item(
    state: State<'_, AppState>,
    request: GetMediaItemRequest,
) -> Result<Option<MediaItem>, String> {
    state
        .system
        .get_media_item(request.resource_id)
        .await
        .map_err(|error| map_rule_error(&error))
}

#[tauri::command]
pub async fn get_media_items(
    state: State<'_, AppState>,
    request: GetMediaItemsRequest,
) -> Result<Vec<MediaItem>, String> {
    state
        .system
        .get_media_items(request.resource_ids)
        .await
        .map_err(|error| map_rule_error(&error))
}

#[tauri::command]
pub async fn list_media_units(
    state: State<'_, AppState>,
    request: ListMediaUnitsRequest,
) -> Result<MediaUnitPage, String> {
    state
        .system
        .list_media_units(request.item_id, request.offset, request.limit)
        .await
        .map_err(|error| map_rule_error(&error))
}

#[tauri::command]
pub async fn list_media_assets(
    state: State<'_, AppState>,
    request: ListMediaAssetsRequest,
) -> Result<MediaAssetPage, String> {
    state
        .system
        .list_media_assets(request.unit_id, request.offset, request.limit)
        .await
        .map_err(|error| map_rule_error(&error))
}

async fn forward_execution_events<F>(
    mut events: BoxStream<'static, ExecutionEvent>,
    execution_id: ExecutionId,
    registry: CancellationRegistry,
    mut emit: F,
) where
    F: FnMut(&RuleExecutionEvent) -> Result<(), String> + Send,
{
    while let Some(event) = events.next().await {
        let payload = RuleExecutionEvent::from(event);
        let is_terminal = payload.is_terminal();
        if emit(&payload).is_err() {
            tracing::warn!(
                ?execution_id,
                sequence = payload.sequence,
                "rule-execution-event 投递失败"
            );
        }
        if is_terminal {
            remove_cancellation(&registry, execution_id);
            return;
        }
    }
    tracing::warn!(
        ?execution_id,
        "execution delivery stream 在终态前结束，保留取消注册表"
    );
}

fn remove_cancellation(registry: &CancellationRegistry, execution_id: ExecutionId) {
    if let Ok(mut entries) = registry.lock() {
        entries.remove(&execution_id);
    } else {
        tracing::warn!(?execution_id, "execution 终态后无法清理取消注册表");
    }
}

fn map_rule_error(error: &RuleError) -> String {
    format!(
        "{}: {} (trace_id={})",
        error.code, error.message, error.trace_id
    )
}
