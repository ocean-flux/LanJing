//! native rule document 作者生命周期 DTO。
//!
//! wire 一律 `snake_case`；`request` wrapper 由 Tauri command 层包装。Definition 直接使用
//! `lj-rule-model` 的 current shape（含 contract tag），本模块不定义平行 Definition。

use lj_capability::StandardIntent;
use lj_rule_model::RuleDefinition;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 预期响应数据类型闭集。
///
/// wire 为 `snake_case`（`html|xml|json`）；`lj-rule-model` 的 `ExpectedDataType` 使用
/// PascalCase，因此本 DTO 层单独定义并在构造节点配置时映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedDataType {
    /// HTML 文档。
    Html,
    /// XML 文档。
    Xml,
    /// JSON 文档。
    Json,
}

/// 创建方式。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CreateMode {
    /// 空文档：空 `FlowGraph` + 空 intent export，可保存但不可编译。
    Blank,
    /// 最小 Http→Extract→Mapper 骨架 + 单个标准意图导出。
    Template {
        /// 展示标题；只写入文档 metadata，不进入 Definition/hash。
        title: String,
        /// 模板覆盖的标准意图。
        intent: StandardIntent,
        /// 预期响应数据类型。
        data_type: ExpectedDataType,
        /// 来源基础 URL。
        base_url: String,
    },
    /// 导入一个 current `RuleDefinition`；不接受历史 shape 或 rule package。
    Import {
        /// 展示标题；只写入文档 metadata，不进入 Definition/hash。
        title: String,
        /// 已通过 current schema 反序列化的 canonical Definition。
        definition: RuleDefinition,
    },
}

/// 创建 native rule document 请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateNativeRuleDocumentRequest {
    /// 创建方式。
    pub mode: CreateMode,
}

/// 凭证槽位变更动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialMutationAction {
    /// 写入/替换槽位值（携带一次性明文）。
    Replace,
    /// 清除槽位值并释放 secret owner。
    Clear,
}

/// 一次凭证槽位变更。
///
/// `value` 仅 `replace` 时存在，后端立即包成 carrier 写 secret artifact 并丢弃明文，
/// 不进入 Definition、manifest 或任何历史记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialMutationRequest {
    /// 目标 HTTP 节点 ID。
    pub node_id: Uuid,
    /// 指向该节点 `HttpSpec.headers` 字段的 RFC 6901 JSON Pointer。
    pub json_pointer: String,
    /// 槽位的逻辑名称（敏感名称策略归一化后必须唯一）。
    pub logical_name: String,
    /// 变更动作。
    pub action: CredentialMutationAction,
    /// replace 的一次性值；clear 必须为 `None`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// 语义域保存载荷。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticSave {
    /// 客户端已知的已保存语义 revision；不匹配则返回分域 conflict。
    pub expected_revision: i64,
    /// 本次保存的完整 Definition（current shape）。
    pub definition: RuleDefinition,
    /// 一次性凭证槽位变更（与 Definition 一起原子校验）。
    #[serde(default)]
    pub credential_mutations: Vec<CredentialMutationRequest>,
}

/// 布局域保存载荷。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutSave {
    /// 客户端已知的已保存布局 revision。
    pub expected_revision: i64,
    /// 布局 JSON（作者编辑器的 opaque 内容）。
    pub layout_json: String,
}

/// 保存 native rule document 请求。
///
/// 语义/布局分域保存：每个域携带独立 `expected_revision`，冲突不影响另一域。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveNativeRuleDocumentRequest {
    /// 文档 ID。
    pub document_id: String,
    /// 语义域保存；`None` 表示本请求不保存语义。
    pub semantic: Option<SemanticSave>,
    /// 布局域保存；`None` 表示本请求不保存布局。
    pub layout: Option<LayoutSave>,
}

/// revision 冲突描述。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionConflict {
    /// 客户端期望的 revision。
    pub expected: i64,
    /// 存储中的当前 revision。
    pub current: i64,
}

/// 语义保存后的用户可见状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticActivation {
    /// 当前 semantic snapshot 仅是可继续编辑的草稿。
    Draft,
    /// 当前 semantic snapshot 已成为 Effective Rule Revision。
    Effective,
}

/// 单域保存结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainOutcome {
    /// 写入后的新 revision；冲突时保持未写。
    pub revision: i64,
    /// revision 冲突描述；仅当该域写入被拒绝时存在。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<RevisionConflict>,
    /// semantic 域成功保存后的激活状态；布局域和冲突时为空。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation: Option<SemanticActivation>,
}

/// 保存结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveNativeRuleDocumentOutcome {
    /// 文档 ID。
    pub document_id: String,
    /// 语义域结果。
    pub semantic: Option<DomainOutcome>,
    /// 布局域结果。
    pub layout: Option<DomainOutcome>,
}

/// Effective Rule Revision 的只读安全摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeRuleRevisionSummary {
    /// Effective Rule Revision 编号。
    pub revision: i64,
    /// canonical Definition BLAKE3。
    pub definition_hash: String,
    /// 该版本成为 Effective 的时刻（UTC epoch milliseconds）。
    pub effective_at_ms: i64,
}

/// 从 Effective 历史创建新 Draft 的请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreNativeRuleRevisionRequest {
    /// 目标文档 ID。
    pub document_id: String,
    /// 要恢复的历史 Effective Rule Revision。
    pub revision: i64,
    /// 客户端已知的当前 Draft Revision。
    pub expected_revision: i64,
}

/// 历史恢复结果；恢复只产生 Draft，不会直接替换 Effective。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreNativeRuleRevisionOutcome {
    /// 目标文档 ID。
    pub document_id: String,
    /// 写入后的 Draft Revision；冲突时保持当前 revision。
    pub revision: i64,
    /// Draft revision 冲突。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<RevisionConflict>,
}

/// 校验 native rule document 请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidateNativeRuleDocumentRequest {
    /// 文档 ID。
    pub document_id: String,
    /// 客户端已知的语义 revision；不匹配则返回 typed 错误。
    pub revision: i64,
}

/// 校验预览（安全摘要，不含 Plan/Graph JSON）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidateNativeRuleDocumentPreview {
    /// 校验的语义 revision。
    pub revision: i64,
    /// canonical Definition BLAKE3。
    pub definition_hash: String,
    /// Definition 是否通过 compiler 校验并可准备安装。
    pub valid: bool,
    /// immutable Plan BLAKE3；Definition 无效时为空。
    pub plan_hash: Option<String>,
    /// validator 与 compiler 诊断。
    pub diagnostics: Vec<lj_rule_model::Diagnostic>,
    /// 稳定来源资料；Definition 无效时为空。
    pub profile: Option<lj_media::SourceProfile>,
    /// 所需最小能力。
    pub capability: lj_rule_model::PolicyCapabilities,
}

/// native rule document 摘要（镜像 storage `DocumentSummary`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeRuleDocumentSummary {
    /// 文档 ID。
    pub document_id: String,
    /// 文档格式；当前仅 `native_rule`。
    pub format: String,
    /// 展示标题。
    pub title: String,
    /// 来源稳定身份。
    pub source_identity: String,
    /// 文档状态：`draft` 或 `linked`。
    pub state: String,
    /// 已保存语义 revision。
    pub semantic_revision: i64,
    /// 已保存布局 revision。
    pub layout_revision: i64,
    /// link revision（安装/链接推进）。
    pub link_revision: i64,
    /// 创建时间（UTC epoch 毫秒）。
    pub created_at_ms: i64,
    /// 更新时间（UTC epoch 毫秒）。
    pub updated_at_ms: i64,
}

/// 获取 native rule document 请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetNativeRuleDocumentRequest {
    /// 文档 ID。
    pub document_id: String,
}

/// native rule document 详情。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeRuleDocumentDetail {
    /// 文档摘要。
    pub summary: NativeRuleDocumentSummary,
    /// 已保存草稿语义 revision（与摘要冗余，便于无摘要时区分）。
    pub semantic_revision: i64,
    /// 最近一次有效语义的 revision；没有通过校验的版本时为空。
    pub effective_semantic_revision: Option<i64>,
    /// 当前 Effective Rule Revision 的只读摘要。
    pub effective_summary: Option<NativeRuleRevisionSummary>,
    /// 已保存布局 revision。
    pub layout_revision: i64,
    /// 当前已保存的脱敏草稿 Definition；凭证值不在 Definition 中。
    pub definition: Option<RuleDefinition>,
    /// 当前已保存的作者布局 JSON；布局不进入语义 hash。
    pub layout_json: Option<String>,
    /// 可选 provenance 摘要（不含 secret 引用）。
    pub provenance: Option<ProvenanceSummaryView>,
}

/// provenance 摘要视图（不含原文与 secret 引用）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceSummaryView {
    /// 来源格式（如 `legado`/`maccms`）。
    pub format: String,
    /// 导入适配器版本。
    pub adapter_version: String,
    /// 输入原文 hash。
    pub input_hash: String,
    /// 导入诊断。
    pub diagnostics: Vec<lj_rule_model::Diagnostic>,
    /// 导入时间（UTC epoch 毫秒）。
    pub imported_at_ms: i64,
}

/// 重命名 native rule document 请求。
///
/// 标题是展示 metadata：不推进语义 revision、不改 Definition hash。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameNativeRuleDocumentRequest {
    /// 文档 ID。
    pub document_id: String,
    /// 新标题。
    pub title: String,
    /// 客户端已知的文档 revision（乐观并发保护）。
    pub expected_revision: i64,
}

/// 删除 native rule document 请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteNativeRuleDocumentRequest {
    /// 文档 ID。
    pub document_id: String,
    /// 文档为 `linked` 状态时必须显式确认。
    #[serde(default)]
    pub confirm_linked: bool,
}

/// 获取 provenance 原文请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetNativeRuleProvenanceRequest {
    /// 文档 ID。
    pub document_id: String,
}

/// provenance 只读视图（原文经敏感名称/URL query 脱敏）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeRuleProvenanceView {
    /// 来源格式。
    pub format: String,
    /// 导入适配器版本。
    pub adapter_version: String,
    /// 输入原文 hash。
    pub input_hash: String,
    /// 导入诊断。
    pub diagnostics: Vec<lj_rule_model::Diagnostic>,
    /// 导入时间（UTC epoch 毫秒）。
    pub imported_at_ms: i64,
    /// 脱敏后的只读原文。
    pub masked_text: String,
}
