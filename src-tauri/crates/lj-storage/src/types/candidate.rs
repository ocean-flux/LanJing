//! Candidate staging、来源安装与 credential 引用 DTO。
//!
//! secret 输入不实现日志友好的 trait；跨越此模块边界后只能保留 AES-256-GCM Secret Artifact
//! 引用，不能把明文带入 event、projection 或公开查询。

use lj_media::SourceProfile;
use lj_rule_model::{Diagnostic, ExecutionPlan, PolicyCapabilities, RulePackage, SecretRef};
use uuid::Uuid;

/// 需要在 source 安装前暂存的敏感凭证输入。
///
/// 此类型故意不实现 `Debug`、`Clone` 或序列化；`secret_bytes` 只能短暂跨越到 storage 的
/// blocking writer，并会被加密为 Secret Artifact。
pub struct SourceCredentialInput {
    /// 凭证随之 durable staging 的 opaque candidate ID。
    pub candidate_id: Uuid,
    /// 凭证归属的稳定 source identity。
    pub source_identity: String,
    /// 调用方私有格式的 credential/cookie snapshot 字节。
    pub secret_bytes: Vec<u8>,
    /// staging 创建时刻（UTC epoch milliseconds）。
    pub created_at_ms: i64,
}

/// 已 durable staging、可随 source install 原子消费的凭证引用。
///
/// 仅含 cookie namespace 与加密 Secret Artifact ref，绝不包含凭证明文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCredentialSnapshot {
    /// 由 source identity 派生的 cookie namespace。
    pub cookie_namespace: String,
    /// 安装时可原子引用的 AES-256-GCM Secret Artifact。
    pub secret_ref: SecretRef,
}

/// candidate staging 的完整输入。
#[derive(Debug, Clone)]
pub struct CandidateDraft {
    /// opaque candidate ID。
    pub candidate_id: Uuid,
    /// 作者包；不会以 Graph JSON 写入 `SQLite`。
    pub package: RulePackage,
    /// 已编译不可变 Plan。
    pub plan: ExecutionPlan,
    /// 用于预览与安装的来源 profile。
    pub profile: SourceProfile,
    /// candidate 所需 grant。
    pub required_grant: PolicyCapabilities,
    /// 导入、校验、编译诊断。
    pub diagnostics: Vec<Diagnostic>,
    /// 到期时间；`None` 时使用 24 小时默认值。
    pub expires_at_ms: Option<i64>,
    /// 安全 trace 标识。
    pub trace_id: String,
    /// 可选关联 ID。
    pub correlation_id: Option<Uuid>,
    /// candidate 创建时刻（UTC epoch milliseconds）。
    pub created_at_ms: i64,
}

/// candidate 已 durable staging 后返回的安全摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateSummary {
    /// opaque candidate ID。
    pub candidate_id: Uuid,
    /// 稳定来源身份。
    pub source_identity: String,
    /// 仅用于预览的来源资料；不含作者包或执行计划。
    pub profile: SourceProfile,
    /// staging 时固定、安装时必须覆盖的能力需求。
    pub required_grant: PolicyCapabilities,
    /// 导入、校验与编译诊断。
    pub diagnostics: Vec<Diagnostic>,
    /// Definition BLAKE3 hash。
    pub definition_hash: String,
    /// Plan BLAKE3 hash。
    pub plan_hash: String,
    /// 到期时间。
    pub expires_at_ms: i64,
}

/// 将 staged candidate 原子安装为来源版本的请求。
#[derive(Debug, Clone)]
pub struct InstallCandidateRequest {
    /// 要消费的 opaque candidate。
    pub candidate_id: Uuid,
    /// 用户批准后的能力 grant。
    pub grant: PolicyCapabilities,
    /// 可选的 source-owned cookie/credential Secret Artifact 引用；不含明文。
    pub source_credentials: Option<SourceCredentialSnapshot>,
    /// 当前 source stream version；首次安装为 0。
    pub expected_source_version: u64,
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
    /// 已安装版本。
    pub version: String,
    /// immutable 作者包。
    pub package: RulePackage,
    /// immutable 编译 Plan。
    pub plan: ExecutionPlan,
    /// 来源展示资料。
    pub profile: SourceProfile,
    /// 已批准能力。
    pub grant: PolicyCapabilities,
    /// source stream 当前 revision。
    pub revision: u64,
}

/// 用于来源列表的稳定安全投影记录。
///
/// 不包含作者包、Definition、Plan、artifact ref 或 secret；适合作为跨层查询 DTO 的 domain
/// 边界。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstalledSourceRecord {
    /// 稳定来源身份。
    pub source_identity: String,
    /// 当前已安装版本。
    pub version: String,
    /// 来源展示资料。
    pub profile: SourceProfile,
    /// 当前已批准的能力。
    pub grant: PolicyCapabilities,
    /// source stream 当前 revision。
    pub revision: u64,
}
