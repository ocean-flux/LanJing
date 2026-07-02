//! JavaScript 节点错误类型。

use thiserror::Error;

/// JavaScript 节点错误。
#[derive(Debug, Error)]
pub enum JsError {
    /// Runtime 创建失败。
    #[error("Runtime 创建失败: {0}")]
    RuntimeCreate(String),

    /// Context 创建失败。
    #[error("Context 创建失败: {0}")]
    ContextCreate(String),

    /// JS 执行错误。
    #[error("JS 执行错误: {0}")]
    EvalError(String),

    /// JS 执行超时。
    #[error("JS 执行超时(超过 {0}ms)")]
    Timeout(u64),

    /// 能力被阻止。
    #[error("能力被阻止: {0}")]
    CapabilityBlocked(String),
}
