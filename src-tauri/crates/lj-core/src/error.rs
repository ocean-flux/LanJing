//! lj-core 错误类型。

use crate::sandbox::Capability;

/// lj-core 错误。
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// 导入错误。
    #[error("导入错误: {0}")]
    Import(String),
    /// 图验证失败。
    #[error("图验证失败: {0}")]
    GraphValidation(String),
    /// 节点执行错误。
    #[error("节点执行错误: {0}")]
    NodeExecution(String),
    /// 存储错误。
    #[error("存储错误: {0}")]
    Storage(String),
    /// 能力被阻止。
    #[error("能力被阻止: {0:?}")]
    CapabilityBlocked(Capability),
    /// HTTP body 超过上限。
    #[error("HTTP body 超过上限: {actual} bytes (max {max} bytes)")]
    BodyTooLarge {
        /// 实际 body 大小。
        actual: usize,
        /// 上限。
        max: usize,
    },
    /// SSRF 防护:目标地址被阻止。
    #[error("SSRF 防护: 目标地址被阻止: {0}")]
    SsrfBlocked(String),
    /// JSON 序列化/反序列化错误。
    #[error("JSON 序列化/反序列化错误: {0}")]
    Json(#[from] serde_json::Error),
    /// 其他错误。
    #[error("其他错误: {0}")]
    Other(String),
}
