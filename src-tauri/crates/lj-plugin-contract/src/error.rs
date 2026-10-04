//! plugin contract 的稳定错误。

use thiserror::Error;

use crate::identity::{OperationId, PluginId};

/// plugin 注册与 contract 词汇的稳定错误。
///
/// [`PluginError::code`] 是与 message 无关的稳定标识，调用方、诊断与测试断言都以它为准。
/// 错误 message 只包含 identity 与形状原因，不包含 plugin payload。
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PluginError {
    /// plugin identity 不是规范的 namespaced identity。
    #[error("plugin identity 非法：{0}")]
    InvalidPluginId(String),
    /// operation identity 不是规范的 namespaced identity。
    #[error("operation identity 非法：{0}")]
    InvalidOperationId(String),
    /// 版本字符串形状非法。
    #[error("版本字符串非法：{0}")]
    InvalidVersion(String),
    /// 同一 plugin identity 被注册两次。
    #[error("plugin {0} 重复注册")]
    DuplicatePlugin(PluginId),
    /// 同一 operation identity 被两个 plugin 提供。
    #[error("operation {0} 已被注册")]
    DuplicateOperation(OperationId),
    /// 注册的 operation 不在 manifest 声明的 provided 列表里。
    #[error("plugin {plugin} 未在 manifest 中声明 operation {operation}")]
    UndeclaredOperation {
        /// 声明该 manifest 的 plugin。
        plugin: PluginId,
        /// 未声明的 operation。
        operation: OperationId,
    },
}

impl PluginError {
    /// 稳定错误码。
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidPluginId(_) => "plugin_id_invalid",
            Self::InvalidOperationId(_) => "operation_id_invalid",
            Self::InvalidVersion(_) => "version_invalid",
            Self::DuplicatePlugin(_) => "duplicate_plugin",
            Self::DuplicateOperation(_) => "duplicate_operation",
            Self::UndeclaredOperation { .. } => "undeclared_operation",
        }
    }
}
