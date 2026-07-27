//! Candidate-v2 composite staging、来源安装与 runtime credential carrier。
//!
//! candidate 由单个 writer command 同时发布 package/Plan、document baseline 与所有 secret refs。
//! transient quick-install 只进入加密 staging，不创建 draft。所有 plaintext carrier 都不实现
//! `Debug`、`Clone` 或 serde。

use lj_media::SourceProfile;
use lj_rule_model::{
    CredentialSlotManifest, Diagnostic, ExecutionPlan, PolicyCapabilities, RulePackage,
    SourceDocumentFormat,
};
use uuid::Uuid;

use super::{DocumentRef, SourceDocumentId};

/// 当前 durable candidate schema 版本。
pub const INSTALL_CANDIDATE_SCHEMA_VERSION: u32 = 2;

/// quick-install 使用的临时来源文档；不会创建 `source_document_projection`。
pub struct TransientSourceDocumentInput {
    /// 作者格式。
    pub format: SourceDocumentFormat,
    /// credential sentinel-masked 文本。
    pub masked_text: String,
    /// codec 已验证的完整原文，只会进入随机 secret artifact。
    pub raw_text: String,
    /// 不含 plaintext、但仍加密保存的 manifest。
    pub manifest: CredentialSlotManifest,
}

/// candidate 的来源文档基线。
pub enum CandidateDocumentInput {
    /// 精确指向已经显式保存的 current/pinned revision。
    Saved(DocumentRef),
    /// quick-install 临时加密 staging，不创建 draft。
    Transient(TransientSourceDocumentInput),
}

/// runtime adapter 使用的 opaque credential bytes。
///
/// storage 不解析其格式；该 carrier 不实现 `Debug`、`Clone` 或 serde。
pub struct RuntimeCredentialMaterial {
    bytes: Vec<u8>,
}

impl RuntimeCredentialMaterial {
    /// 包装 codec/runtime owner 生成的 opaque credential bytes。
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// 显式借用 plaintext；不得记录或放入公开 DTO。
    #[must_use]
    pub fn expose_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// 消费 carrier，避免向 writer 移交时复制 plaintext。
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// candidate-v2 单 writer composite publish 输入。
///
/// 此类型因可能承载 transient raw/runtime credential 而故意不实现 `Debug`、`Clone` 或 serde。
pub struct CandidateDraft {
    /// opaque candidate ID。
    pub candidate_id: Uuid,
    /// 作者包；以非 secret body artifact 固定。
    pub package: RulePackage,
    /// 已编译 immutable Plan。
    pub plan: ExecutionPlan,
    /// 用于预览与安装的来源 profile。
    pub profile: SourceProfile,
    /// candidate 所需 grant。
    pub required_grant: PolicyCapabilities,
    /// 导入、校验、编译诊断。
    pub diagnostics: Vec<Diagnostic>,
    /// 已保存 document ref 或 transient source material。
    pub document: CandidateDocumentInput,
    /// runtime 使用的 opaque credential snapshot。
    pub runtime_credentials: Option<RuntimeCredentialMaterial>,
    /// prepare 时固定的已安装 source revision；首次安装必须为 `0`。
    pub expected_installed_revision: u64,
    /// 到期时间；`None` 时使用默认 TTL。
    pub expires_at_ms: Option<i64>,
    /// 安全 trace 标识。
    pub trace_id: String,
    /// 可选关联 ID。
    pub correlation_id: Option<Uuid>,
    /// candidate 创建时刻（UTC epoch milliseconds）。
    pub created_at_ms: i64,
}

/// candidate 已 durable composite publish 后返回的安全摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateSummary {
    /// opaque candidate ID。
    pub candidate_id: Uuid,
    /// 稳定来源身份。
    pub source_identity: String,
    /// 已保存来源文档；transient quick-install 为 `None`。
    pub document_ref: Option<DocumentRef>,
    /// transient quick-install 为 `true`。
    pub transient: bool,
    /// prepare 时固定的 target installed revision。
    pub expected_installed_revision: u64,
    /// 仅用于预览的来源资料；不含作者包或执行计划。
    pub profile: SourceProfile,
    /// staging 时固定、安装时必须覆盖的能力需求。
    pub required_grant: PolicyCapabilities,
    /// 导入、校验与编译诊断。
    pub diagnostics: Vec<Diagnostic>,
    /// Definition BLAKE3 hash；只是一致性字段，不是 source revision identity。
    pub definition_hash: String,
    /// Plan BLAKE3 hash。
    pub plan_hash: String,
    /// 到期时间。
    pub expires_at_ms: i64,
}

/// 原子消费 candidate-v2 的请求。
#[derive(Debug, Clone)]
pub struct InstallCandidateRequest {
    /// 要消费的 opaque candidate。
    pub candidate_id: Uuid,
    /// 用户批准后的能力 grant。
    pub grant: PolicyCapabilities,
    /// 可重试安装事件 ID。
    pub event_id: Uuid,
    /// 安全 trace 标识。
    pub trace_id: String,
    /// 安装时刻（UTC epoch milliseconds）。
    pub occurred_at_ms: i64,
    /// 可选关联 ID。
    pub correlation_id: Option<Uuid>,
}

/// 已安装来源及其 immutable package/Plan。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledSource {
    /// 稳定来源身份。
    pub source_identity: String,
    /// Definition/package version；不作为历史唯一键。
    pub version: String,
    /// immutable 作者包。
    pub package: RulePackage,
    /// immutable 编译 Plan。
    pub plan: ExecutionPlan,
    /// 来源展示资料。
    pub profile: SourceProfile,
    /// 已批准能力。
    pub grant: PolicyCapabilities,
    /// 安装事务生成的权威 source revision。
    pub source_revision: u64,
    /// 关联来源文档；transient quick-install 为 `None`。
    pub document_id: Option<SourceDocumentId>,
    /// 对应已保存 document revision；transient 为 `None`。
    pub document_revision: Option<u64>,
}

/// 用于来源列表的稳定安全投影记录。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstalledSourceRecord {
    /// 稳定来源身份。
    pub source_identity: String,
    /// 当前 Definition/package version。
    pub version: String,
    /// 来源展示资料。
    pub profile: SourceProfile,
    /// 当前已批准的能力。
    pub grant: PolicyCapabilities,
    /// 当前权威 source revision。
    pub source_revision: u64,
    /// 是否存在可编辑、已保存的来源文档关联。
    pub has_editable_source: bool,
    /// 已关联文档 ID；legacy/transient source 为 `None`。
    pub document_id: Option<SourceDocumentId>,
    /// 已关联的 document revision。
    pub document_revision: Option<u64>,
}
