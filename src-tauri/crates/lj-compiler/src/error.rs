//! 规则编译器错误类型。

use thiserror::Error;

/// 规则编译器错误。
#[derive(Debug, Error)]
pub enum CompilerError {
    /// 规则语法错误。
    #[error("规则语法错误: {0}")]
    SyntaxError(String),

    /// 不支持的选择器类型。
    #[error("不支持的选择器类型: {0}")]
    UnsupportedSelector(String),

    /// 不支持的规则版本。
    #[error("不支持的规则版本: {0}")]
    UnsupportedVersion(String),

    /// 内部错误。
    #[error("内部错误: {0}")]
    Internal(String),
}
