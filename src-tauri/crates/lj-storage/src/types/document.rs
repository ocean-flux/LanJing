//! 来源文档保险库的稳定 DTO 与仅在可信后端流转的 secret carrier。
//!
//! 本模块是文档身份、revision、masked wire 与 mutation outcome 的唯一 owner。公开 serde
//! 类型永远不携带原文、manifest、credential value、artifact locator 或 key；需要跨越
//! `RuleSystem → EventProjectionStorage` 的明文只放在不实现 `Debug`、`Clone`、`Serialize` 的
//! carrier 中，并应在 bounded writer 内尽快加密。

use std::fmt;
use std::str::FromStr;

use lj_rule_model::{CredentialSlotId, CredentialSlotManifest, SourceDocumentFormat};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::StorageError;

/// 来源文档正文的 UTF-8 最大字节数（2 MiB）。
pub const MAX_SOURCE_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
/// 当前来源文档 projection/snapshot schema 版本。
pub const SOURCE_DOCUMENT_SCHEMA_VERSION: u32 = 1;

/// 与内容无关、随机生成的来源文档身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceDocumentId(Uuid);

impl SourceDocumentId {
    /// 生成随机 UUID v4 文档身份。
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 返回底层 UUID，供 storage row 与 stream key 使用。
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for SourceDocumentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SourceDocumentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.hyphenated(), formatter)
    }
}

impl From<Uuid> for SourceDocumentId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl FromStr for SourceDocumentId {
    type Err = StorageError;

    /// 解析 canonical lowercase hyphenated UUID 文档身份。
    ///
    /// # Errors
    ///
    /// 输入不是 canonical UUID 时返回 [`StorageError::InvalidInput`]。
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parsed = Uuid::parse_str(value)
            .map_err(|_| StorageError::InvalidInput("来源文档 ID 无效".to_string()))?;
        if parsed.hyphenated().to_string() != value {
            return Err(StorageError::InvalidInput(
                "来源文档 ID 必须是 canonical UUID".to_string(),
            ));
        }
        Ok(Self(parsed))
    }
}

/// 与 plaintext 无关、随机生成的加密 secret artifact 身份。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretArtifactId(Uuid);

impl SecretArtifactId {
    /// 生成随机 UUID v4 secret artifact 身份。
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 返回底层 UUID，供 storage ownership row 使用。
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for SecretArtifactId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for SecretArtifactId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretArtifactId(<opaque>)")
    }
}

impl fmt::Display for SecretArtifactId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.hyphenated(), formatter)
    }
}

impl From<Uuid> for SecretArtifactId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl FromStr for SecretArtifactId {
    type Err = StorageError;

    /// 解析 canonical lowercase hyphenated UUID secret artifact 身份。
    ///
    /// # Errors
    ///
    /// 输入不是 canonical UUID 时返回 [`StorageError::InvalidInput`]。
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parsed = Uuid::parse_str(value)
            .map_err(|_| StorageError::InvalidInput("secret artifact ID 无效".to_string()))?;
        if parsed.hyphenated().to_string() != value {
            return Err(StorageError::InvalidInput(
                "secret artifact ID 必须是 canonical UUID".to_string(),
            ));
        }
        Ok(Self(parsed))
    }
}

/// 来源文档与已安装来源的关联状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDocumentState {
    /// 尚未关联已安装来源，可在 revision 匹配时删除。
    Draft,
    /// 已关联稳定 source identity，不允许删除。
    Linked,
}

impl SourceDocumentState {
    /// 返回 `SQLite` 使用的稳定文本。
    #[must_use]
    pub const fn as_db(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Linked => "linked",
        }
    }

    /// 从 `SQLite` 稳定文本恢复状态。
    ///
    /// # Errors
    ///
    /// 数据库出现未知状态时返回 [`StorageError::InvalidInput`]。
    pub fn from_db(value: &str) -> Result<Self, StorageError> {
        match value {
            "draft" => Ok(Self::Draft),
            "linked" => Ok(Self::Linked),
            _ => Err(StorageError::InvalidInput("未知来源文档状态".to_string())),
        }
    }
}

/// 来源文档列表使用的安全摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDocumentSummary {
    /// 随机、不透明文档身份。
    pub document_id: SourceDocumentId,
    /// 作者格式；字段解释仍由 authoring codec owner 负责。
    pub format: SourceDocumentFormat,
    /// 用户可见标题。
    pub title: String,
    /// draft/linked 状态。
    pub state: SourceDocumentState,
    /// 已关联的稳定 source identity；draft 为 `None`。
    pub source_identity: Option<String>,
    /// 当前工作副本 revision。
    pub revision: u64,
    /// 最近成功安装的 source revision；尚未安装为 `None`。
    pub installed_revision: Option<u64>,
    /// masked 文本的 BLAKE3 hash；不用于 secret identity。
    pub masked_hash: String,
    /// 当前 manifest 中的 credential slot 数。
    pub credential_slot_count: u32,
    /// projection/secret layout schema 版本。
    pub schema_version: u32,
    /// 创建时刻（UTC epoch milliseconds）。
    pub created_at_ms: i64,
    /// 最近持久化变更时刻（UTC epoch milliseconds）。
    pub updated_at_ms: i64,
}

/// 默认读取可见的单个 credential slot 摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialSlotSummary {
    /// 随机、不透明 slot ID。
    pub slot_id: CredentialSlotId,
    /// authoring codec 固定的 JSON Pointer。
    pub path: String,
    /// 共享敏感名称策略识别的逻辑名称。
    pub name: String,
    /// 当前 revision 是否拥有对应加密 material。
    pub has_value: bool,
}

/// 默认读取返回的 masked 来源文档。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskedSourceDocument {
    /// 可用于列表和 optimistic concurrency 的安全摘要。
    pub summary: SourceDocumentSummary,
    /// 只含 revision-bound sentinel、不含 credential plaintext 的合法 JSON 文本。
    pub masked_text: String,
    /// 当前 manifest 的安全 slot 摘要；不暴露 manifest artifact ref。
    pub credential_slots: Vec<CredentialSlotSummary>,
}

/// 精确指向一个已保存来源文档 revision。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentRef {
    /// 随机、不透明文档身份。
    pub document_id: SourceDocumentId,
    /// 已保存 revision；`0` 永远无效。
    pub document_revision: u64,
}

/// 后端为编辑会话固定的来源文档 revision。
///
/// pin 只公开随机 ID、owner 与到期时间；secret 引用和 slot metadata 保持在 storage 内部表中。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDocumentRevisionPin {
    /// 随机、不透明 pin ID。
    pub pin_id: Uuid,
    /// 被固定的来源文档。
    pub document_id: SourceDocumentId,
    /// 被固定的已保存 revision。
    pub document_revision: u64,
    /// 到期时刻（UTC epoch milliseconds）。
    pub expires_at_ms: i64,
}

/// single writer 创建 revision pin 的安全输入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinSourceDocumentRevisionInput {
    /// 后端预先生成的随机 pin ID。
    pub pin_id: Uuid,
    /// 只能固定当前已保存 revision。
    pub document_ref: DocumentRef,
    /// 创建时刻（UTC epoch milliseconds）。
    pub created_at_ms: i64,
    /// 到期时刻（UTC epoch milliseconds）。
    pub expires_at_ms: i64,
}

/// pin/material 校验的稳定安全失败类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceDocumentRebaseInvalidReason {
    /// pin 不存在或已被幂等释放。
    PinNotFound,
    /// pin 已过期且其 secret owners 已释放。
    PinExpired,
    /// pin 不属于请求的 document。
    PinOwnerMismatch,
    /// pin 固定的 base revision 与请求不一致。
    PinRevisionMismatch,
}

/// single writer 读取可信 base/current material 的请求。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadSourceDocumentRebaseMaterialInput {
    /// 必须仍处于有效期内的 pin。
    pub pin_id: Uuid,
    /// 前端声明的 base owner；必须与 pin 完全一致。
    pub document_ref: DocumentRef,
    /// 前端观察到的 current revision。
    pub current_revision: u64,
    /// 校验到期时间使用的 UTC epoch milliseconds。
    pub now_ms: i64,
}

/// 可信 base/current material 对。
///
/// 两个 material 都携带 credential plaintext，因此此类型故意不实现 `Debug`、`Clone` 或 serde。
pub struct SourceDocumentRebaseMaterials {
    base: SourceDocumentMaterial,
    current: SourceDocumentMaterial,
}

impl SourceDocumentRebaseMaterials {
    #[must_use]
    pub(crate) fn new(base: SourceDocumentMaterial, current: SourceDocumentMaterial) -> Self {
        Self { base, current }
    }

    /// 消费 material 对，不复制 credential plaintext。
    #[must_use]
    pub fn into_parts(self) -> (SourceDocumentMaterial, SourceDocumentMaterial) {
        (self.base, self.current)
    }
}

/// 可信 base/current material 的内部加载结果。
///
/// `Ready` 携带 boxed credential plaintext carrier，因此此类型故意不实现 `Debug`、`Clone`
/// 或 serde。
pub enum SourceDocumentRebaseMaterialOutcome {
    /// pin 与 current revision 均通过验证。
    Ready(Box<SourceDocumentRebaseMaterials>),
    /// pin owner、revision 或 expiry 不满足请求。
    Invalid(SourceDocumentRebaseInvalidReason),
    /// current revision 已漂移；返回安全 masked current。
    Conflict {
        /// 调用方提交的 current revision。
        expected_revision: u64,
        /// 当前已提交 revision。
        actual_revision: u64,
        /// 当前服务端 masked document。
        current: MaskedSourceDocument,
    },
    /// 来源文档不存在。
    NotFound,
}

/// authoring codec 或 storage limit 返回的安全校验问题。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentValidationIssue {
    /// 稳定机器码。
    pub code: String,
    /// 不含正文与 credential 的用户可见消息。
    pub message: String,
    /// 可选 RFC 6901 JSON Pointer。
    pub path: Option<String>,
    /// 可选 UTF-8 byte offset。
    pub byte_offset: Option<u64>,
    /// 可选 UTF-8 byte length。
    pub byte_length: Option<u64>,
}

impl DocumentValidationIssue {
    /// 构造不带源码位置的安全校验问题。
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path: None,
            byte_offset: None,
            byte_length: None,
        }
    }
}

/// 文档写操作的跨层 tagged outcome。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DocumentMutationOutcome {
    /// mutation 已提交；删除成功时 `document` 为 `None`。
    Saved {
        /// 提交后的 masked current document；删除成功时为空。
        document: Option<MaskedSourceDocument>,
    },
    /// expected revision 已过期，且服务端内容未被覆盖。
    Conflict {
        /// 调用方提交的 expected revision。
        expected_revision: u64,
        /// 当前已提交 revision。
        actual_revision: u64,
        /// 当前服务端 masked document；永远不包含原文或 credential。
        current: MaskedSourceDocument,
    },
    /// 文档或目标 slot 不存在。
    NotFound,
    /// 平台 secure store 暂时锁定，可在解锁后重试。
    Locked,
    /// 当前平台没有可用 secure store。
    KeyUnavailable,
    /// 数据库记录的 key ID 已不在 secure store 中。
    KeyLost,
    /// 密文 envelope、ciphertext hash 或 AEAD 认证失败。
    Corrupt,
    /// codec/limit/ownership 校验失败，且没有写入 artifact。
    Invalid {
        /// 可安全返回给作者界面的校验问题。
        issues: Vec<DocumentValidationIssue>,
    },
}

/// 一个 slot 对应的短生命周期 credential plaintext。
///
/// 此类型故意不实现 `Debug`、`Clone` 或 serde。调用方只能显式借用或消费 value，并应立即
/// 交给 authoring codec 或 bounded writer。
pub struct CredentialSlotMaterial {
    slot_id: CredentialSlotId,
    value: String,
}

impl CredentialSlotMaterial {
    /// 构造待原子加密的 slot material。
    #[must_use]
    pub fn new(slot_id: CredentialSlotId, value: String) -> Self {
        Self { slot_id, value }
    }

    /// 返回随机 slot ID。
    #[must_use]
    pub const fn slot_id(&self) -> CredentialSlotId {
        self.slot_id
    }

    /// 显式借用 plaintext；不得进入日志、Event、projection 或 serde DTO。
    #[must_use]
    pub fn expose_value(&self) -> &str {
        &self.value
    }

    /// 消费 carrier，避免向 writer 移交时复制 plaintext。
    #[must_use]
    pub fn into_value(self) -> String {
        self.value
    }
}

/// authoring codec 已完成 split/校验后的一个新文档 revision。
///
/// 此 carrier 不实现 `Debug`、`Clone` 或 serde；storage 只校验 byte limit、target/slot ownership
/// 与 caller-provided issues，不解析 Legado/Maccms 字段。
pub struct SourceDocumentRevisionInput {
    /// 来源格式。
    pub format: SourceDocumentFormat,
    /// 可默认读取的 sentinel-masked JSON。
    pub masked_text: String,
    /// codec 已恢复并验证的完整原文，只会进入加密 artifact。
    pub raw_text: String,
    /// 不含 plaintext、但仍按 secret artifact 加密的 slot manifest。
    pub manifest: CredentialSlotManifest,
    /// 与 manifest slot 一一对应的短生命周期 plaintext。
    pub credentials: Vec<CredentialSlotMaterial>,
    /// codec/complexity 检查结果；非空时 writer 必须在创建 artifact 前返回 `Invalid`。
    pub issues: Vec<DocumentValidationIssue>,
}

/// 原子创建 draft 的 writer 输入。
///
/// secret-bearing revision 不实现 `Debug`/`Clone`/serde。
pub struct CreateSourceDocumentInput {
    /// 由后端预先生成的随机文档 ID；manifest target 必须使用同一 ID 和 revision `1`。
    pub document_id: SourceDocumentId,
    /// 用户可见标题。
    pub title: String,
    /// codec 已完成 split 的 revision `1`。
    pub revision: SourceDocumentRevisionInput,
    /// 创建时刻（UTC epoch milliseconds）。
    pub created_at_ms: i64,
}

/// 原子保存一个新工作 revision 的 writer 输入。
///
/// secret-bearing revision 不实现 `Debug`/`Clone`/serde。
pub struct SaveSourceDocumentInput {
    /// 要保存的文档。
    pub document_id: SourceDocumentId,
    /// optimistic concurrency 基线。
    pub expected_revision: u64,
    /// codec 已重新签发 sentinel 的 `expected_revision + 1` 内容。
    pub revision: SourceDocumentRevisionInput,
    /// 保存时刻（UTC epoch milliseconds）。
    pub saved_at_ms: i64,
}

/// optimistic rename 输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameSourceDocumentInput {
    /// 要重命名的文档。
    pub document_id: SourceDocumentId,
    /// optimistic concurrency 基线。
    pub expected_revision: u64,
    /// 非空新标题。
    pub title: String,
    /// 变更时刻（UTC epoch milliseconds）。
    pub renamed_at_ms: i64,
}

/// 只允许未关联 draft 使用的 optimistic delete 输入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteSourceDocumentInput {
    /// 要删除的文档。
    pub document_id: SourceDocumentId,
    /// optimistic concurrency 基线。
    pub expected_revision: u64,
    /// 删除时刻（UTC epoch milliseconds）。
    pub deleted_at_ms: i64,
}

/// 显式 credential 操作必须绑定的 document/revision/slot owner。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDocumentCredentialTarget {
    /// slot 所属文档。
    pub document_id: SourceDocumentId,
    /// slot 所属 revision。
    pub document_revision: u64,
    /// manifest 中的随机 slot ID。
    pub slot_id: CredentialSlotId,
}

/// replace/clear 在 codec 重新签发下一 revision 后提交的原子输入。
///
/// `next_revision` 包含完整新 revision secret，因此此类型不实现 `Debug`、`Clone` 或 serde。
pub struct EditSourceDocumentCredentialInput {
    /// 必须存在于当前 revision 的旧 slot owner。
    pub target: SourceDocumentCredentialTarget,
    /// codec 已完成 replace/clear 与重新 split 的下一 revision。
    pub next_revision: SourceDocumentRevisionInput,
    /// 提交时刻（UTC epoch milliseconds）。
    pub saved_at_ms: i64,
}

/// storage 原子 rebase 的提交模式。
pub enum SourceDocumentRebaseCommitMode {
    /// 把 resolved local revision 保存为 current 的下一 revision。
    Merge,
    /// 创建新的 revision-1 draft，原文档保持不变。
    Fork {
        /// 后端生成的新文档 ID。
        document_id: SourceDocumentId,
        /// 新 draft 的用户可见标题。
        title: String,
    },
}

/// single writer 提交 slot-aware rebase 的 secret-bearing 输入。
///
/// `revision` 包含完整原文和 credential plaintext，因此此类型故意不实现 `Debug`、`Clone`
/// 或 serde。
pub struct RebaseSourceDocumentInput {
    /// 必须仍有效的 base pin。
    pub pin_id: Uuid,
    /// pin 与 current 共同所属的文档。
    pub document_id: SourceDocumentId,
    /// pin 固定的 base revision。
    pub base_revision: u64,
    /// codec 计算时观察到的 current revision。
    pub current_revision: u64,
    /// merge/fork 的原子提交模式。
    pub mode: SourceDocumentRebaseCommitMode,
    /// codec 已按目标 document/revision 重新 split 的完整 revision。
    pub revision: SourceDocumentRevisionInput,
    /// 提交时刻（UTC epoch milliseconds）。
    pub saved_at_ms: i64,
}

/// 从保险库内部读取、供 codec reconstitute/prepare 使用的完整 revision material。
///
/// 此类型不实现 `Debug`、`Clone` 或 serde；默认 document query 永远不返回它。
pub struct SourceDocumentMaterial {
    document_ref: DocumentRef,
    format: SourceDocumentFormat,
    masked_text: String,
    raw_text: String,
    manifest: CredentialSlotManifest,
    credentials: Vec<CredentialSlotMaterial>,
}

impl SourceDocumentMaterial {
    pub(crate) fn new(
        document_ref: DocumentRef,
        format: SourceDocumentFormat,
        masked_text: String,
        raw_text: String,
        manifest: CredentialSlotManifest,
        credentials: Vec<CredentialSlotMaterial>,
    ) -> Self {
        Self {
            document_ref,
            format,
            masked_text,
            raw_text,
            manifest,
            credentials,
        }
    }

    /// 返回精确 document revision。
    #[must_use]
    pub const fn document_ref(&self) -> DocumentRef {
        self.document_ref
    }

    /// 返回来源格式。
    #[must_use]
    pub const fn format(&self) -> SourceDocumentFormat {
        self.format
    }

    /// 借用不含 plaintext credential 的 masked JSON。
    #[must_use]
    pub fn masked_text(&self) -> &str {
        &self.masked_text
    }

    /// 显式借用完整原文；不得进入日志、Event、projection 或 serde DTO。
    #[must_use]
    pub fn expose_raw_text(&self) -> &str {
        &self.raw_text
    }

    /// 借用已验证的 slot manifest。
    #[must_use]
    pub const fn manifest(&self) -> &CredentialSlotManifest {
        &self.manifest
    }

    /// 借用短生命周期 slot plaintext carriers。
    #[must_use]
    pub fn credentials(&self) -> &[CredentialSlotMaterial] {
        &self.credentials
    }

    /// 消费完整 material，把 slot plaintext carrier 交给 codec，避免额外复制。
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        DocumentRef,
        SourceDocumentFormat,
        String,
        String,
        CredentialSlotManifest,
        Vec<CredentialSlotMaterial>,
    ) {
        (
            self.document_ref,
            self.format,
            self.masked_text,
            self.raw_text,
            self.manifest,
            self.credentials,
        )
    }
}

/// 显式 reveal 返回的单 slot plaintext carrier。
///
/// 此类型不实现 `Debug`、`Clone` 或 serde；只有 command owner 可把单次 value 放入短生命周期
/// reveal response，默认 query 永远不能返回它。
pub struct RevealedSourceDocumentCredential {
    target: SourceDocumentCredentialTarget,
    value: String,
}

impl RevealedSourceDocumentCredential {
    pub(crate) fn new(target: SourceDocumentCredentialTarget, value: String) -> Self {
        Self { target, value }
    }

    /// 返回已验证 owner target。
    #[must_use]
    pub const fn target(&self) -> SourceDocumentCredentialTarget {
        self.target
    }

    /// 显式借用 plaintext；不得缓存、记录或并入默认 document DTO。
    #[must_use]
    pub fn expose_value(&self) -> &str {
        &self.value
    }

    /// 消费短生命周期 carrier，避免 command response 构造时复制 plaintext。
    #[must_use]
    pub fn into_value(self) -> String {
        self.value
    }
}
