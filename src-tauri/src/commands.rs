//! Tauri IPC 的 `RuleSystem` delivery、查询、取消与导入 payload 拉取边界。
//!
//! 规则生命周期、执行与投影委托给 `RuleSystem`；深链 JSON 拉取仅委托给 `lj-node-http`
//! 安全 façade。这里不组装 Definition、Plan、storage 或任何 effect handler。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::stream::BoxStream;
use lj_rule_system::{
    CandidateId, CapabilityGrant, ClearSourceDocumentCredentialRequest,
    CreateSourceDocumentRequest, DeleteSourceDocumentRequest, DocumentMutationOutcome, DocumentRef,
    ExecuteRequest, ExecutionCancellation, ExecutionEvent, ExecutionEventKind, ExecutionId,
    GetSourceDocumentRequest, InstallCandidate, InstalledSource, LibraryEntryUpdate,
    LibraryProjection, LibraryUpdateReceipt, ListSourceDocumentsRequest, MaskedSourceDocument,
    MediaAssetPage, MediaItem, MediaUnitPage, PinSourceDocumentRevisionRequest,
    RebaseSourceDocumentRequest, ReleaseSourceDocumentRevisionPinRequest,
    RenameSourceDocumentRequest, ReplaceSourceDocumentCredentialRequest,
    RevealSourceDocumentCredentialRequest, RuleError, RuleInput, RuleSystem,
    SaveSourceDocumentRequest, SourceDocumentCredentialReveal, SourceDocumentRevisionPin,
    SourceDocumentRevisionPinReleaseOutcome, SourceDocumentSummary,
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

/// 深链导入地址拉取请求。
#[derive(Debug, Deserialize)]
pub struct FetchImportSrcRequest {
    /// 仅允许不含用户凭据的 HTTP(S) URL。
    pub url: String,
}

/// 安装请求：opaque candidate 与用户批准的固定 capability 预设。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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

/// 通过 `lj-node-http` 安全边界拉取导入预览文本。
///
/// # Errors
///
/// URL/SSRF/redirect 校验、30 秒总超时、非 2xx、2 MiB body 上限、响应读取或 UTF-8
/// 校验失败时返回不含 URL query、响应 body 与底层网络详情的安全字符串。
#[tauri::command]
pub async fn fetch_import_src(request: FetchImportSrcRequest) -> Result<String, String> {
    lj_node_http::fetch_import_source(&request.url)
        .await
        .map_err(|error| error.to_string())
}

/// 列出来源文档的安全摘要；即使空请求也保留统一 `{ request }` wire。
///
/// # Errors
///
/// 投影读取失败时返回经 `RuleSystem` 收敛的安全 IPC 错误。
#[tauri::command]
pub async fn list_source_documents(
    state: State<'_, AppState>,
    request: ListSourceDocumentsRequest,
) -> Result<Vec<SourceDocumentSummary>, String> {
    list_source_documents_inner(state.system.as_ref(), request).await
}

async fn list_source_documents_inner(
    system: &RuleSystem,
    request: ListSourceDocumentsRequest,
) -> Result<Vec<SourceDocumentSummary>, String> {
    system
        .list_source_documents(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 创建并加密保存来源文档 revision `1`。
///
/// # Errors
///
/// façade 初始化、codec 或 storage 边界发生非预期失败时返回安全 IPC 错误；预期校验与
/// keyring 状态保留在 tagged [`DocumentMutationOutcome`] 中。
#[tauri::command]
pub async fn create_source_document(
    state: State<'_, AppState>,
    request: CreateSourceDocumentRequest,
) -> Result<DocumentMutationOutcome, String> {
    create_source_document_inner(state.system.as_ref(), request).await
}

async fn create_source_document_inner(
    system: &RuleSystem,
    request: CreateSourceDocumentRequest,
) -> Result<DocumentMutationOutcome, String> {
    system
        .create_source_document(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 默认读取一个只含 masked JSON 与安全 slot 摘要的来源文档。
///
/// # Errors
///
/// 参数或投影读取失败时返回经 `RuleSystem` 收敛的安全 IPC 错误。
#[tauri::command]
pub async fn get_source_document(
    state: State<'_, AppState>,
    request: GetSourceDocumentRequest,
) -> Result<Option<MaskedSourceDocument>, String> {
    get_source_document_inner(state.system.as_ref(), request).await
}

async fn get_source_document_inner(
    system: &RuleSystem,
    request: GetSourceDocumentRequest,
) -> Result<Option<MaskedSourceDocument>, String> {
    system
        .get_source_document(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 以 expected revision 保存 masked 编辑文本。
///
/// # Errors
///
/// façade 或 storage 发生非预期失败时返回安全 IPC 错误；冲突、校验和 keyring 状态通过
/// tagged [`DocumentMutationOutcome`] 返回。
#[tauri::command]
pub async fn save_source_document(
    state: State<'_, AppState>,
    request: SaveSourceDocumentRequest,
) -> Result<DocumentMutationOutcome, String> {
    save_source_document_inner(state.system.as_ref(), request).await
}

async fn save_source_document_inner(
    system: &RuleSystem,
    request: SaveSourceDocumentRequest,
) -> Result<DocumentMutationOutcome, String> {
    system
        .save_source_document(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 固定一个 current saved revision 的 secret refs，供 conflict rebase 使用。
///
/// # Errors
///
/// document 缺失、revision 漂移或 storage owner/密文验证失败时返回安全 IPC 错误。
#[tauri::command]
pub async fn pin_source_document_revision(
    state: State<'_, AppState>,
    request: PinSourceDocumentRevisionRequest,
) -> Result<SourceDocumentRevisionPin, String> {
    pin_source_document_revision_inner(state.system.as_ref(), request).await
}

async fn pin_source_document_revision_inner(
    system: &RuleSystem,
    request: PinSourceDocumentRevisionRequest,
) -> Result<SourceDocumentRevisionPin, String> {
    system
        .pin_source_document_revision(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 幂等释放 revision pin；重复或未知 pin 返回同一 released outcome。
///
/// # Errors
///
/// storage owner/ref-count transaction 失败时返回安全 IPC 错误。
#[tauri::command]
pub async fn release_source_document_revision_pin(
    state: State<'_, AppState>,
    request: ReleaseSourceDocumentRevisionPinRequest,
) -> Result<SourceDocumentRevisionPinReleaseOutcome, String> {
    release_source_document_revision_pin_inner(state.system.as_ref(), request).await
}

async fn release_source_document_revision_pin_inner(
    system: &RuleSystem,
    request: ReleaseSourceDocumentRevisionPinRequest,
) -> Result<SourceDocumentRevisionPinReleaseOutcome, String> {
    system
        .release_source_document_revision_pin(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 以 trusted pin/current material 执行 slot-aware merge 或 fork。
///
/// # Errors
///
/// storage 基础设施失败时返回安全 IPC 错误；pin、resolution、codec 与并发失败保留在 tagged
/// [`DocumentMutationOutcome`]。
#[tauri::command]
pub async fn rebase_source_document(
    state: State<'_, AppState>,
    request: RebaseSourceDocumentRequest,
) -> Result<DocumentMutationOutcome, String> {
    rebase_source_document_inner(state.system.as_ref(), request).await
}

async fn rebase_source_document_inner(
    system: &RuleSystem,
    request: RebaseSourceDocumentRequest,
) -> Result<DocumentMutationOutcome, String> {
    system
        .rebase_source_document(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 以 expected revision 重命名来源文档。
///
/// # Errors
///
/// façade 或 storage 发生非预期失败时返回安全 IPC 错误；冲突等预期结果使用 tagged outcome。
#[tauri::command]
pub async fn rename_source_document(
    state: State<'_, AppState>,
    request: RenameSourceDocumentRequest,
) -> Result<DocumentMutationOutcome, String> {
    rename_source_document_inner(state.system.as_ref(), request).await
}

async fn rename_source_document_inner(
    system: &RuleSystem,
    request: RenameSourceDocumentRequest,
) -> Result<DocumentMutationOutcome, String> {
    system
        .rename_source_document(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 删除 expected revision 匹配且尚未关联来源的 draft。
///
/// # Errors
///
/// façade 或 storage 发生非预期失败时返回安全 IPC 错误；冲突、已关联和缺失使用 tagged outcome。
#[tauri::command]
pub async fn delete_source_document(
    state: State<'_, AppState>,
    request: DeleteSourceDocumentRequest,
) -> Result<DocumentMutationOutcome, String> {
    delete_source_document_inner(state.system.as_ref(), request).await
}

async fn delete_source_document_inner(
    system: &RuleSystem,
    request: DeleteSourceDocumentRequest,
) -> Result<DocumentMutationOutcome, String> {
    system
        .delete_source_document(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 显式 reveal 当前 revision 的单个 owner-bound credential。
///
/// # Errors
///
/// 文档、revision、slot ownership 或 keyring 校验失败时返回安全 IPC 错误；不会解析错误消息分支。
#[tauri::command]
pub async fn reveal_source_document_credential(
    state: State<'_, AppState>,
    request: RevealSourceDocumentCredentialRequest,
) -> Result<SourceDocumentCredentialReveal, String> {
    reveal_source_document_credential_inner(state.system.as_ref(), request).await
}

async fn reveal_source_document_credential_inner(
    system: &RuleSystem,
    request: RevealSourceDocumentCredentialRequest,
) -> Result<SourceDocumentCredentialReveal, String> {
    system
        .reveal_source_document_credential(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 替换当前 revision 的单个 owner-bound credential 并保存下一 revision。
///
/// # Errors
///
/// façade 或 storage 发生非预期失败时返回安全 IPC 错误；预期校验与 keyring 状态使用 tagged
/// outcome。
#[tauri::command]
pub async fn replace_source_document_credential(
    state: State<'_, AppState>,
    request: ReplaceSourceDocumentCredentialRequest,
) -> Result<DocumentMutationOutcome, String> {
    replace_source_document_credential_inner(state.system.as_ref(), request).await
}

async fn replace_source_document_credential_inner(
    system: &RuleSystem,
    request: ReplaceSourceDocumentCredentialRequest,
) -> Result<DocumentMutationOutcome, String> {
    system
        .replace_source_document_credential(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 清除当前 revision 的单个 owner-bound credential 并保存下一 revision。
///
/// # Errors
///
/// façade 或 storage 发生非预期失败时返回安全 IPC 错误；预期校验与 keyring 状态使用 tagged
/// outcome。
#[tauri::command]
pub async fn clear_source_document_credential(
    state: State<'_, AppState>,
    request: ClearSourceDocumentCredentialRequest,
) -> Result<DocumentMutationOutcome, String> {
    clear_source_document_credential_inner(state.system.as_ref(), request).await
}

async fn clear_source_document_credential_inner(
    system: &RuleSystem,
    request: ClearSourceDocumentCredentialRequest,
) -> Result<DocumentMutationOutcome, String> {
    system
        .clear_source_document_credential(request)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 从一个已保存的精确 document revision 生成 durable install candidate。
///
/// # Errors
///
/// 文档缺失、revision 过期、解密/codec/compile 或 candidate staging 失败时返回安全 IPC 错误。
#[tauri::command]
pub async fn prepare_install_from_document(
    state: State<'_, AppState>,
    request: DocumentRef,
) -> Result<InstallCandidate, String> {
    prepare_install_from_document_inner(state.system.as_ref(), request).await
}

async fn prepare_install_from_document_inner(
    system: &RuleSystem,
    request: DocumentRef,
) -> Result<InstallCandidate, String> {
    system
        .prepare_install_from_document(request)
        .await
        .map_err(|error| map_rule_error(&error))
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

/// 按稳定 item ID 取标准媒体摘要；缺失时返回 JSON `null`。
#[derive(Debug, Deserialize)]
pub struct GetMediaItemRequest {
    /// 标准媒体主体稳定 ID（通常来自 library `resource_id`）。
    pub resource_id: String,
}

/// 批量按稳定 item ID 取标准媒体摘要。
#[derive(Debug, Deserialize)]
pub struct GetMediaItemsRequest {
    /// 1..=64 个非空稳定 ID。
    pub resource_ids: Vec<String>,
}

/// 有界列出 item 下消费单元。
#[derive(Debug, Deserialize)]
pub struct ListMediaUnitsRequest {
    /// 父 item 稳定 ID。
    pub item_id: String,
    /// 分页 offset；默认 0。
    #[serde(default)]
    pub offset: u32,
    /// 分页 limit；省略时 façade 使用默认 50，硬上限 100。
    pub limit: Option<u32>,
}

/// 有界列出 unit 下资产。
#[derive(Debug, Deserialize)]
pub struct ListMediaAssetsRequest {
    /// 父 unit 稳定 ID。
    pub unit_id: String,
    /// 分页 offset；默认 0。
    #[serde(default)]
    pub offset: u32,
    /// 分页 limit；省略时 façade 使用默认 50，硬上限 100。
    pub limit: Option<u32>,
}

/// 按稳定 ID 读取标准媒体主体；不存在时 `Ok(None)` → JSON `null`。
///
/// # Errors
///
/// 参数非法或投影读取失败时返回安全 IPC 错误。
#[tauri::command]
pub async fn get_media_item(
    state: State<'_, AppState>,
    request: GetMediaItemRequest,
) -> Result<Option<MediaItem>, String> {
    get_media_item_inner(state.system.as_ref(), request).await
}

async fn get_media_item_inner(
    system: &RuleSystem,
    request: GetMediaItemRequest,
) -> Result<Option<MediaItem>, String> {
    system
        .get_media_item(request.resource_id)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 批量读取标准媒体主体；跳过缺失 ID。
///
/// # Errors
///
/// 参数非法或投影读取失败时返回安全 IPC 错误。
#[tauri::command]
pub async fn get_media_items(
    state: State<'_, AppState>,
    request: GetMediaItemsRequest,
) -> Result<Vec<MediaItem>, String> {
    get_media_items_inner(state.system.as_ref(), request).await
}

async fn get_media_items_inner(
    system: &RuleSystem,
    request: GetMediaItemsRequest,
) -> Result<Vec<MediaItem>, String> {
    system
        .get_media_items(request.resource_ids)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 有界列出目录单元。
///
/// # Errors
///
/// 参数非法或投影读取失败时返回安全 IPC 错误。
#[tauri::command]
pub async fn list_media_units(
    state: State<'_, AppState>,
    request: ListMediaUnitsRequest,
) -> Result<MediaUnitPage, String> {
    list_media_units_inner(state.system.as_ref(), request).await
}

async fn list_media_units_inner(
    system: &RuleSystem,
    request: ListMediaUnitsRequest,
) -> Result<MediaUnitPage, String> {
    system
        .list_media_units(request.item_id, request.offset, request.limit)
        .await
        .map_err(|error| map_rule_error(&error))
}

/// 有界列出单元资产（正文/封面/流 locator）。
///
/// # Errors
///
/// 参数非法或投影读取失败时返回安全 IPC 错误。
#[tauri::command]
pub async fn list_media_assets(
    state: State<'_, AppState>,
    request: ListMediaAssetsRequest,
) -> Result<MediaAssetPage, String> {
    list_media_assets_inner(state.system.as_ref(), request).await
}

async fn list_media_assets_inner(
    system: &RuleSystem,
    request: ListMediaAssetsRequest,
) -> Result<MediaAssetPage, String> {
    system
        .list_media_assets(request.unit_id, request.offset, request.limit)
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

    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, Once};
    use std::time::Duration;

    use keyring_core::{mock, set_default_store};
    use lj_rule_system::{
        DocumentValidationIssue, RuleSystemConfig, SourceDocumentCredentialTarget,
    };
    use serde_json::{Value, json};
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

    const TEST_CREDENTIAL: &str = "Bearer root-vault-command-secret";

    fn credential_legado_source(title: &str) -> String {
        credential_legado_source_with_value(title, TEST_CREDENTIAL)
    }

    fn credential_legado_source_with_value(title: &str, credential: &str) -> String {
        let mut source = legado_source_value(title);
        source["header"] = Value::String(format!("{{\"Authorization\":\"{credential}\"}}"));
        serde_json::to_string(&source).expect("序列化 credential Legado fixture")
    }

    fn legado_source_without_header(title: &str) -> String {
        let mut source = legado_source_value(title);
        source
            .as_object_mut()
            .expect("Legado fixture root 应为 object")
            .remove("header");
        serde_json::to_string(&source).expect("序列化缺少 header 的 Legado fixture")
    }

    fn legado_source_value(title: &str) -> Value {
        let mut source = serde_json::from_str::<Value>(include_str!(
            "../crates/lj-importer/fixtures/legado_synthetic_source.json"
        ))
        .expect("解析合成 Legado fixture");
        source["bookSourceName"] = Value::String(title.to_string());
        source
            .as_object_mut()
            .expect("Legado fixture root 应为 object")
            .remove("ruleBookInfo");
        source["enabledCookieJar"] = Value::Bool(false);
        source
    }

    fn expect_saved_document(
        outcome: DocumentMutationOutcome,
        operation: &str,
    ) -> MaskedSourceDocument {
        match outcome {
            DocumentMutationOutcome::Saved {
                document: Some(document),
            } => document,
            other => panic!("{operation} 应返回带 masked document 的 saved outcome: {other:?}"),
        }
    }

    fn assert_invalid_code(outcome: DocumentMutationOutcome, expected_code: &str) {
        let DocumentMutationOutcome::Invalid { issues } = outcome else {
            panic!("应返回 invalid outcome");
        };
        assert!(
            issues.iter().any(|issue| issue.code == expected_code),
            "invalid outcome 缺少稳定 code {expected_code}"
        );
    }

    fn credential_target(document: &MaskedSourceDocument) -> SourceDocumentCredentialTarget {
        let slot = document
            .credential_slots
            .first()
            .expect("credential fixture 必须生成一个 slot");
        SourceDocumentCredentialTarget {
            document_id: document.summary.document_id,
            document_revision: document.summary.revision,
            slot_id: slot.slot_id,
        }
    }

    struct RebaseConflictFixture {
        base: MaskedSourceDocument,
        current: MaskedSourceDocument,
        pin: SourceDocumentRevisionPin,
    }

    async fn create_rebase_conflict(
        system: &RuleSystem,
        title: &str,
        base_value: &str,
        current_value: &str,
    ) -> RebaseConflictFixture {
        let create = serde_json::from_value::<CreateSourceDocumentRequest>(json!({
            "format": "legado",
            "title": title,
            "text": credential_legado_source_with_value(title, base_value)
        }))
        .expect("rebase create wire");
        let base = expect_saved_document(
            create_source_document_inner(system, create)
                .await
                .expect("create rebase base"),
            "create rebase base",
        );
        let pin = pin_source_document_revision_inner(
            system,
            PinSourceDocumentRevisionRequest {
                document_id: base.summary.document_id,
                document_revision: base.summary.revision,
            },
        )
        .await
        .expect("pin rebase base");
        let replace = serde_json::from_value::<ReplaceSourceDocumentCredentialRequest>(json!({
            "target": credential_target(&base),
            "value": format!("{{\"Authorization\":\"{current_value}\"}}")
        }))
        .expect("current replacement wire");
        let current = expect_saved_document(
            replace_source_document_credential_inner(system, replace)
                .await
                .expect("advance conflicting current"),
            "advance conflicting current",
        );
        RebaseConflictFixture { base, current, pin }
    }

    fn rebase_merge_request(
        conflict: &RebaseConflictFixture,
        document_id: lj_rule_system::SourceDocumentId,
        base_revision: u64,
        current_revision: u64,
        local_masked_text: &str,
        credential_resolutions: &Value,
    ) -> RebaseSourceDocumentRequest {
        serde_json::from_value(json!({
            "pin_id": conflict.pin.pin_id,
            "document_id": document_id,
            "base_revision": base_revision,
            "current_revision": current_revision,
            "local_masked_text": local_masked_text,
            "mode": { "kind": "merge" },
            "credential_resolutions": credential_resolutions
        }))
        .expect("merge rebase request wire")
    }

    async fn assert_rebase_merge_case(
        system: &RuleSystem,
        label: &str,
        action: Value,
        expected_value: Option<String>,
    ) {
        let base_value = "Bearer rebase-base";
        let current_value = "Bearer rebase-current";
        let title = format!("rebase-{label}");
        let conflict = create_rebase_conflict(system, &title, base_value, current_value).await;
        let local_title = format!("{title}-local-layout");
        let local_masked_text = conflict.base.masked_text.replacen(&title, &local_title, 1);
        let request = serde_json::from_value::<RebaseSourceDocumentRequest>(json!({
            "pin_id": conflict.pin.pin_id,
            "document_id": conflict.base.summary.document_id,
            "base_revision": conflict.base.summary.revision,
            "current_revision": conflict.current.summary.revision,
            "local_masked_text": local_masked_text,
            "mode": { "kind": "merge" },
            "credential_resolutions": [{ "path": "/header", "action": action }]
        }))
        .expect("rebase action wire");
        let rebased = expect_saved_document(
            rebase_source_document_inner(system, request)
                .await
                .expect("merge rebase outcome"),
            "merge rebase",
        );
        assert_eq!(
            rebased.summary.document_id,
            conflict.base.summary.document_id
        );
        assert_eq!(
            rebased.summary.revision,
            conflict.current.summary.revision + 1
        );
        assert!(rebased.masked_text.contains(&local_title));
        if let Some(expected_value) = expected_value {
            assert_default_document_wire_is_safe(
                &rebased,
                &[base_value, current_value, "Bearer rebase-replacement"],
            );
            let rebased_target = credential_target(&rebased);
            assert_ne!(
                rebased_target.slot_id,
                credential_target(&conflict.base).slot_id
            );
            assert_ne!(
                rebased_target.slot_id,
                credential_target(&conflict.current).slot_id
            );
            let revealed = reveal_source_document_credential_inner(
                system,
                RevealSourceDocumentCredentialRequest {
                    target: rebased_target,
                },
            )
            .await
            .expect("reveal rebased credential");
            assert_eq!(revealed.value, expected_value);
        } else {
            let wire = serde_json::to_value(&rebased).expect("cleared rebase wire");
            let encoded = wire.to_string();
            for plaintext in [base_value, current_value, "Bearer rebase-replacement"] {
                assert!(!encoded.contains(plaintext));
            }
            assert_no_forbidden_default_fields(&wire);
            assert!(rebased.credential_slots.is_empty());
            assert!(
                !rebased
                    .masked_text
                    .contains("__LANJING_CREDENTIAL_SLOT_V1__:")
            );
        }
        for _ in 0..2 {
            let released = release_source_document_revision_pin_inner(
                system,
                ReleaseSourceDocumentRevisionPinRequest {
                    pin_id: conflict.pin.pin_id,
                },
            )
            .await
            .expect("pin release is idempotent");
            assert_eq!(
                serde_json::to_value(released).expect("release wire"),
                json!({ "status": "released", "pin_id": conflict.pin.pin_id })
            );
        }
    }

    async fn assert_union_path_missing_current_is_explicit(system: &RuleSystem) {
        let title = "rebase-union-missing-current";
        let base = expect_saved_document(
            create_source_document_inner(
                system,
                serde_json::from_value(json!({
                    "format": "legado",
                    "title": title,
                    "text": credential_legado_source_with_value(title, "Bearer union-base")
                }))
                .expect("union base create wire"),
            )
            .await
            .expect("create union base"),
            "create union base",
        );
        let pin = pin_source_document_revision_inner(
            system,
            PinSourceDocumentRevisionRequest {
                document_id: base.summary.document_id,
                document_revision: base.summary.revision,
            },
        )
        .await
        .expect("pin union base");
        let current = expect_saved_document(
            clear_source_document_credential_inner(
                system,
                ClearSourceDocumentCredentialRequest {
                    target: credential_target(&base),
                },
            )
            .await
            .expect("clear current union credential"),
            "clear current union credential",
        );
        let conflict = RebaseConflictFixture { base, current, pin };
        let missing_current = rebase_merge_request(
            &conflict,
            conflict.base.summary.document_id,
            conflict.base.summary.revision,
            conflict.current.summary.revision,
            &conflict.base.masked_text,
            &json!([{ "path": "/header", "action": { "kind": "keep_current" } }]),
        );
        assert_invalid_code(
            rebase_source_document_inner(system, missing_current)
                .await
                .expect("missing current path is typed"),
            "credential_resolution_current_missing",
        );
        let keep_local = rebase_merge_request(
            &conflict,
            conflict.base.summary.document_id,
            conflict.base.summary.revision,
            conflict.current.summary.revision,
            &conflict.base.masked_text,
            &json!([{ "path": "/header", "action": { "kind": "keep_local" } }]),
        );
        let rebased = expect_saved_document(
            rebase_source_document_inner(system, keep_local)
                .await
                .expect("keep local union outcome"),
            "keep local union",
        );
        let revealed = reveal_source_document_credential_inner(
            system,
            RevealSourceDocumentCredentialRequest {
                target: credential_target(&rebased),
            },
        )
        .await
        .expect("reveal kept union local");
        assert_eq!(revealed.value, "{\"Authorization\":\"Bearer union-base\"}");
        release_source_document_revision_pin_inner(
            system,
            ReleaseSourceDocumentRevisionPinRequest {
                pin_id: conflict.pin.pin_id,
            },
        )
        .await
        .expect("release missing-current union pin");
    }

    async fn assert_union_path_missing_local_is_explicit(system: &RuleSystem) {
        let title = "rebase-union-missing-local";
        let base = expect_saved_document(
            create_source_document_inner(
                system,
                serde_json::from_value(json!({
                    "format": "legado",
                    "title": title,
                    "text": legado_source_without_header(title)
                }))
                .expect("credential-free create wire"),
            )
            .await
            .expect("create credential-free base"),
            "create credential-free base",
        );
        assert!(base.credential_slots.is_empty());
        let pin = pin_source_document_revision_inner(
            system,
            PinSourceDocumentRevisionRequest {
                document_id: base.summary.document_id,
                document_revision: base.summary.revision,
            },
        )
        .await
        .expect("pin credential-free base");
        let current = expect_saved_document(
            save_source_document_inner(
                system,
                SaveSourceDocumentRequest {
                    document_id: base.summary.document_id,
                    expected_revision: base.summary.revision,
                    masked_text: credential_legado_source_with_value(title, "Bearer union-current"),
                },
            )
            .await
            .expect("add current union credential"),
            "add current union credential",
        );
        let conflict = RebaseConflictFixture { base, current, pin };
        let missing_local = rebase_merge_request(
            &conflict,
            conflict.base.summary.document_id,
            conflict.base.summary.revision,
            conflict.current.summary.revision,
            &conflict.base.masked_text,
            &json!([{ "path": "/header", "action": { "kind": "keep_local" } }]),
        );
        assert_invalid_code(
            rebase_source_document_inner(system, missing_local)
                .await
                .expect("missing local path is typed"),
            "credential_resolution_local_missing",
        );
        let keep_current = rebase_merge_request(
            &conflict,
            conflict.base.summary.document_id,
            conflict.base.summary.revision,
            conflict.current.summary.revision,
            &conflict.base.masked_text,
            &json!([{ "path": "/header", "action": { "kind": "keep_current" } }]),
        );
        let rebased = expect_saved_document(
            rebase_source_document_inner(system, keep_current)
                .await
                .expect("keep current union outcome"),
            "keep current union",
        );
        let revealed = reveal_source_document_credential_inner(
            system,
            RevealSourceDocumentCredentialRequest {
                target: credential_target(&rebased),
            },
        )
        .await
        .expect("reveal kept union current");
        assert_eq!(
            revealed.value,
            "{\"Authorization\":\"Bearer union-current\"}"
        );
        release_source_document_revision_pin_inner(
            system,
            ReleaseSourceDocumentRevisionPinRequest {
                pin_id: conflict.pin.pin_id,
            },
        )
        .await
        .expect("release missing-local union pin");
    }

    fn assert_default_document_wire_is_safe(
        document: &MaskedSourceDocument,
        forbidden_plaintexts: &[&str],
    ) {
        let wire = serde_json::to_value(document).expect("masked document 应可序列化");
        let encoded = wire.to_string();
        for plaintext in forbidden_plaintexts {
            assert!(
                !encoded.contains(plaintext),
                "默认 document wire 不得包含 credential plaintext"
            );
        }
        assert!(
            document
                .masked_text
                .contains("__LANJING_CREDENTIAL_SLOT_V1__:"),
            "credential value 必须被 opaque sentinel 遮罩"
        );
        assert_no_forbidden_default_fields(&wire);
    }

    fn assert_no_forbidden_default_fields(value: &Value) {
        match value {
            Value::Object(fields) => {
                for (name, value) in fields {
                    assert!(
                        !matches!(
                            name.as_str(),
                            "raw_text"
                                | "manifest"
                                | "credentials"
                                | "credential_value"
                                | "secret_artifact_id"
                                | "secret_id"
                                | "artifact_ref"
                                | "artifact_path"
                                | "blob_locator"
                                | "key_id"
                                | "value"
                        ),
                        "默认 document wire 暴露了内部字段 {name}"
                    );
                    assert_no_forbidden_default_fields(value);
                }
            }
            Value::Array(values) => {
                for value in values {
                    assert_no_forbidden_default_fields(value);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn root_command_registry_is_complete_and_keeps_one_execution_event() {
        let actual = crate::REGISTERED_COMMAND_NAMES
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual.len(),
            crate::REGISTERED_COMMAND_NAMES.len(),
            "command registry 不得包含重复名称"
        );
        let expected = [
            "fetch_import_src",
            "list_source_documents",
            "create_source_document",
            "get_source_document",
            "save_source_document",
            "pin_source_document_revision",
            "release_source_document_revision_pin",
            "rebase_source_document",
            "rename_source_document",
            "delete_source_document",
            "reveal_source_document_credential",
            "replace_source_document_credential",
            "clear_source_document_credential",
            "prepare_install_from_document",
            "prepare_install",
            "install",
            "execute",
            "cancel_execution",
            "catch_up_execution",
            "list_installed_sources",
            "get_library_projection",
            "update_library_entry",
            "get_media_item",
            "get_media_items",
            "list_media_units",
            "list_media_assets",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "实际注册集合必须与固定 wire 完全一致");
        assert_eq!(RULE_EXECUTION_EVENT, "rule-execution-event");
    }

    #[test]
    fn document_mutation_outcomes_use_fixed_snake_case_status_tags() {
        for (outcome, status) in [
            (DocumentMutationOutcome::NotFound, "not_found"),
            (DocumentMutationOutcome::Locked, "locked"),
            (DocumentMutationOutcome::KeyUnavailable, "key_unavailable"),
            (DocumentMutationOutcome::KeyLost, "key_lost"),
            (DocumentMutationOutcome::Corrupt, "corrupt"),
        ] {
            let wire = serde_json::to_value(outcome).expect("outcome 应可序列化");
            assert_eq!(wire.get("status").and_then(Value::as_str), Some(status));
        }

        let invalid = DocumentMutationOutcome::Invalid {
            issues: vec![DocumentValidationIssue::new(
                "document_invalid",
                "来源文档无效",
            )],
        };
        let wire = serde_json::to_value(invalid).expect("invalid outcome 应可序列化");
        assert_eq!(wire["status"], "invalid");
        assert_eq!(wire["issues"][0]["code"], "document_invalid");
    }

    #[test]
    fn revision_pin_release_and_rebase_wire_are_fixed_and_strict() {
        let document_id = lj_rule_system::SourceDocumentId::new();
        let pin_id = Uuid::new_v4();
        let pin = SourceDocumentRevisionPin {
            pin_id,
            document_id,
            document_revision: 7,
            expires_at_ms: 42,
        };
        assert_eq!(
            serde_json::to_value(pin).expect("pin response wire"),
            json!({
                "pin_id": pin_id,
                "document_id": document_id,
                "document_revision": 7,
                "expires_at_ms": 42
            })
        );
        let released = SourceDocumentRevisionPinReleaseOutcome::Released { pin_id };
        assert_eq!(
            serde_json::to_value(released).expect("release outcome wire"),
            json!({ "status": "released", "pin_id": pin_id })
        );

        let request = serde_json::from_value::<RebaseSourceDocumentRequest>(json!({
            "pin_id": pin_id,
            "document_id": document_id,
            "base_revision": 7,
            "current_revision": 9,
            "local_masked_text": "{}",
            "mode": { "kind": "fork", "title": "forked" },
            "credential_resolutions": [
                { "path": "/a", "action": { "kind": "keep_current" } },
                { "path": "/b", "action": { "kind": "keep_local" } },
                { "path": "/c", "action": { "kind": "replace", "value": "wire-secret" } },
                { "path": "/d", "action": { "kind": "clear" } }
            ]
        }))
        .expect("strict snake_case rebase wire");
        assert_eq!(request.base_revision, 7);
        assert_eq!(request.current_revision, 9);
        assert!(matches!(
            &request.mode,
            lj_rule_system::SourceDocumentRebaseMode::Fork { title } if title == "forked"
        ));
        assert!(matches!(
            &request.credential_resolutions[0].action,
            lj_rule_system::SourceDocumentCredentialAction::KeepCurrent
        ));
        assert!(matches!(
            &request.credential_resolutions[1].action,
            lj_rule_system::SourceDocumentCredentialAction::KeepLocal
        ));
        assert!(matches!(
            &request.credential_resolutions[2].action,
            lj_rule_system::SourceDocumentCredentialAction::Replace { value }
                if value == "wire-secret"
        ));
        assert!(matches!(
            &request.credential_resolutions[3].action,
            lj_rule_system::SourceDocumentCredentialAction::Clear
        ));
        assert!(
            serde_json::from_value::<PinSourceDocumentRevisionRequest>(json!({
                "document_id": document_id,
                "document_revision": 7,
                "unexpected": true
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<DocumentRef>(json!({
                "document_id": document_id,
                "document_revision": 7,
                "unexpected": true
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<SourceDocumentCredentialTarget>(json!({
                "document_id": document_id,
                "document_revision": 7,
                "slot_id": Uuid::new_v4(),
                "unexpected": true
            }))
            .is_err()
        );

        assert!(
            serde_json::from_value::<RebaseSourceDocumentRequest>(json!({
                "pin_id": pin_id,
                "document_id": document_id,
                "base_revision": 7,
                "current_revision": 9,
                "local_masked_text": "{}",
                "mode": { "kind": "merge", "unexpected": true },
                "credential_resolutions": []
            }))
            .is_err()
        );
    }

    #[tokio::test]
    async fn rebase_merge_applies_all_credential_actions_and_resigns_sentinels() {
        init_mock_keyring();
        let fixture = TempRoot::new();
        let system = fixture.open().await;
        let base_header = "{\"Authorization\":\"Bearer rebase-base\"}";
        let current_header = "{\"Authorization\":\"Bearer rebase-current\"}";
        let replacement_header = "{\"Authorization\":\"Bearer rebase-replacement\"}";
        let cases = vec![
            (
                "keep-current",
                json!({ "kind": "keep_current" }),
                Some(current_header.to_string()),
            ),
            (
                "keep-local",
                json!({ "kind": "keep_local" }),
                Some(base_header.to_string()),
            ),
            (
                "replace",
                json!({ "kind": "replace", "value": replacement_header }),
                Some(replacement_header.to_string()),
            ),
            ("clear", json!({ "kind": "clear" }), None),
        ];
        for (label, action, expected_value) in cases {
            assert_rebase_merge_case(system.as_ref(), label, action, expected_value).await;
        }
    }

    #[tokio::test]
    async fn rebase_requires_one_resolution_for_each_base_current_union_path() {
        init_mock_keyring();
        let fixture = TempRoot::new();
        let system = fixture.open().await;
        assert_union_path_missing_current_is_explicit(system.as_ref()).await;
        assert_union_path_missing_local_is_explicit(system.as_ref()).await;
    }

    #[tokio::test]
    async fn rebase_current_only_header_actions_use_real_legado_codec_path() {
        init_mock_keyring();
        let fixture = TempRoot::new();
        let system = fixture.open().await;
        let current_header = r#"{"Authorization":"Bearer current-only-current"}"#;
        let replacement_header = r#"{"Authorization":"Bearer current-only-replacement"}"#;
        let cases = vec![
            (
                "keep-current",
                json!({ "kind": "keep_current" }),
                Some(current_header),
            ),
            (
                "replace",
                json!({ "kind": "replace", "value": replacement_header }),
                Some(replacement_header),
            ),
            ("clear", json!({ "kind": "clear" }), None),
        ];

        for (label, action, expected_value) in cases {
            let title = format!("current-only-{label}");
            let base = expect_saved_document(
                create_source_document_inner(
                    system.as_ref(),
                    serde_json::from_value(json!({
                        "format": "legado",
                        "title": title,
                        "text": legado_source_without_header(&title)
                    }))
                    .expect("current-only base create wire"),
                )
                .await
                .expect("create current-only base"),
                "create current-only base",
            );
            assert!(base.credential_slots.is_empty());
            assert!(
                serde_json::from_str::<Value>(&base.masked_text)
                    .expect("current-only base JSON")
                    .get("header")
                    .is_none()
            );
            let pin = pin_source_document_revision_inner(
                system.as_ref(),
                PinSourceDocumentRevisionRequest {
                    document_id: base.summary.document_id,
                    document_revision: base.summary.revision,
                },
            )
            .await
            .expect("pin current-only base");
            let current = expect_saved_document(
                save_source_document_inner(
                    system.as_ref(),
                    SaveSourceDocumentRequest {
                        document_id: base.summary.document_id,
                        expected_revision: base.summary.revision,
                        masked_text: credential_legado_source_with_value(
                            &title,
                            "Bearer current-only-current",
                        ),
                    },
                )
                .await
                .expect("add current-only current header"),
                "add current-only current header",
            );
            assert_eq!(current.credential_slots.len(), 1);
            assert_eq!(current.credential_slots[0].path, "/header");

            let conflict = RebaseConflictFixture { base, current, pin };
            let request = rebase_merge_request(
                &conflict,
                conflict.base.summary.document_id,
                conflict.base.summary.revision,
                conflict.current.summary.revision,
                &conflict.base.masked_text,
                &json!([{ "path": "/header", "action": action }]),
            );
            let rebased = expect_saved_document(
                rebase_source_document_inner(system.as_ref(), request)
                    .await
                    .expect("current-only rebase outcome"),
                "current-only rebase",
            );
            assert_eq!(
                rebased.summary.revision,
                conflict.current.summary.revision + 1
            );
            if let Some(expected_value) = expected_value {
                assert_eq!(rebased.credential_slots.len(), 1);
                assert_eq!(rebased.credential_slots[0].path, "/header");
                assert_default_document_wire_is_safe(
                    &rebased,
                    &["Bearer current-only-current", "current-only-replacement"],
                );
                let revealed = reveal_source_document_credential_inner(
                    system.as_ref(),
                    RevealSourceDocumentCredentialRequest {
                        target: credential_target(&rebased),
                    },
                )
                .await
                .expect("reveal current-only rebased credential");
                assert_eq!(revealed.value, expected_value);
            } else {
                assert!(rebased.credential_slots.is_empty());
                assert_eq!(rebased.masked_text, conflict.base.masked_text);
            }
            release_source_document_revision_pin_inner(
                system.as_ref(),
                ReleaseSourceDocumentRevisionPinRequest {
                    pin_id: conflict.pin.pin_id,
                },
            )
            .await
            .expect("release current-only pin");
        }
    }

    #[tokio::test]
    async fn rebase_fork_creates_revision_one_without_mutating_current_document() {
        init_mock_keyring();
        let fixture = TempRoot::new();
        let system = fixture.open().await;
        let conflict = create_rebase_conflict(
            system.as_ref(),
            "fork-base",
            "Bearer fork-local",
            "Bearer fork-current",
        )
        .await;
        let local_masked_text =
            conflict
                .base
                .masked_text
                .replacen("fork-base", "fork-preserves-local-layout", 1);
        let request = serde_json::from_value::<RebaseSourceDocumentRequest>(json!({
            "pin_id": conflict.pin.pin_id,
            "document_id": conflict.base.summary.document_id,
            "base_revision": conflict.base.summary.revision,
            "current_revision": conflict.current.summary.revision,
            "local_masked_text": local_masked_text,
            "mode": { "kind": "fork", "title": "Forked draft" },
            "credential_resolutions": [{
                "path": "/header",
                "action": { "kind": "keep_local" }
            }]
        }))
        .expect("fork rebase wire");
        let forked = expect_saved_document(
            rebase_source_document_inner(system.as_ref(), request)
                .await
                .expect("fork rebase outcome"),
            "fork rebase",
        );
        assert_ne!(
            forked.summary.document_id,
            conflict.base.summary.document_id
        );
        assert_eq!(forked.summary.revision, 1);
        assert_eq!(forked.summary.title, "Forked draft");
        assert_eq!(
            forked.summary.state,
            lj_rule_system::SourceDocumentState::Draft
        );
        assert!(forked.masked_text.contains("fork-preserves-local-layout"));
        let revealed = reveal_source_document_credential_inner(
            system.as_ref(),
            RevealSourceDocumentCredentialRequest {
                target: credential_target(&forked),
            },
        )
        .await
        .expect("forked credential reveal");
        assert_eq!(revealed.value, "{\"Authorization\":\"Bearer fork-local\"}");
        let original = get_source_document_inner(
            system.as_ref(),
            GetSourceDocumentRequest {
                document_id: conflict.base.summary.document_id,
            },
        )
        .await
        .expect("read original after fork")
        .expect("original remains");
        assert_eq!(original, conflict.current);
        release_source_document_revision_pin_inner(
            system.as_ref(),
            ReleaseSourceDocumentRevisionPinRequest {
                pin_id: conflict.pin.pin_id,
            },
        )
        .await
        .expect("release fork pin");
    }

    #[tokio::test]
    async fn rebase_rejects_resolution_gaps_duplicates_path_drift_and_sentinel_replay() {
        init_mock_keyring();
        let fixture = TempRoot::new();
        let system = fixture.open().await;
        let conflict = create_rebase_conflict(
            system.as_ref(),
            "rebase-validation",
            "Bearer validation-local",
            "Bearer validation-current",
        )
        .await;
        let cases = vec![
            (json!([]), "credential_resolution_missing"),
            (
                json!([
                    { "path": "/header", "action": { "kind": "keep_local" } },
                    { "path": "/header", "action": { "kind": "keep_current" } }
                ]),
                "credential_resolution_duplicate",
            ),
            (
                json!([{ "path": "/unknown", "action": { "kind": "clear" } }]),
                "credential_resolution_path_unknown",
            ),
        ];
        for (resolutions, expected_code) in cases {
            let request = rebase_merge_request(
                &conflict,
                conflict.base.summary.document_id,
                conflict.base.summary.revision,
                conflict.current.summary.revision,
                &conflict.base.masked_text,
                &resolutions,
            );
            let outcome = rebase_source_document_inner(system.as_ref(), request)
                .await
                .expect("typed resolution rejection");
            assert_invalid_code(outcome, expected_code);
        }

        let replay_request = rebase_merge_request(
            &conflict,
            conflict.base.summary.document_id,
            conflict.base.summary.revision,
            conflict.current.summary.revision,
            &conflict.current.masked_text,
            &json!([{ "path": "/header", "action": { "kind": "keep_local" } }]),
        );
        assert_invalid_code(
            rebase_source_document_inner(system.as_ref(), replay_request)
                .await
                .expect("old/current sentinel replay is typed"),
            "credential_owner_mismatch",
        );
        let drifted_local =
            conflict
                .base
                .masked_text
                .replacen("\"header\":", "\"movedHeader\":", 1);
        let drift_request = rebase_merge_request(
            &conflict,
            conflict.base.summary.document_id,
            conflict.base.summary.revision,
            conflict.current.summary.revision,
            &drifted_local,
            &json!([{ "path": "/header", "action": { "kind": "keep_local" } }]),
        );
        assert_invalid_code(
            rebase_source_document_inner(system.as_ref(), drift_request)
                .await
                .expect("path drift is typed"),
            "credential_path_mismatch",
        );
        let current = get_source_document_inner(
            system.as_ref(),
            GetSourceDocumentRequest {
                document_id: conflict.base.summary.document_id,
            },
        )
        .await
        .expect("read current after rejected rebases")
        .expect("current remains");
        assert_eq!(current, conflict.current);
        release_source_document_revision_pin_inner(
            system.as_ref(),
            ReleaseSourceDocumentRevisionPinRequest {
                pin_id: conflict.pin.pin_id,
            },
        )
        .await
        .expect("release validation pin");
    }

    #[tokio::test]
    async fn rebase_rejects_pin_owner_base_and_concurrent_current_drift() {
        init_mock_keyring();
        let fixture = TempRoot::new();
        let system = fixture.open().await;
        let conflict = create_rebase_conflict(
            system.as_ref(),
            "rebase-concurrency",
            "Bearer concurrency-local",
            "Bearer concurrency-current",
        )
        .await;
        let resolution = json!([{ "path": "/header", "action": { "kind": "keep_current" } }]);
        let wrong_owner = rebase_merge_request(
            &conflict,
            lj_rule_system::SourceDocumentId::new(),
            conflict.base.summary.revision,
            conflict.current.summary.revision,
            &conflict.base.masked_text,
            &resolution,
        );
        assert_invalid_code(
            rebase_source_document_inner(system.as_ref(), wrong_owner)
                .await
                .expect("pin owner mismatch is typed"),
            "revision_pin_owner_mismatch",
        );
        let wrong_base = rebase_merge_request(
            &conflict,
            conflict.base.summary.document_id,
            conflict.base.summary.revision + 1,
            conflict.current.summary.revision,
            &conflict.base.masked_text,
            &resolution,
        );
        assert_invalid_code(
            rebase_source_document_inner(system.as_ref(), wrong_base)
                .await
                .expect("pin base mismatch is typed"),
            "revision_pin_revision_mismatch",
        );

        let advanced = expect_saved_document(
            save_source_document_inner(
                system.as_ref(),
                SaveSourceDocumentRequest {
                    document_id: conflict.current.summary.document_id,
                    expected_revision: conflict.current.summary.revision,
                    masked_text: conflict.current.masked_text.clone(),
                },
            )
            .await
            .expect("advance current before rebase commit"),
            "advance concurrent current",
        );
        let stale_current = rebase_merge_request(
            &conflict,
            conflict.base.summary.document_id,
            conflict.base.summary.revision,
            conflict.current.summary.revision,
            &conflict.base.masked_text,
            &resolution,
        );
        let outcome = rebase_source_document_inner(system.as_ref(), stale_current)
            .await
            .expect("concurrent drift returns conflict");
        let DocumentMutationOutcome::Conflict {
            expected_revision,
            actual_revision,
            current,
        } = outcome
        else {
            panic!("current drift must return conflict");
        };
        assert_eq!(expected_revision, conflict.current.summary.revision);
        assert_eq!(actual_revision, advanced.summary.revision);
        assert_eq!(current, advanced);
        assert_default_document_wire_is_safe(
            &current,
            &["Bearer concurrency-local", "Bearer concurrency-current"],
        );
        release_source_document_revision_pin_inner(
            system.as_ref(),
            ReleaseSourceDocumentRevisionPinRequest {
                pin_id: conflict.pin.pin_id,
            },
        )
        .await
        .expect("release concurrency pin");
    }

    #[test]
    fn fetch_import_src_request_uses_request_url_wire() {
        let request = serde_json::from_value::<FetchImportSrcRequest>(json!({
            "url": "https://example.com/sources.json"
        }))
        .expect("fetch_import_src request wire 应反序列化");
        assert_eq!(request.url, "https://example.com/sources.json");
    }

    #[tokio::test]
    // 单个持久化场景必须跨 create/save/credential/install/restart 验证同一 document owner。
    #[allow(clippy::too_many_lines)]
    async fn source_document_commands_preserve_masking_conflicts_and_slot_ownership() {
        init_mock_keyring();
        let fixture = TempRoot::new();
        let system = fixture.open().await;

        let empty_request = serde_json::from_value::<ListSourceDocumentsRequest>(json!({}))
            .expect("list_source_documents 空 request 应反序列化");
        assert!(
            list_source_documents_inner(system.as_ref(), empty_request)
                .await
                .expect("初始 document list 应可读取")
                .is_empty(),
            "新 vault 不得伪造 draft"
        );

        let create = serde_json::from_value::<CreateSourceDocumentRequest>(json!({
            "format": "legado",
            "title": "Root Vault 来源",
            "text": credential_legado_source("Root Vault 来源")
        }))
        .expect("create_source_document request wire 应反序列化");
        let created_outcome = create_source_document_inner(system.as_ref(), create)
            .await
            .expect("create_source_document 应返回 typed outcome");
        let created_wire = serde_json::to_value(&created_outcome).expect("create outcome wire");
        assert_eq!(created_wire["status"], "saved", "{created_wire}");
        let created = expect_saved_document(created_outcome, "create_source_document");
        assert_eq!(created.summary.revision, 1);
        assert_eq!(
            created.summary.state,
            lj_rule_system::SourceDocumentState::Draft
        );
        assert_default_document_wire_is_safe(&created, &[TEST_CREDENTIAL]);
        let created_target = credential_target(&created);

        let second_create = serde_json::from_value::<CreateSourceDocumentRequest>(json!({
            "format": "legado",
            "title": "Ownership 对照来源",
            "text": credential_legado_source("Ownership 对照来源")
        }))
        .expect("第二个 create request wire 应反序列化");
        let second = expect_saved_document(
            create_source_document_inner(system.as_ref(), second_create)
                .await
                .expect("第二个 document 应创建"),
            "第二个 create_source_document",
        );

        let listed = list_source_documents_inner(
            system.as_ref(),
            serde_json::from_value(json!({})).expect("list request wire"),
        )
        .await
        .expect("document list 应返回两个安全摘要");
        assert_eq!(listed.len(), 2);
        assert!(
            listed
                .iter()
                .all(|summary| summary.credential_slot_count == 1)
        );

        let get_request = serde_json::from_value::<GetSourceDocumentRequest>(json!({
            "document_id": created.summary.document_id
        }))
        .expect("get_source_document request wire 应反序列化");
        let default_document = get_source_document_inner(system.as_ref(), get_request)
            .await
            .expect("get_source_document 应读取 masked 文档")
            .expect("刚创建的文档必须存在");
        assert_eq!(default_document, created);
        assert_default_document_wire_is_safe(&default_document, &[TEST_CREDENTIAL]);

        let reveal_request = serde_json::from_value::<RevealSourceDocumentCredentialRequest>(
            json!({ "target": created_target }),
        )
        .expect("reveal request wire 应反序列化");
        let revealed = reveal_source_document_credential_inner(system.as_ref(), reveal_request)
            .await
            .expect("正确 owner 的 reveal 应成功");
        assert_eq!(revealed.target, created_target);
        assert_eq!(
            revealed.value,
            format!("{{\"Authorization\":\"{TEST_CREDENTIAL}\"}}")
        );

        let wrong_document_target = SourceDocumentCredentialTarget {
            document_id: second.summary.document_id,
            document_revision: second.summary.revision,
            slot_id: created_target.slot_id,
        };
        let wrong_document_request =
            serde_json::from_value::<RevealSourceDocumentCredentialRequest>(json!({
                "target": wrong_document_target
            }))
            .expect("跨文档 reveal request wire 应反序列化");
        let Err(ownership_error) =
            reveal_source_document_credential_inner(system.as_ref(), wrong_document_request).await
        else {
            panic!("跨文档 slot 必须被拒绝");
        };
        assert!(ownership_error.starts_with("credential_owner_mismatch:"));
        assert!(!ownership_error.contains(TEST_CREDENTIAL));

        let replacement_header = "{\"Authorization\":\"Bearer root-vault-command-replaced\"}";
        let replace_request =
            serde_json::from_value::<ReplaceSourceDocumentCredentialRequest>(json!({
                "target": created_target,
                "value": replacement_header
            }))
            .expect("replace request wire 应反序列化");
        let replaced = expect_saved_document(
            replace_source_document_credential_inner(system.as_ref(), replace_request)
                .await
                .expect("replace credential 应返回 typed outcome"),
            "replace_source_document_credential",
        );
        assert_eq!(replaced.summary.revision, created.summary.revision + 1);
        assert_default_document_wire_is_safe(
            &replaced,
            &[TEST_CREDENTIAL, "root-vault-command-replaced"],
        );
        let replaced_target = credential_target(&replaced);
        assert_ne!(replaced_target.slot_id, created_target.slot_id);

        let stale_reveal = serde_json::from_value::<RevealSourceDocumentCredentialRequest>(json!({
            "target": created_target
        }))
        .expect("旧 revision reveal request wire 应反序列化");
        match reveal_source_document_credential_inner(system.as_ref(), stale_reveal).await {
            Ok(_) => panic!("旧 revision slot 必须被拒绝"),
            Err(error) => {
                assert!(error.starts_with("credential_owner_mismatch:"));
                assert!(!error.contains(TEST_CREDENTIAL));
            }
        }

        let edited_masked_text =
            replaced
                .masked_text
                .replacen("Root Vault 来源", "Root Vault 已保存", 1);
        assert_ne!(edited_masked_text, replaced.masked_text);
        let save_request = serde_json::from_value::<SaveSourceDocumentRequest>(json!({
            "document_id": replaced.summary.document_id,
            "expected_revision": replaced.summary.revision,
            "masked_text": edited_masked_text
        }))
        .expect("save request wire 应反序列化");
        let saved = expect_saved_document(
            save_source_document_inner(system.as_ref(), save_request)
                .await
                .expect("save_source_document 应返回 typed outcome"),
            "save_source_document",
        );
        assert_eq!(saved.summary.revision, replaced.summary.revision + 1);
        assert_default_document_wire_is_safe(
            &saved,
            &[TEST_CREDENTIAL, "root-vault-command-replaced"],
        );

        let saved_target = credential_target(&saved);
        let reveal_saved = serde_json::from_value::<RevealSourceDocumentCredentialRequest>(json!({
            "target": saved_target
        }))
        .expect("saved revision reveal request wire 应反序列化");
        let revealed_saved = reveal_source_document_credential_inner(system.as_ref(), reveal_saved)
            .await
            .expect("save 后 credential 必须仍由新 slot 持有");
        assert_eq!(revealed_saved.value, replacement_header);

        let stale_save = serde_json::from_value::<SaveSourceDocumentRequest>(json!({
            "document_id": replaced.summary.document_id,
            "expected_revision": replaced.summary.revision,
            "masked_text": replaced.masked_text
        }))
        .expect("stale save request wire 应反序列化");
        let conflict = save_source_document_inner(system.as_ref(), stale_save)
            .await
            .expect("stale save 应返回 conflict outcome");
        let conflict_wire = serde_json::to_value(&conflict).expect("conflict outcome wire");
        assert_eq!(conflict_wire["status"], "conflict");
        let conflict_fields = conflict_wire
            .as_object()
            .expect("conflict 必须是对象")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            conflict_fields,
            ["status", "expected_revision", "actual_revision", "current"]
                .into_iter()
                .collect()
        );
        let conflict_current = match conflict {
            DocumentMutationOutcome::Conflict {
                expected_revision,
                actual_revision,
                current,
            } => {
                assert_eq!(expected_revision, replaced.summary.revision);
                assert_eq!(actual_revision, saved.summary.revision);
                current
            }
            other => panic!("stale save 必须返回 conflict: {other:?}"),
        };
        assert_eq!(conflict_current.summary.revision, saved.summary.revision);
        assert_default_document_wire_is_safe(
            &conflict_current,
            &[TEST_CREDENTIAL, "root-vault-command-replaced"],
        );

        let document_ref = serde_json::from_value::<DocumentRef>(json!({
            "document_id": saved.summary.document_id,
            "document_revision": saved.summary.revision
        }))
        .expect("prepare-from-document request wire 应反序列化");
        let candidate = prepare_install_from_document_inner(system.as_ref(), document_ref)
            .await
            .expect("已保存 document revision 应生成 candidate");
        assert_eq!(candidate.document_ref, Some(document_ref));
        assert!(!candidate.transient);
        assert_eq!(candidate.expected_installed_revision, 0);
        let candidate_wire = serde_json::to_value(&candidate).expect("candidate wire");
        let candidate_encoded = candidate_wire.to_string();
        assert!(!candidate_encoded.contains(TEST_CREDENTIAL));
        assert!(!candidate_encoded.contains("root-vault-command-replaced"));
        for forbidden in [
            "document_id",
            "document_revision",
            "raw_text",
            "manifest",
            "secret_artifact_id",
            "artifact_ref",
            "blob_locator",
            "key_id",
        ] {
            assert!(
                candidate_wire.get(forbidden).is_none(),
                "public candidate 不得暴露内部字段 {forbidden}"
            );
        }

        let install_state = AppState::new(system.clone());
        let installed_from_document = install_inner(
            &install_state,
            InstallRequest {
                candidate_id: candidate.id,
                grant: CapabilityGrantPreset::NetworkOnly,
            },
        )
        .await
        .expect("document candidate 应由现有 install command 原子消费");
        assert_eq!(installed_from_document.document_ref, Some(document_ref));
        let installed_list = list_installed_sources_inner(system.as_ref())
            .await
            .expect("install 后来源列表应可读取");
        let listed_source = installed_list
            .iter()
            .find(|source| source.source_id == installed_from_document.source_id)
            .expect("install response 必须出现在来源列表");
        assert_eq!(listed_source.document_ref, Some(document_ref));
        assert_eq!(listed_source.revision, installed_from_document.revision);
        let linked_request = serde_json::from_value::<GetSourceDocumentRequest>(json!({
            "document_id": saved.summary.document_id
        }))
        .expect("linked document get request wire");
        let linked = get_source_document_inner(system.as_ref(), linked_request)
            .await
            .expect("install 后应能读取 linked document")
            .expect("install 不得删除工作副本");
        assert_eq!(
            linked.summary.state,
            lj_rule_system::SourceDocumentState::Linked
        );
        assert_eq!(
            linked.summary.installed_revision,
            Some(installed_from_document.revision)
        );
        assert_eq!(linked.summary.revision, saved.summary.revision);
        drop(install_state);

        let clear_request = serde_json::from_value::<ClearSourceDocumentCredentialRequest>(json!({
            "target": saved_target
        }))
        .expect("clear request wire 应反序列化");
        let cleared = expect_saved_document(
            clear_source_document_credential_inner(system.as_ref(), clear_request)
                .await
                .expect("clear credential 应返回 typed outcome"),
            "clear_source_document_credential",
        );
        assert!(cleared.credential_slots.is_empty());
        assert!(!cleared.masked_text.contains(TEST_CREDENTIAL));
        assert!(!cleared.masked_text.contains("root-vault-command-replaced"));
        assert_no_forbidden_default_fields(
            &serde_json::to_value(&cleared).expect("cleared document wire"),
        );

        let rename_request = serde_json::from_value::<RenameSourceDocumentRequest>(json!({
            "document_id": cleared.summary.document_id,
            "expected_revision": cleared.summary.revision,
            "title": "Root Vault 已重命名"
        }))
        .expect("rename request wire 应反序列化");
        let renamed = expect_saved_document(
            rename_source_document_inner(system.as_ref(), rename_request)
                .await
                .expect("rename_source_document 应返回 typed outcome"),
            "rename_source_document",
        );
        assert_eq!(renamed.summary.title, "Root Vault 已重命名");

        let delete_request = serde_json::from_value::<DeleteSourceDocumentRequest>(json!({
            "document_id": second.summary.document_id,
            "expected_revision": second.summary.revision
        }))
        .expect("delete request wire 应反序列化");
        let deleted = delete_source_document_inner(system.as_ref(), delete_request)
            .await
            .expect("delete_source_document 应返回 typed outcome");
        assert!(matches!(
            &deleted,
            DocumentMutationOutcome::Saved { document: None }
        ));
        assert_eq!(
            serde_json::to_value(&deleted).expect("delete outcome wire"),
            json!({ "status": "saved", "document": null })
        );

        let linked_delete_request = serde_json::from_value::<DeleteSourceDocumentRequest>(json!({
            "document_id": renamed.summary.document_id,
            "expected_revision": renamed.summary.revision
        }))
        .expect("linked delete request wire 应反序列化");
        let linked_delete = delete_source_document_inner(system.as_ref(), linked_delete_request)
            .await
            .expect("linked delete 应返回 typed outcome");
        match linked_delete {
            DocumentMutationOutcome::Invalid { issues } => assert!(
                issues
                    .iter()
                    .any(|issue| issue.code == "document_delete_unsafe"),
                "linked document 删除必须给出稳定安全 issue"
            ),
            other => panic!("linked document 不得删除: {other:?}"),
        }

        let final_list = list_source_documents_inner(
            system.as_ref(),
            serde_json::from_value(json!({})).expect("final list request wire"),
        )
        .await
        .expect("final document list 应可读取");
        assert_eq!(final_list.len(), 1);
        assert_eq!(final_list[0].document_id, renamed.summary.document_id);

        system
            .shutdown_for_test()
            .await
            .expect("重启前应关闭 RuleSystem writer");
        drop(system);

        let reopened = fixture.open().await;
        let reopened_request = serde_json::from_value::<GetSourceDocumentRequest>(json!({
            "document_id": renamed.summary.document_id
        }))
        .expect("重启后 get request wire 应反序列化");
        let reopened_document = get_source_document_inner(reopened.as_ref(), reopened_request)
            .await
            .expect("重启后默认 get 应成功")
            .expect("重启后已保存文档必须存在");
        assert_eq!(reopened_document.summary.title, "Root Vault 已重命名");
        assert!(reopened_document.credential_slots.is_empty());
        let reopened_wire =
            serde_json::to_value(&reopened_document).expect("重启后 masked document wire");
        assert!(!reopened_wire.to_string().contains(TEST_CREDENTIAL));
        assert_no_forbidden_default_fields(&reopened_wire);
        reopened
            .shutdown_for_test()
            .await
            .expect("document command 测试结束应关闭 RuleSystem writer");
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

        let missing = get_media_item_inner(
            system,
            GetMediaItemRequest {
                resource_id: "item:missing".to_string(),
            },
        )
        .await
        .expect("get_media_item 缺失应成功返回 null");
        assert!(missing.is_none(), "缺失资源必须序列化为 null");

        let invalid = get_media_item_inner(
            system,
            GetMediaItemRequest {
                resource_id: "   ".to_string(),
            },
        )
        .await
        .expect_err("空 resource_id 必须校验失败");
        assert!(
            invalid.contains("resource_id_invalid"),
            "校验错误应暴露稳定 code: {invalid}"
        );

        let units = list_media_units_inner(
            system,
            ListMediaUnitsRequest {
                item_id: "item:missing".to_string(),
                offset: 0,
                limit: Some(50),
            },
        )
        .await
        .expect("缺失父 item 的 list_media_units 应空页");
        assert!(!units.parent_found);
        assert!(units.items.is_empty());
        assert!(!units.has_more);

        let limit_err = list_media_assets_inner(
            system,
            ListMediaAssetsRequest {
                unit_id: "unit:any".to_string(),
                offset: 0,
                limit: Some(101),
            },
        )
        .await
        .expect_err("limit>100 必须校验失败");
        assert!(
            limit_err.contains("media_page_limit_invalid"),
            "limit 校验错误: {limit_err}"
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
    // 单个 root-adapter 场景必须维持同一 execution 的 delivery/cancel/catch-up 状态。
    #[allow(clippy::too_many_lines)]
    async fn test_root_command_lifecycle_delivers_cancel_and_catch_up() {
        init_mock_keyring();
        let fixture = TempRoot::new();
        let server = MockServer::start().await;
        mount_slow_discover_route(&server).await;

        let system = fixture.open().await;
        let state = AppState::new(system.clone());
        assert!(
            list_source_documents_inner(system.as_ref(), ListSourceDocumentsRequest::default(),)
                .await
                .expect("quick-install 前 document list 应可读取")
                .is_empty(),
            "quick-install fixture 初始不得有 draft"
        );
        let candidate = prepare_install_inner(
            system.as_ref(),
            RuleInput::MaccmsJson {
                url: format!("{}/api.php/provide/vod/", server.uri()),
            },
        )
        .await
        .expect("prepare_install 应返回安全 candidate");
        assert!(candidate.transient);
        assert!(candidate.document_ref.is_none());
        assert_eq!(candidate.expected_installed_revision, 0);
        assert!(
            list_source_documents_inner(system.as_ref(), ListSourceDocumentsRequest::default(),)
                .await
                .expect("quick prepare 后 document list 应可读取")
                .is_empty(),
            "raw quick prepare 必须只创建 transient encrypted candidate，不得创建 draft"
        );
        let installed = install_inner(
            &state,
            InstallRequest {
                candidate_id: candidate.id,
                grant: CapabilityGrantPreset::NetworkOnly,
            },
        )
        .await
        .expect("install 应安装 candidate");
        assert!(
            list_source_documents_inner(system.as_ref(), ListSourceDocumentsRequest::default(),)
                .await
                .expect("quick install 后 document list 应可读取")
                .is_empty(),
            "消费 transient candidate 不得留下来源文档 draft"
        );
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
