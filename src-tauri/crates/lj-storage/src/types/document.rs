//! 原生规则文档生命周期 DTO。
//!
//! 全部字段 `snake_case` 直通 wire；`definition_json`/`manifest_json`/`diagnostics_json` 均为
//! 调用方序列化好的 JSON 文本。provenance 原文与凭证明文只作为一次性请求字段存在，
//! 立即写入 secret artifact 后即丢弃，绝不进入任何持久化 JSON 或日志 DTO。

use serde::{Deserialize, Serialize};
/// 创建原生规则文档的请求。
///
/// `initial` 携带可选的初始 semantic/layout 快照；`provenance` 可选，供第三方导入
/// 流程记录来源与原文（原文立即加密，表内只留随机 secret 引用）。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateDocumentRequest {
    /// 文档 ID；原生文档由 `native:<uuid>` 形式生成。
    pub document_id: String,
    /// 文档格式；current 仅支持 `native_rule`。
    pub format: String,
    /// 展示标题；不进入 definition 或 hash。
    pub title: String,
    /// 文档对应的 source identity（UNIQUE）。
    pub source_identity: String,
    /// 初始内容快照。
    pub initial: DocumentInitial,
    /// 可选导入 provenance（原文立即加密保存）。
    pub provenance: Option<ProvenanceCreateInput>,
    /// 安全 trace 标识。
    pub trace_id: String,
    /// 发生时刻（UTC epoch milliseconds）。
    pub occurred_at_ms: i64,
}

/// 创建时的初始内容快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentInitial {
    /// 初始草稿 semantic 快照；不提供则 `semantic_revision` 从 0 开始。
    pub semantic: Option<SemanticSnapshot>,
    /// 初始 Effective Rule Revision；必须与初始草稿 semantic 相同。
    pub effective_semantic: Option<SemanticSnapshot>,
    /// 初始 layout 快照；不提供则 `layout_revision` 从 0 开始。
    pub layout: Option<LayoutSnapshot>,
}

/// 单次 semantic 快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticSnapshot {
    /// 快照 revision；创建时固定为 1。
    pub revision: i64,
    /// canonical masked definition JSON 文本。
    pub definition_json: String,
    /// definition 的 BLAKE3 hash。
    pub definition_hash: String,
    /// credential manifest JSON 文本。
    pub manifest_json: String,
}

/// 单次 layout 快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutSnapshot {
    /// 快照 revision；创建时固定为 1。
    pub revision: i64,
    /// layout JSON 文本。
    pub layout_json: String,
}

/// 导入 provenance 写入输入；原文立即加密为 secret artifact。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceCreateInput {
    /// 来源格式标识（如 `legado`）。
    pub format: String,
    /// 导入适配器版本。
    pub adapter_version: String,
    /// 输入源的 BLAKE3 hash。
    pub input_hash: String,
    /// 来源原文；一次性明文，写入 secret 后即丢弃。
    pub source_text: String,
    /// 导入诊断 JSON 文本。
    pub diagnostics_json: String,
    /// 导入发生时刻（UTC epoch milliseconds）。
    pub imported_at_ms: i64,
}

/// 保存文档内容（semantic/layout 分域乐观并发）的请求。
///
/// semantic 保存可携带一次性凭证明文，所以该请求仅在 Rust writer 内部传递，不能序列化或
/// 自动格式化到日志。
#[derive(Clone, PartialEq, Eq)]
pub struct SaveDocumentRequest {
    /// 目标文档 ID。
    pub document_id: String,
    /// 待保存的 semantic 域；`None` 表示本次不写 semantic。
    pub semantic: Option<SemanticSaveInput>,
    /// 待保存的 layout 域；`None` 表示本次不写 layout。
    pub layout: Option<LayoutSaveInput>,
    /// 安全 trace 标识。
    pub trace_id: String,
    /// 发生时刻（UTC epoch milliseconds）。
    pub occurred_at_ms: i64,
}

/// semantic 保存后的持久化状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticActivation {
    /// 保存为 Rule Draft Revision，不替换现有 Effective Rule Revision。
    Draft,
    /// 保存后替换 Effective Rule Revision。
    Effective,
}

/// 文档语义保存时一次性传入的凭证变更类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentCredentialMutationAction {
    /// 用一次性明文写入新的 secret artifact。
    Replace,
    /// 移除当前 Draft Revision 中的槽位。
    Clear,
}

/// 文档语义保存时的一次性凭证变更。
///
/// `value` 只在 writer transaction 内加密，故意不实现 `Debug` 或 serde，避免它进入日志、
/// IPC 或持久化 DTO。
#[derive(Clone, PartialEq, Eq)]
pub struct DocumentCredentialMutation {
    /// 凭证所在 Flow 节点 ID。
    pub node_id: String,
    /// 节点配置内指向 secret-capable 字段的 JSON pointer。
    pub json_pointer: String,
    /// 凭证逻辑名（敏感名称策略由 `RuleSystem` 层负责）。
    pub logical_name: String,
    /// 变更类型。
    pub action: DocumentCredentialMutationAction,
    /// replace 的一次性明文值；clear 必须为空。
    pub value: Option<String>,
}

/// semantic 域保存输入。
#[derive(Clone, PartialEq, Eq)]
pub struct SemanticSaveInput {
    /// 客户端所知的已保存 revision；等于当前 revision 才接受并写 current+1。
    pub expected_revision: i64,
    /// canonical masked definition JSON 文本。
    pub definition_json: String,
    /// definition 的 BLAKE3 hash。
    pub definition_hash: String,
    /// 一次性凭证变更；由同一 writer transaction 与 semantic snapshot 一起提交。
    pub credential_mutations: Vec<DocumentCredentialMutation>,
    /// 成功保存后是否替换 Effective Rule Revision。
    pub activation: SemanticActivation,
}

/// layout 域保存输入。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutSaveInput {
    /// 客户端所知的已保存 revision；等于当前 revision 才接受并写 current+1。
    pub expected_revision: i64,
    /// layout JSON 文本。
    pub layout_json: String,
}

/// 保存请求的分域结果；每个请求携带的域都有对应 outcome。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveDocumentOutcome {
    /// 目标文档 ID。
    pub document_id: String,
    /// semantic 域结果（请求携带时存在）。
    pub semantic: Option<DomainSaveOutcome>,
    /// layout 域结果（请求携带时存在）。
    pub layout: Option<DomainSaveOutcome>,
}

/// Effective Rule Revision 的只读安全摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleRevisionHistoryRecord {
    /// Effective Rule Revision 编号。
    pub revision: i64,
    /// canonical Definition BLAKE3。
    pub definition_hash: String,
    /// 该版本成为 Effective 的时刻（UTC epoch milliseconds）。
    pub effective_at_ms: i64,
}

/// 从历史 Effective Rule Revision 创建新 Draft 的请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreDocumentRevisionRequest {
    /// 目标文档 ID。
    pub document_id: String,
    /// 要恢复的历史 Effective Rule Revision。
    pub revision: i64,
    /// 客户端已知的当前 Draft Revision。
    pub expected_revision: i64,
    /// 安全 trace 标识。
    pub trace_id: String,
    /// 发生时刻（UTC epoch milliseconds）。
    pub occurred_at_ms: i64,
}

/// 历史恢复结果；恢复只产生 Draft，不会直接替换 Effective。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreDocumentRevisionOutcome {
    /// 目标文档 ID。
    pub document_id: String,
    /// 成功创建的 Draft Revision；冲突时为当前 revision。
    pub revision: i64,
    /// 当前 Draft Revision 冲突。
    pub conflict: Option<RevisionConflict>,
}

/// 单个域的保存结果：成功返回新 revision；冲突返回当前已保存 revision。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainSaveOutcome {
    /// 成功时为写入后的 revision，冲突时为当前已保存 revision。
    pub revision: i64,
    /// 乐观并发冲突；`None` 表示该域已成功写入。
    pub conflict: Option<RevisionConflict>,
    /// semantic 域成功写入后的状态；布局域和冲突时为空。
    pub activation: Option<SemanticActivation>,
}

/// 单域乐观并发冲突详情。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionConflict {
    /// 调用方期望的 revision。
    pub expected: i64,
    /// 当前已保存 revision。
    pub current: i64,
}

/// 文档列表与创建/重命名返回的安全摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSummary {
    /// 文档 ID。
    pub document_id: String,
    /// 文档格式。
    pub format: String,
    /// 展示标题。
    pub title: String,
    /// 文档对应的 source identity。
    pub source_identity: String,
    /// 文档状态：`draft` 或 `linked`。
    pub state: String,
    /// 当前 semantic revision；0 表示尚未保存过 semantic。
    pub semantic_revision: i64,
    /// 当前 layout revision；0 表示尚未保存过 layout。
    pub layout_revision: i64,
    /// 当前 link revision；current 恒为 0。
    pub link_revision: i64,
    /// 创建时刻（UTC epoch milliseconds）。
    pub created_at_ms: i64,
    /// 最近更新时刻（UTC epoch milliseconds）。
    pub updated_at_ms: i64,
}

/// 文档详情；summary 与当前各域快照在同一只读 transaction 内取得。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentDetail {
    /// 文档摘要。
    pub summary: DocumentSummary,
    /// 当前草稿 semantic 快照；未保存过则为 `None`。
    pub semantic: Option<SemanticSnapshot>,
    /// 最近一次有效 semantic 快照；从未生效则为 `None`。
    pub effective_semantic: Option<SemanticSnapshot>,
    /// 当前 Effective Rule Revision 的只读摘要。
    pub effective_summary: Option<RuleRevisionHistoryRecord>,
    /// 当前 layout 快照；未保存过则为 `None`。
    pub layout: Option<LayoutSnapshot>,
    /// 导入 provenance 摘要；原生创建（无导入）为 `None`。
    pub provenance: Option<ProvenanceSummary>,
}

/// 导入 provenance 的安全摘要；不含 secret ID 与原文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceSummary {
    /// 来源格式标识。
    pub format: String,
    /// 导入适配器版本。
    pub adapter_version: String,
    /// 输入源 BLAKE3 hash。
    pub input_hash: String,
    /// 导入诊断 JSON 文本。
    pub diagnostics_json: String,
    /// 导入发生时刻（UTC epoch milliseconds）。
    pub imported_at_ms: i64,
}

/// 重命名文档请求。
///
/// `expected_revision` 是乐观并发守卫：仅在当前 semantic revision 匹配时应用；标题是
/// 展示 metadata，不推进任何 revision、不改变 definition hash。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameDocumentRequest {
    /// 目标文档 ID。
    pub document_id: String,
    /// 新标题。
    pub title: String,
    /// 客户端所知的当前 semantic revision。
    pub expected_revision: i64,
    /// 安全 trace 标识。
    pub trace_id: String,
    /// 发生时刻（UTC epoch milliseconds）。
    pub occurred_at_ms: i64,
}

/// 删除文档请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteDocumentRequest {
    /// 目标文档 ID。
    pub document_id: String,
    /// 文档处于 `linked` 状态时要求显式确认；`false` 触发守卫错误。
    pub confirm_linked: bool,
    /// 安全 trace 标识。
    pub trace_id: String,
    /// 发生时刻（UTC epoch milliseconds）。
    pub occurred_at_ms: i64,
}
