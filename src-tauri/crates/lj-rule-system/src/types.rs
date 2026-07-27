//! `RuleSystem` 的公开输入、候选、安装、执行与配置 DTO。
//!
//! 这些类型是 façade 的唯一外部边界；不会公开 `Definition`、`ExecutionPlan`、旧执行图、
//! runtime handler、`SQLite` connection 或 storage transaction。

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use futures::stream::BoxStream;
use lj_capability::{IntentInput, StandardIntent};
use lj_media::{MediaAsset, MediaGraphDelta, MediaUnit, SourceProfile};
pub use lj_rule_model::SourceDocumentFormat;
use lj_rule_model::{ArtifactRef, Diagnostic, PolicyCapabilities};
use lj_runtime::CancellationHandle;
use lj_storage::EventProjectionStorage;
pub use lj_storage::{
    CredentialSlotSummary, DocumentMutationOutcome, DocumentRef, DocumentValidationIssue,
    MaskedSourceDocument, SourceDocumentCredentialTarget, SourceDocumentId,
    SourceDocumentRevisionPin, SourceDocumentState, SourceDocumentSummary,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::RuleError;

/// `RuleSystem` 当前接受的来源输入。
#[derive(PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuleInput {
    /// Maccms JSON 采集 API 端点。
    MaccmsJson {
        /// 形如 `https://example.invalid/api.php/provide/vod/` 的来源端点。
        url: String,
    },
    /// Legado 书源 JSON；只在 prepare 阶段由 importer 解析，候选和安装 DTO 不回显原文。
    Legado {
        /// 原始 Legado 书源 JSON。敏感 header 仅在内存中交给 C2 加密 snapshot。
        source_json: String,
    },
}

impl fmt::Debug for RuleInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaccmsJson { url } => formatter
                .debug_struct("MaccmsJson")
                .field("url_bytes", &url.len())
                .finish(),
            Self::Legado { source_json } => formatter
                .debug_struct("Legado")
                .field("source_json_bytes", &source_json.len())
                .finish(),
        }
    }
}

/// 列出来源文档的空 façade 请求；保留真实 `{ request: {} }` command wrapper。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListSourceDocumentsRequest {}

/// 读取一个默认 masked 文档的 façade 请求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetSourceDocumentRequest {
    /// 随机、不透明文档身份。
    pub document_id: SourceDocumentId,
}

/// 创建并保存来源文档 revision `1` 的 façade 请求。
///
/// `text` 可能包含 credential plaintext，因此此类型不实现 `Debug`、`Clone` 或 `Serialize`。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSourceDocumentRequest {
    /// 作者格式。
    pub format: SourceDocumentFormat,
    /// 非空用户可见标题。
    pub title: String,
    /// 完整原始 JSON；进入 storage 前必须先由 authoring codec 校验并 split。
    pub text: String,
}

/// 保存 masked 编辑文本为下一 revision 的 façade 请求。
///
/// 正文即使已遮罩也不应进入默认日志，因此此类型不实现 `Debug`、`Clone` 或 `Serialize`。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveSourceDocumentRequest {
    /// 要保存的文档。
    pub document_id: SourceDocumentId,
    /// optimistic concurrency 基线。
    pub expected_revision: u64,
    /// 当前编辑器持有的 masked JSON；旧 sentinel 必须仍严格绑定该 revision/path。
    pub masked_text: String,
}

/// 固定一个已保存来源文档 revision 的 façade 请求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinSourceDocumentRevisionRequest {
    /// 要固定的来源文档。
    pub document_id: SourceDocumentId,
    /// 必须仍为 current 的已保存 revision。
    pub document_revision: u64,
}

/// 幂等释放 revision pin 的 façade 请求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseSourceDocumentRevisionPinRequest {
    /// 要释放的随机、不透明 pin ID。
    pub pin_id: Uuid,
}

/// revision pin 释放命令的固定 tagged outcome。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SourceDocumentRevisionPinReleaseOutcome {
    /// pin owners 已释放，或此前已经释放/不存在。
    Released {
        /// 调用方提交的 pin ID。
        pin_id: Uuid,
    },
}

/// slot-aware rebase 的持久化模式。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceDocumentRebaseMode {
    /// 把 resolved local 内容保存为 current 的下一 revision。
    Merge {},
    /// 创建新的 revision-1 draft，原文档保持不变。
    Fork {
        /// 新 draft 的用户可见标题。
        title: String,
    },
}

/// 单个可信 credential path 的用户 resolution。
///
/// `Replace.value` 是 plaintext，因此此类型故意只实现 `Deserialize`，不实现 `Debug`、`Clone`
/// 或 `Serialize`。
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceDocumentCredentialAction {
    /// 使用 current revision 在同一路径的 credential。
    KeepCurrent,
    /// 使用 pin base/local 在同一路径的 credential。
    KeepLocal,
    /// 使用本次请求携带的新 plaintext。
    Replace {
        /// 只在本次 rebase 调用栈中存在的 replacement。
        value: String,
    },
    /// 按来源格式清除该 credential path。
    Clear,
}

/// path-only credential resolution；slot owner 始终由后端 manifest 派生并验证。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDocumentCredentialResolution {
    /// 必须属于 trusted base/current manifest path union。
    pub path: String,
    /// 对该 trusted path 的显式动作。
    pub action: SourceDocumentCredentialAction,
}

/// slot-aware merge/fork 的 façade 请求。
///
/// local masked text 与 replacement 可能携带敏感编辑上下文，因此此类型故意只实现
/// `Deserialize`，不实现 `Debug`、`Clone` 或 `Serialize`。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RebaseSourceDocumentRequest {
    /// 固定 base material 的随机 pin。
    pub pin_id: Uuid,
    /// pin、base 与 current 共同所属的文档。
    pub document_id: SourceDocumentId,
    /// pin 固定的 base revision。
    pub base_revision: u64,
    /// 用户开始处理冲突时观察到的 current revision。
    pub current_revision: u64,
    /// 仅允许包含 base owner sentinel 的 local masked 文本。
    pub local_masked_text: String,
    /// merge 或 fork。
    pub mode: SourceDocumentRebaseMode,
    /// trusted base/current credential path union 的完整且唯一 resolution。
    pub credential_resolutions: Vec<SourceDocumentCredentialResolution>,
}

/// optimistic rename 的安全 façade 请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameSourceDocumentRequest {
    /// 要重命名的文档。
    pub document_id: SourceDocumentId,
    /// optimistic concurrency 基线。
    pub expected_revision: u64,
    /// 非空新标题。
    pub title: String,
}

/// 只删除未关联 draft 的安全 façade 请求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteSourceDocumentRequest {
    /// 要删除的文档。
    pub document_id: SourceDocumentId,
    /// optimistic concurrency 基线。
    pub expected_revision: u64,
}

/// 显式 reveal 单个 owner-bound credential slot 的 façade 请求。
///
/// 为避免 reveal 路径被整体日志化，此类型不实现 `Debug`、`Clone` 或 `Serialize`。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevealSourceDocumentCredentialRequest {
    /// 必须严格匹配当前 document/revision/slot owner。
    pub target: SourceDocumentCredentialTarget,
}

/// 清除单个 owner-bound credential slot 的 façade 请求。
///
/// 此请求会触发完整 secret material 的短生命周期重组，因此不实现 `Debug`、`Clone` 或
/// `Serialize`。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClearSourceDocumentCredentialRequest {
    /// 必须严格匹配当前 document/revision/slot owner。
    pub target: SourceDocumentCredentialTarget,
}

/// 显式替换单个 credential slot 的 façade 请求。
///
/// `value` 是短生命周期 plaintext，因此此类型不实现 `Debug`、`Clone` 或 `Serialize`。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplaceSourceDocumentCredentialRequest {
    /// 必须严格匹配当前 document/revision/slot owner。
    pub target: SourceDocumentCredentialTarget,
    /// Legado `/header` slot 的完整 header JSON string，或 Maccms10 单 header value。
    pub value: String,
}

/// 显式单 slot reveal 的短生命周期 wire response。
///
/// 此类型只实现 response 所需的 `Serialize`；不实现 `Debug`、`Clone` 或 `Deserialize`，也绝不
/// 出现在默认 document query。
#[derive(Serialize)]
pub struct SourceDocumentCredentialReveal {
    /// storage 已验证的 owner target。
    pub target: SourceDocumentCredentialTarget,
    /// 单次 plaintext value。
    pub value: String,
}

/// 只可作为 install token 传递的 opaque candidate ID。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CandidateId(Uuid);

impl CandidateId {
    pub(crate) const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    pub(crate) const fn as_uuid(&self) -> Uuid {
        self.0
    }
}

/// 已安装来源的稳定身份。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceId(String);

impl SourceId {
    pub(crate) fn from_identity(identity: String) -> Self {
        Self(identity)
    }

    pub(crate) fn as_identity(&self) -> &str {
        &self.0
    }
}

/// execution 的 stable opaque ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExecutionId(Uuid);

impl ExecutionId {
    pub(crate) const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    pub(crate) const fn as_uuid(self) -> Uuid {
        self.0
    }
}

/// 用户批准的 capability 集合。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityGrant(PolicyCapabilities);

impl CapabilityGrant {
    /// 创建不授予任何 capability 的 grant。
    #[must_use]
    pub fn none() -> Self {
        Self(PolicyCapabilities::default())
    }

    /// 创建仅授予 network capability 的 grant。
    #[must_use]
    pub fn network_only() -> Self {
        Self(PolicyCapabilities {
            network: true,
            system: lj_rule_model::SystemCapabilities::default(),
        })
    }

    /// 返回该 grant 是否包含 network capability。
    #[must_use]
    pub fn requires_network(&self) -> bool {
        self.0.network
    }

    pub(crate) fn from_policy(value: PolicyCapabilities) -> Self {
        Self(value)
    }

    pub(crate) fn policy(&self) -> &PolicyCapabilities {
        &self.0
    }
}

/// 已 staging、尚未安装的安全 candidate 预览。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallCandidate {
    /// 只可原样交给 [`crate::RuleSystem::install`] 的 opaque token。
    pub id: CandidateId,
    /// 已保存来源文档基线；quick-install transient candidate 为 `None`。
    pub document_ref: Option<DocumentRef>,
    /// 是否来自不创建 draft 的临时加密 staging。
    pub transient: bool,
    /// prepare 时固定的 installed source revision；首次安装为 `0`。
    pub expected_installed_revision: u64,
    /// 稳定来源资料。
    pub profile: SourceProfile,
    /// 安装所需的最小 capability grant。
    pub required_grant: CapabilityGrant,
    /// importer、validator 与 compiler 产生的可定位诊断。
    pub diagnostics: Vec<Diagnostic>,
    /// canonical Definition 的 BLAKE3 hex。
    pub definition_hash: String,
    /// immutable Plan 的 BLAKE3 hex。
    pub plan_hash: String,
    /// candidate 到期时刻（UTC epoch milliseconds）。
    pub expires_at_ms: i64,
}

/// 已安装来源的安全摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledSource {
    /// 可用于后续 execution 的稳定来源 ID。
    pub source_id: SourceId,
    /// 此次安装固定的来源版本。
    pub version: String,
    /// 已安装来源资料。
    pub profile: SourceProfile,
    /// source stream 的当前 revision。
    pub revision: u64,
    /// 后端持久化的 editable working-copy 关联；revision 是已安装 baseline。
    pub document_ref: Option<DocumentRef>,
}

/// 面向根层的资料库消费进度；不包含来源凭证或内部投影句柄。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryProgress {
    /// 当前消费单元的稳定资源 ID；没有单元时为空。
    pub unit_id: Option<String>,
    /// 当前消费位置。
    pub position: u64,
    /// 可选总长度。
    pub total: Option<u64>,
}

/// 一个资源的安全资料库投影条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryProjectionEntry {
    /// 标准媒体资源 ID。
    pub resource_id: String,
    /// 是否已收藏。
    pub favorite: bool,
    /// 是否已固定。
    pub pinned: bool,
    /// 最近打开时间（RFC3339 文本）。
    pub last_opened_at: Option<String>,
    /// 可选消费进度。
    pub progress: Option<LibraryProgress>,
    /// 此资源 library stream 的当前 revision。
    pub revision: u64,
    /// 最近更新时的全局序号。
    pub updated_global_seq: u64,
}

/// 完整、安全的资料库投影快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryProjection {
    /// 快照覆盖到的全局序号。
    pub global_seq: u64,
    /// 按稳定资源 ID 排序的资料库条目。
    pub entries: Vec<LibraryProjectionEntry>,
}

/// 根层更新一个资料库条目的安全请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryEntryUpdate {
    /// 要更新的标准媒体资源 ID。
    pub resource_id: String,
    /// 是否收藏。
    pub favorite: bool,
    /// 是否固定。
    pub pinned: bool,
    /// 最近打开时间（RFC3339 文本）。
    pub last_opened_at: Option<String>,
    /// 可选消费进度。
    pub progress: Option<LibraryProgress>,
    /// optimistic concurrency 所需的当前 resource revision；新条目为 0。
    pub expected_version: u64,
}

/// 资料库更新成功后的安全持久化收据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryUpdateReceipt {
    /// 提交后的单调全局序号。
    pub global_seq: u64,
    /// 提交后的资源 library stream revision。
    pub revision: u64,
}

/// 有界消费单元页：仅标准 `MediaUnit`，不含 Plan/effect/secret。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaUnitPage {
    /// 本页单元；顺序与 storage 稳定排序一致。
    pub items: Vec<MediaUnit>,
    /// 请求的 offset。
    pub offset: u32,
    /// 请求的 limit（硬上限由 façade 校验）。
    pub limit: u32,
    /// 是否还有后续页。
    pub has_more: bool,
    /// 父 item 是否仍存在（tombstone/缺失为 false）。
    pub parent_found: bool,
}

/// 有界资产页：仅标准 `MediaAsset`，含 locator 枚举原文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAssetPage {
    /// 本页资产；按 `id` 升序。
    pub items: Vec<MediaAsset>,
    /// 请求的 offset。
    pub offset: u32,
    /// 请求的 limit（硬上限由 façade 校验）。
    pub limit: u32,
    /// 是否还有后续页。
    pub has_more: bool,
    /// 父 unit 是否仍存在（tombstone/缺失为 false）。
    pub parent_found: bool,
}

/// execution 请求模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ExecutionMode {
    /// 调用真实 effect adapter，并通过 C2 archive capture 每个 effect。
    Live,
    /// 只读取指定历史 execution 的 immutable pin 与 effect archive。
    Replay {
        /// 被重放的历史 execution。
        execution_id: ExecutionId,
    },
}

/// 启动已安装来源的一次标准意图 execution。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecuteRequest {
    /// 已安装来源 ID。
    pub source_id: SourceId,
    /// 要执行的标准意图。
    pub intent: StandardIntent,
    /// 标准意图输入。
    pub input: IntentInput,
    /// live 或历史 replay。
    pub mode: ExecutionMode,
}

/// execution session 的一个持久化后可 delivery 事件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionEvent {
    /// 所属 execution。
    pub execution_id: ExecutionId,
    /// execution stream 内连续递增的 sequence。
    pub sequence: u64,
    /// 安全 trace ID。
    pub trace_id: String,
    /// 已持久化事件的 UTC epoch milliseconds。
    pub occurred_at_ms: i64,
    /// 事件内容。
    pub kind: ExecutionEventKind,
}

/// execution session 中对调用方可见的状态转换。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutionEventKind {
    /// execution start event 已 durable 提交。
    Started,
    /// 不含敏感载荷的诊断已 durable 提交。
    Diagnostic {
        /// 稳定诊断 code。
        code: String,
        /// 可安全展示的短消息。
        message: String,
    },
    /// 一个 live effect 的 archive capture 已 durable 提交。
    EffectCaptured {
        /// effect 的 execution-local ID。
        effect_id: Uuid,
        /// 已持久化的 body/secret artifact 安全引用。
        artifact_refs: Vec<ArtifactRef>,
        /// effect 输出的 BLAKE3 hex。
        output_hash: String,
    },
    /// `MediaGraphDelta` 与规范化 projection 已在同一 C2 transaction 提交。
    DeltaCommitted {
        /// C2 分配的全局 revision。
        global_revision: u64,
        /// execution stream 的 source-local revision。
        source_revision: u64,
        /// 已提交的标准媒体资源增量。
        delta: MediaGraphDelta,
    },
    /// execution 正常结束。
    Completed,
    /// execution 以安全错误结束。
    Failed {
        /// 启动后发生的安全错误。
        error: RuleError,
    },
    /// cancellation 已被观察，并且不会再调度新 effect。
    Cancelled,
}

/// 只在 `test-support` 中可读的已归档安全 witness。
///
/// 不含 Plan node、effect topology、body、cookie、token、authorization 或 URL query。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectWitnessCaptureForTest {
    /// C2 已验证的 canonical witness BLAKE3 hash。
    pub witness_hash: String,
    /// 按 effect 类型划分的脱敏 witness。
    pub witness: EffectWitnessForTest,
}

/// 只在测试中使用的类型化、脱敏 effect witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EffectWitnessForTest {
    /// HTTP 请求、redirect、DNS target 与 timing。
    Http(HttpEffectWitnessForTest),
    /// `QuickJS` hash、host-call 序列与 timing。
    QuickJs(QuickJsEffectWitnessForTest),
    /// Extract 输入 hash 与 timing。
    Extract(ExtractEffectWitnessForTest),
}

/// 脱敏 HTTP witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpEffectWitnessForTest {
    /// 初始安全请求描述。
    pub request: HttpRequestWitnessForTest,
    /// 实际跟随的安全 redirect hops。
    pub redirects: Vec<HttpRedirectWitnessForTest>,
    /// 实际 target 的 DNS/IP witness。
    pub dns_targets: Vec<HttpDnsTargetWitnessForTest>,
    /// HTTP effect 已发生后的安全失败类别；成功时为空。
    pub error: Option<HttpEffectErrorKindForTest>,
    /// effect 从开始到完成的耗时。
    pub duration_ms: u64,
}

/// 脱敏 HTTP 请求描述。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestWitnessForTest {
    /// 实际请求方法。
    pub method: HttpMethodForTest,
    /// 仅包含 scheme、host、port 与 path 的 URL。
    pub safe_url: String,
    /// 非敏感 header 的名称和 BLAKE3 value hash。
    pub headers: Vec<HttpRequestHeaderWitnessForTest>,
    /// request body 的 hash 与长度；不含原始 bytes。
    pub body: Option<HttpRequestBodyWitnessForTest>,
}

/// 非敏感 HTTP request header 的安全摘要。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestHeaderWitnessForTest {
    /// 小写、非敏感 header 名称。
    pub name: String,
    /// header 值的 BLAKE3 hash。
    pub value_hash: String,
}

/// HTTP request body 的安全摘要。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestBodyWitnessForTest {
    /// body 的 BLAKE3 hash。
    pub hash: String,
    /// body 字节长度。
    pub byte_len: u64,
}

/// 脱敏 HTTP redirect hop。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRedirectWitnessForTest {
    /// 重定向响应状态码。
    pub status: u16,
    /// 去敏后的来源 URL。
    pub from_url: String,
    /// 去敏后的目标 URL。
    pub to_url: String,
}

/// 脱敏 HTTP DNS/IP target witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpDnsTargetWitnessForTest {
    /// 实际请求时观察到的 host。
    pub host: String,
    /// 已解析或 literal 的 IP 地址。
    pub addresses: Vec<String>,
    /// 地址产生方式。
    pub kind: HttpDnsTargetKindForTest,
}

/// HTTP 方法的安全测试枚举。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpMethodForTest {
    /// GET。
    Get,
    /// POST。
    Post,
}

/// HTTP effect 的安全失败类别。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpEffectErrorKindForTest {
    /// DNS 或 SSRF target 校验失败。
    TargetValidation,
    /// request 构建或发送失败。
    Request,
    /// redirect 失败。
    Redirect,
    /// response body 读取失败。
    ResponseRead,
}

/// DNS/IP target 产生方式的安全测试枚举。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpDnsTargetKindForTest {
    /// SSRF 防护固定的 DNS 地址。
    PinnedDns,
    /// IP literal。
    IpLiteral,
    /// 只观察到 host 的直连测试路径。
    DirectHost,
}

/// 脱敏 `QuickJS` witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickJsEffectWitnessForTest {
    /// 编译后脚本 BLAKE3 hash。
    pub script_hash: String,
    /// effect 输入 BLAKE3 hash。
    pub input_hash: String,
    /// 输出 BLAKE3 hash。
    pub output_hash: String,
    /// 可安全归档的脚本失败类别。
    pub error: Option<QuickJsErrorKindForTest>,
    /// runtime host 调用的发生顺序。
    pub host_calls: Vec<QuickJsHostCallWitnessForTest>,
    /// effect 从开始到 worker 返回的耗时。
    pub duration_ms: u64,
}

/// 脱敏 `QuickJS` host 调用。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickJsHostCallWitnessForTest {
    /// 从一开始递增的调用序号。
    pub sequence: u32,
    /// 实际调用与其安全结果。
    pub call: QuickJsHostCallForTest,
}

/// `QuickJS` runtime host 调用的安全结果。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuickJsHostCallForTest {
    /// `Date.now()` 返回的 UTC epoch milliseconds。
    Time {
        /// 返回给脚本的 UTC epoch milliseconds。
        epoch_millis: i64,
    },
    /// `Math.random()` 返回值的 IEEE-754 bit pattern。
    Random {
        /// 返回给脚本的随机值 bit pattern。
        value_bits: u64,
    },
}

/// 可安全归档的 `QuickJS` 失败类别。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuickJsErrorKindForTest {
    /// 脚本求值失败。
    Evaluation,
    /// runtime 创建失败。
    RuntimeInitialization,
    /// context 创建失败。
    ContextInitialization,
    /// watchdog 超时。
    Timeout,
    /// watchdog 清理失败。
    Watchdog,
    /// worker 异常结束。
    WorkerFailure,
}

/// 脱敏 Extract witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractEffectWitnessForTest {
    /// 上游 HTTP 输出 BLAKE3 hash。
    pub input_hash: String,
    /// 解析和提取耗时。
    pub duration_ms: u64,
}

/// 可由 delivery 所有者之外的调用方持有的 opaque execution 取消句柄。
///
/// 它不暴露 runtime 类型、计划或 effect；根层可在移动 [`ExecutionSession`] 的 delivery stream
/// 前克隆此句柄，并在终态后自行从 registry 清理。
#[derive(Clone)]
pub struct ExecutionCancellation {
    inner: CancellationHandle,
}

impl ExecutionCancellation {
    pub(crate) const fn new(inner: CancellationHandle) -> Self {
        Self { inner }
    }

    /// 请求取消，并返回本次调用是否首次改变状态。
    #[must_use]
    pub fn cancel(&self) -> bool {
        self.inner.cancel()
    }

    /// 返回当前是否已经请求取消。
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }
}

/// 有界 delivery、幂等取消与持久 catch-up 的 execution session。
pub struct ExecutionSession {
    /// 当前 execution 的 opaque ID。
    pub id: ExecutionId,
    /// 仅在持久化后写入的有界 delivery stream。
    pub events: BoxStream<'static, ExecutionEvent>,
    cancellation: ExecutionCancellation,
    storage: EventProjectionStorage,
}

impl ExecutionSession {
    pub(crate) fn new(
        id: ExecutionId,
        events: BoxStream<'static, ExecutionEvent>,
        cancellation: CancellationHandle,
        storage: EventProjectionStorage,
    ) -> Self {
        Self {
            id,
            events,
            cancellation: ExecutionCancellation::new(cancellation),
            storage,
        }
    }

    /// 请求取消，并返回本次调用是否首次改变状态。
    #[must_use]
    pub fn cancel(&self) -> bool {
        self.cancellation.cancel()
    }

    /// 返回当前是否已经请求取消。
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// 克隆可在移动 delivery stream 后继续使用的 opaque 取消句柄。
    #[must_use]
    pub fn cancellation_handle(&self) -> ExecutionCancellation {
        self.cancellation.clone()
    }

    /// 取得 session 的有界 delivery stream。
    #[must_use]
    pub fn into_events(self) -> BoxStream<'static, ExecutionEvent> {
        self.events
    }

    /// 从 C2 execution stream 补读指定 sequence 之后的所有持久事件。
    ///
    /// # Errors
    ///
    /// execution 不存在、event payload 损坏或 C2 read lane 失败时返回 [`RuleError`]。
    pub async fn catch_up(&self, after_sequence: u64) -> Result<Vec<ExecutionEvent>, RuleError> {
        crate::system::catch_up_execution(&self.storage, self.id, after_sequence).await
    }
}

/// `RuleSystem` 的本地持久化与有界执行配置。
#[derive(Debug, Clone)]
pub struct RuleSystemConfig {
    pub(crate) database_path: PathBuf,
    pub(crate) artifact_root: PathBuf,
    pub(crate) keyring_service: String,
    pub(crate) candidate_ttl: Duration,
    pub(crate) session_event_capacity: usize,
    pub(crate) max_concurrent_executions: usize,
    pub(crate) max_concurrent_effects: usize,
    pub(crate) max_concurrent_effects_per_source: usize,
    pub(crate) local_fixture_http: bool,
}

impl RuleSystemConfig {
    /// 用桌面默认保留策略与有界执行容量创建配置。
    #[must_use]
    pub fn desktop(database_path: PathBuf, artifact_root: PathBuf) -> Self {
        Self {
            database_path,
            artifact_root,
            keyring_service: "lanjing.event-store.master-key".to_string(),
            candidate_ttl: Duration::from_hours(24),
            session_event_capacity: 64,
            max_concurrent_executions: 16,
            max_concurrent_effects: 16,
            max_concurrent_effects_per_source: 4,
            local_fixture_http: false,
        }
    }

    /// 创建允许环回地址的本地 fixture 配置。
    ///
    /// 该构造器只在 `test-support` feature 中提供，仍会使用真实 HTTP adapter、SQLite 与
    /// artifact archive；它不能用于生产网络策略。
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn local_fixture(database_path: PathBuf, artifact_root: PathBuf) -> Self {
        Self {
            local_fixture_http: true,
            ..Self::desktop(database_path, artifact_root)
        }
    }

    /// 覆盖安装级主密钥 service 名，便于隔离多个本地实例。
    #[must_use]
    pub fn with_keyring_service(mut self, keyring_service: String) -> Self {
        self.keyring_service = keyring_service;
        self
    }

    /// 覆盖 candidate staging 的到期时长。
    #[must_use]
    pub fn with_candidate_ttl(mut self, candidate_ttl: Duration) -> Self {
        self.candidate_ttl = candidate_ttl;
        self
    }
}
