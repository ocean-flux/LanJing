//! Tauri IPC 的 `RuleSystem` delivery、查询与取消边界。
//!
//! 本模块只持有安全 wire DTO、会话投递和取消注册表。规则导入、编译、执行、投影与持久化
//! 全部委托给 `RuleSystem`；不会组装 Graph、processor、storage 或任何 effect handler。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::stream::BoxStream;
use lj_rule_system::{
    CandidateId, CapabilityGrant, ExecuteRequest, ExecutionCancellation, ExecutionEvent,
    ExecutionEventKind, ExecutionId, InstalledSource, LibraryEntryUpdate, LibraryProjection,
    LibraryUpdateReceipt, RuleError, RuleInput, RuleSystem,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

const RULE_EXECUTION_EVENT: &str = "rule-execution-event";

type CancellationRegistry = Arc<Mutex<HashMap<ExecutionId, ExecutionCancellation>>>;

/// Tauri 共享状态：生命周期 façade 与活跃 execution 的取消注册表。
///
/// 不保存 Graph、SQLite、importer、runtime 或节点处理器；它们全部封装在 `RuleSystem` 内部。
pub struct AppState {
    system: Arc<RuleSystem>,
    cancellations: CancellationRegistry,
}

impl AppState {
    /// 用唯一的 `RuleSystem` 实例初始化 IPC 边界状态。
    #[must_use]
    pub(crate) fn new(system: Arc<RuleSystem>) -> Self {
        Self {
            system,
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// 安装请求：opaque candidate 与用户批准的固定 capability 预设。
#[derive(Debug, Deserialize)]
pub struct InstallRequest {
    /// 仅能由 `prepare_install` 返回并原样传回的 candidate token。
    pub candidate_id: CandidateId,
    /// 用户确认的最小 capability 预设；不接受任意 policy JSON。
    pub grant: CapabilityGrantPreset,
}

/// 可从 IPC 选择的安全 capability grant 预设。
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityGrantPreset {
    /// 不批准任何 capability。
    None,
    /// 仅批准网络 capability。
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

/// 成功启动 execution 后返回的安全摘要。
#[derive(Debug, Clone, Serialize)]
pub struct ExecuteResponse {
    /// 此 execution 的 opaque ID；后续取消与 catch-up 都使用它。
    pub execution_id: ExecutionId,
}

/// 请求取消一个活跃 execution。
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct CancelExecutionRequest {
    /// 要取消的 opaque execution ID。
    pub execution_id: ExecutionId,
}

/// 取消请求的幂等结果。
#[derive(Debug, Clone, Serialize)]
pub struct CancelExecutionResponse {
    /// 被请求取消的 opaque execution ID。
    pub execution_id: ExecutionId,
    /// 此次调用是否首次把 execution 变为取消状态。
    pub changed: bool,
}

/// 从某个已持久 sequence 之后补发 execution 事件。
#[derive(Debug, Deserialize)]
pub struct CatchUpExecutionRequest {
    /// 需要补发的 execution。
    pub execution_id: ExecutionId,
    /// 已被客户端观察到的最后一个 sequence；`0` 表示从首个事件开始。
    pub after_sequence: u64,
}

/// catch-up 命令的 delivery 摘要。
#[derive(Debug, Clone, Serialize)]
pub struct CatchUpExecutionResponse {
    /// 被补发的 execution。
    pub execution_id: ExecutionId,
    /// 本次通过 `rule-execution-event` 发出的事件数。
    pub replayed_count: usize,
    /// 本次补发后客户端已连续观察到的最大 sequence。
    pub delivered_through_sequence: u64,
}

/// 唯一的 execution event wire payload。
///
/// `kind` 只包含已持久化的安全状态转换、标准媒体增量或 artifact 引用；不会包含 HTTP body、
/// cookie、token、Definition 或 Plan。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleExecutionEvent {
    /// 所属的 opaque execution ID。
    pub execution_id: ExecutionId,
    /// execution 内严格递增且可用于 catch-up 的持久 sequence。
    pub sequence: u64,
    /// 可用于诊断关联的安全 trace ID。
    pub trace_id: String,
    /// 该事件 durable 提交的 UTC epoch milliseconds。
    pub occurred_at_ms: i64,
    /// 已脱敏且可序列化的 execution 状态转换。
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

/// 生成 durable candidate，但不暴露 Definition、Plan 或旧执行图。
///
/// # Errors
///
/// 来源输入、验证、编译或 candidate staging 失败时返回带安全 code 与 trace ID 的 IPC 错误。
#[tauri::command]
pub async fn prepare_install(
    state: State<'_, AppState>,
    request: RuleInput,
) -> Result<lj_rule_system::InstallCandidate, String> {
    prepare_install_inner(state.system.as_ref(), request).await
}

async fn prepare_install_inner(
    system: &RuleSystem,
    request: RuleInput,
) -> Result<lj_rule_system::InstallCandidate, String> {
    system
        .prepare_install(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 原子安装经 staging 和重验的 opaque candidate。
///
/// # Errors
///
/// candidate 缺失、过期、已消费、grant 不足或安装事务失败时返回安全 IPC 错误。
#[tauri::command]
pub async fn install(
    state: State<'_, AppState>,
    request: InstallRequest,
) -> Result<InstalledSource, String> {
    install_inner(&state, request).await
}

async fn install_inner(
    state: &AppState,
    request: InstallRequest,
) -> Result<InstalledSource, String> {
    state
        .system
        .install(request.candidate_id, request.grant.into_grant())
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 启动已安装来源的一次 execution，并异步投递唯一事件 wire。
///
/// # Errors
///
/// 来源不存在、intent 不受支持、replay archive 无效或 execution 无法启动时返回安全 IPC 错误。
#[tauri::command]
pub async fn execute(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ExecuteRequest,
) -> Result<ExecuteResponse, String> {
    let (execution_id, events) = start_execution(&state, request).await?;
    let registry = state.cancellations.clone();

    tauri::async_runtime::spawn(async move {
        forward_execution_events(events, execution_id, registry, |payload| {
            app.emit(RULE_EXECUTION_EVENT, payload)
                .map_err(|_| "execution 事件投递失败".to_string())
        })
        .await;
    });

    Ok(ExecuteResponse { execution_id })
}

async fn start_execution(
    state: &AppState,
    request: ExecuteRequest,
) -> Result<(ExecutionId, BoxStream<'static, ExecutionEvent>), String> {
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

    Ok((execution_id, session.into_events()))
}

/// 幂等请求取消一个仍由本进程投递的 execution。
///
/// # Errors
///
/// 取消注册表不可用时返回安全 IPC 错误。已结束、未知或已取消的 execution 返回 `changed = false`。
#[tauri::command]
pub fn cancel_execution(
    state: State<'_, AppState>,
    request: CancelExecutionRequest,
) -> Result<CancelExecutionResponse, String> {
    let changed = request_cancellation(state, request.execution_id)?;
    Ok(CancelExecutionResponse {
        execution_id: request.execution_id,
        changed,
    })
}

fn request_cancellation(
    state: impl std::ops::Deref<Target = AppState>,
    execution_id: ExecutionId,
) -> Result<bool, String> {
    let registry = state
        .cancellations
        .lock()
        .map_err(|_| "execution 取消注册表不可用".to_string())?;
    Ok(registry
        .get(&execution_id)
        .is_some_and(ExecutionCancellation::cancel))
}

/// 依照持久 sequence 补发一个 execution 的事件。
///
/// 每个补读事件仍使用唯一 `rule-execution-event` payload；命令只返回 delivery 摘要。
///
/// # Errors
///
/// execution 不存在、sequence 不连续或持久读取失败时返回安全 IPC 错误。
#[tauri::command]
pub async fn catch_up_execution(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CatchUpExecutionRequest,
) -> Result<CatchUpExecutionResponse, String> {
    catch_up_execution_inner(&state, request, |payload| {
        app.emit(RULE_EXECUTION_EVENT, payload)
            .map_err(|_| "execution 事件投递失败".to_string())
    })
    .await
}

async fn catch_up_execution_inner<F>(
    state: &AppState,
    request: CatchUpExecutionRequest,
    emit: F,
) -> Result<CatchUpExecutionResponse, String>
where
    F: FnMut(&RuleExecutionEvent) -> Result<(), String> + Send,
{
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

    emit_catch_up_events(events, emit)?;
    if observed_terminal {
        remove_cancellation(&state.cancellations, request.execution_id);
    }

    Ok(CatchUpExecutionResponse {
        execution_id: request.execution_id,
        replayed_count,
        delivered_through_sequence,
    })
}

/// 返回已安装来源的安全投影列表。
///
/// # Errors
///
/// 查询投影失败时返回安全 IPC 错误。
#[tauri::command]
pub async fn list_installed_sources(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledSource>, String> {
    list_installed_sources_inner(state.system.as_ref()).await
}

async fn list_installed_sources_inner(system: &RuleSystem) -> Result<Vec<InstalledSource>, String> {
    system
        .list_installed_sources()
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 返回共享资料库的安全规范化投影。
///
/// # Errors
///
/// 查询投影失败时返回安全 IPC 错误。
#[tauri::command]
pub async fn get_library_projection(
    state: State<'_, AppState>,
) -> Result<LibraryProjection, String> {
    get_library_projection_inner(state.system.as_ref()).await
}

async fn get_library_projection_inner(system: &RuleSystem) -> Result<LibraryProjection, String> {
    system
        .get_library_projection()
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 以 library stream 写入用户资料库状态。
///
/// # Errors
///
/// 请求无效、资源不存在或写入投影失败时返回安全 IPC 错误。
#[tauri::command]
pub async fn update_library_entry(
    state: State<'_, AppState>,
    request: LibraryEntryUpdate,
) -> Result<LibraryUpdateReceipt, String> {
    update_library_entry_inner(state.system.as_ref(), request).await
}

async fn update_library_entry_inner(
    system: &RuleSystem,
    request: LibraryEntryUpdate,
) -> Result<LibraryUpdateReceipt, String> {
    system
        .update_library_entry(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

fn emit_catch_up_events<F>(events: Vec<ExecutionEvent>, mut emit: F) -> Result<(), String>
where
    F: FnMut(&RuleExecutionEvent) -> Result<(), String> + Send,
{
    for event in events {
        emit(&RuleExecutionEvent::from(event))?;
    }
    Ok(())
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
    match registry.lock() {
        Ok(mut entries) => {
            entries.remove(&execution_id);
        }
        Err(_) => {
            tracing::warn!(?execution_id, "execution 终态后无法清理取消注册表");
        }
    }
}

fn map_rule_error(error: &RuleError) -> String {
    format!(
        "{}: {} (trace_id={})",
        error.code, error.message, error.trace_id
    )
}

#[cfg(test)]
mod tests {
    //! Root delivery adapter 的真实 `RuleSystem` 命令合同。

    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, Once};
    use std::time::Duration;

    use keyring_core::{mock, set_default_store};
    use lj_rule_system::RuleSystemConfig;
    use serde_json::json;
    use uuid::Uuid;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    struct TempRoot {
        root: PathBuf,
        keyring_service: String,
    }

    impl TempRoot {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("lanjing-root-command-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).expect("创建 root command 测试目录");
            Self {
                root,
                keyring_service: format!("lanjing.root.command.{}", Uuid::new_v4()),
            }
        }

        async fn open(&self) -> Arc<RuleSystem> {
            Arc::new(
                RuleSystem::open(
                    RuleSystemConfig::local_fixture(
                        self.root.join("event-store.db"),
                        self.root.join("artifacts"),
                    )
                    .with_keyring_service(self.keyring_service.clone()),
                )
                .await
                .expect("打开 RuleSystem fixture"),
            )
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn init_mock_keyring() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            set_default_store(mock::Store::new().expect("keyring-core mock store"));
        });
    }

    async fn mount_slow_discover_route(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/api.php/provide/vod/"))
            .and(query_param("ac", "list"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(300))
                    .set_body_json(json!({
                        "code": 1,
                        "page": 1,
                        "pagecount": 1,
                        "limit": 20,
                        "total": 1,
                        "list": [{
                            "vod_id": 140_789,
                            "vod_name": "取消测试资源",
                            "vod_pic": "/covers/140789.jpg",
                            "type_name": "测试",
                            "vod_remarks": "第 1 集"
                        }]
                    })),
            )
            .mount(server)
            .await;
    }

    async fn assert_root_query_commands(system: &RuleSystem, installed: &InstalledSource) {
        let installed_sources = list_installed_sources_inner(system)
            .await
            .expect("list_installed_sources 应返回安装摘要");
        assert_eq!(installed_sources.len(), 1, "安装后来源投影应只有一个条目");
        assert_eq!(installed_sources[0].source_id, installed.source_id);

        let resource_id = serde_json::to_value(installed)
            .expect("InstalledSource 应可序列化")
            .get("profile")
            .and_then(|profile| profile.get("id"))
            .and_then(serde_json::Value::as_str)
            .expect("来源资料必须包含稳定资源 ID")
            .to_string();
        let initial_library = get_library_projection_inner(system)
            .await
            .expect("get_library_projection 应返回安全快照");
        let update = serde_json::from_value::<LibraryEntryUpdate>(json!({
            "resource_id": resource_id.clone(),
            "favorite": true,
            "pinned": true,
            "last_opened_at": "2026-07-18T00:00:00Z",
            "progress": { "unit_id": null, "position": 42, "total": null },
            "expected_version": 0
        }))
        .expect("稳定 library update wire 应反序列化");
        let receipt = update_library_entry_inner(system, update)
            .await
            .expect("update_library_entry 应提交 library stream");
        assert!(
            receipt.global_seq > initial_library.global_seq,
            "library 写入必须推进全局序号"
        );
        assert_eq!(
            receipt.revision, 1,
            "新 library 条目 revision 必须从 1 开始"
        );
        let library = get_library_projection_inner(system)
            .await
            .expect("更新后应能读取 library 投影");
        let entry = library
            .entries
            .iter()
            .find(|entry| entry.resource_id == resource_id)
            .expect("更新后的 library 条目必须进入投影");
        assert!(entry.favorite && entry.pinned, "library 布尔状态必须投影");
        assert_eq!(
            entry.progress.as_ref().map(|progress| progress.position),
            Some(42),
            "library 进度必须投影"
        );
    }

    async fn assert_mid_execution_catch_up_keeps_cancellation(
        state: &AppState,
        execution_id: ExecutionId,
    ) {
        let replayed = Arc::new(Mutex::new(Vec::<RuleExecutionEvent>::new()));
        let replayed_for_emit = replayed.clone();
        let catch_up = catch_up_execution_inner(
            state,
            CatchUpExecutionRequest {
                execution_id,
                after_sequence: 0,
            },
            move |payload| {
                replayed_for_emit
                    .lock()
                    .expect("mid-execution catch-up 收集锁")
                    .push(payload.clone());
                Ok(())
            },
        )
        .await
        .expect("活跃 execution 的 catch-up 应成功");
        assert!(
            catch_up.replayed_count > 0,
            "execution start 已 durable，mid-execution catch-up 必须有事件"
        );
        assert_eq!(
            replayed
                .lock()
                .expect("读取 mid-execution catch-up 收集锁")
                .len(),
            catch_up.replayed_count,
            "catch-up 必须投递全部已持久事件"
        );
        let delivery_error = catch_up_execution_inner(
            state,
            CatchUpExecutionRequest {
                execution_id,
                after_sequence: 0,
            },
            |_| Err("模拟 catch-up 事件投递失败".to_string()),
        )
        .await
        .expect_err("catch-up 事件投递失败必须返回 IPC 错误");
        assert_eq!(delivery_error, "模拟 catch-up 事件投递失败");
        assert!(
            state
                .cancellations
                .lock()
                .expect("读取活跃取消注册表")
                .contains_key(&execution_id),
            "有限 catch-up stream 结束不得移除活跃 execution 的 cancellation"
        );
    }

    async fn assert_nonterminal_delivery_eof_keeps_cancellation(
        state: &AppState,
        execution_id: ExecutionId,
    ) {
        forward_execution_events(
            futures::stream::empty::<ExecutionEvent>().boxed(),
            execution_id,
            state.cancellations.clone(),
            |_| Ok(()),
        )
        .await;
        assert!(
            state
                .cancellations
                .lock()
                .expect("读取 nonterminal EOF 取消注册表")
                .contains_key(&execution_id),
            "nonterminal delivery EOF 不得移除活跃 execution 的 cancellation"
        );
    }

    /// prepare、install、execute、唯一事件、取消和 sequence catch-up 必须穿过 root adapter。
    #[tokio::test]
    async fn test_root_command_lifecycle_delivers_cancel_and_catch_up() {
        init_mock_keyring();
        let fixture = TempRoot::new();
        let server = MockServer::start().await;
        mount_slow_discover_route(&server).await;

        let system = fixture.open().await;
        let state = AppState::new(system.clone());
        let candidate = prepare_install_inner(
            system.as_ref(),
            RuleInput::MaccmsJson {
                url: format!("{}/api.php/provide/vod/", server.uri()),
            },
        )
        .await
        .expect("prepare_install 应返回安全 candidate");
        let installed = install_inner(
            &state,
            InstallRequest {
                candidate_id: candidate.id,
                grant: CapabilityGrantPreset::NetworkOnly,
            },
        )
        .await
        .expect("install 应安装 candidate");
        assert_root_query_commands(system.as_ref(), &installed).await;
        let request = serde_json::from_value::<ExecuteRequest>(json!({
            "source_id": installed.source_id,
            "intent": "Discover",
            "input": { "type": "None" },
            "mode": { "mode": "live" }
        }))
        .expect("稳定 execute wire 应反序列化");
        let (execution_id, events) = start_execution(&state, request)
            .await
            .expect("execute 应创建 delivery session");
        assert_mid_execution_catch_up_keeps_cancellation(&state, execution_id).await;
        assert_nonterminal_delivery_eof_keeps_cancellation(&state, execution_id).await;

        let delivered = Arc::new(Mutex::new(Vec::<RuleExecutionEvent>::new()));
        let delivered_for_task = delivered.clone();
        let delivery = tokio::spawn(forward_execution_events(
            events,
            execution_id,
            state.cancellations.clone(),
            move |payload| {
                delivered_for_task
                    .lock()
                    .expect("delivery 收集锁")
                    .push(payload.clone());
                Ok(())
            },
        ));

        assert!(
            request_cancellation(&state, execution_id).expect("首次取消应读取注册表"),
            "首次取消必须改变 execution 状态"
        );
        assert!(
            !request_cancellation(&state, execution_id).expect("重复取消应读取注册表"),
            "重复取消必须幂等"
        );
        tokio::time::timeout(Duration::from_secs(2), delivery)
            .await
            .expect("取消后的终态必须及时投递")
            .expect("delivery task 不应 panic");

        let delivered = delivered.lock().expect("读取 delivery 收集锁").clone();
        assert!(!delivered.is_empty(), "execution 必须投递至少一个事件");
        assert!(
            delivered
                .iter()
                .enumerate()
                .all(|(index, event)| event.sequence == (index as u64) + 1),
            "delivery sequence 必须连续: {delivered:?}"
        );
        assert!(
            delivered
                .iter()
                .all(|event| event.execution_id == execution_id),
            "所有事件必须属于本次 execution"
        );
        assert!(
            matches!(
                delivered.last().map(|event| &event.kind),
                Some(ExecutionEventKind::Cancelled)
            ),
            "取消 execution 必须以唯一 Cancelled 终态投递: {delivered:?}"
        );
        let first_wire = serde_json::to_value(&delivered[0]).expect("event wire 必须可序列化");
        for field in [
            "execution_id",
            "sequence",
            "trace_id",
            "occurred_at_ms",
            "kind",
        ] {
            assert!(
                first_wire.get(field).is_some(),
                "唯一事件 wire 缺少字段 {field}: {first_wire}"
            );
        }
        assert!(
            !state
                .cancellations
                .lock()
                .expect("读取终态注册表")
                .contains_key(&execution_id),
            "终态后必须清理 cancellation registry"
        );

        let replayed = Arc::new(Mutex::new(Vec::<RuleExecutionEvent>::new()));
        let replayed_for_emit = replayed.clone();
        let catch_up = catch_up_execution_inner(
            &state,
            CatchUpExecutionRequest {
                execution_id,
                after_sequence: 0,
            },
            move |payload| {
                replayed_for_emit
                    .lock()
                    .expect("catch-up 收集锁")
                    .push(payload.clone());
                Ok(())
            },
        )
        .await
        .expect("catch-up 应补发持久事件");
        let replayed = replayed.lock().expect("读取 catch-up 收集锁").clone();
        assert_eq!(catch_up.replayed_count, delivered.len());
        assert_eq!(
            catch_up.delivered_through_sequence,
            delivered.last().expect("delivery 非空").sequence
        );
        assert_eq!(replayed, delivered, "catch-up 必须按持久 sequence 原样补发");

        drop(state);
        system
            .shutdown_for_test()
            .await
            .expect("测试结束应关闭 RuleSystem writer");
    }
}
