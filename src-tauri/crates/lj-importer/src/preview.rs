//! 导入预览与结果类型（原跨层浅 trait 本地化）。

use lj_rule_model::{Error, PolicyCapabilities};
use lj_runtime::Graph;

/// 导入预览。
#[derive(Debug, Clone)]
pub struct ImportPreview {
    /// 源站 URL。
    pub source_url: String,
    /// 节点数量。
    pub node_count: usize,
    /// 边数量。
    pub edge_count: usize,
    /// JS 块数量。
    pub js_block_count: usize,
    /// 沙箱/策略能力配置。
    pub sandbox: PolicyCapabilities,
    /// 所有 HTTP 目标 URL 模板(SSRF 审计)。
    pub http_target_urls: Vec<String>,
    /// JS 块源码(用户审计)。
    pub js_sources: Vec<String>,
    /// 生成的图（C1 临时；C3+ 改 Definition/Plan）。
    pub graph: Graph,
}

/// 导入结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportOutcome {
    /// 新建。
    New,
    /// 更新(stub 首刀只返 New)。
    Updated,
    /// 替换(stub 首刀不使用)。
    Replaced,
}

/// 导入器结果别名。
pub type ImportResult = Result<ImportPreview, Error>;
