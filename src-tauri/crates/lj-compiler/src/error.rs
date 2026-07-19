//! 规则编译器错误类型。

use lj_rule_model::Diagnostic;
use thiserror::Error;

/// 规则 compiler 的失败。
#[derive(Debug, Error)]
pub enum CompilerError {
    /// 来源专有规则语法错误。
    #[error("规则语法错误: {0}")]
    SyntaxError(String),

    /// 不支持的选择器类型。
    #[error("不支持的选择器类型: {0}")]
    UnsupportedSelector(String),

    /// 不支持的规则版本。
    #[error("不支持的规则版本: {0}")]
    UnsupportedVersion(String),

    /// Definition 未通过可定位校验。
    #[error("Definition 校验失败")]
    Validation {
        /// 全部错误诊断，供作者修复而非只显示第一个错误。
        diagnostics: Vec<Diagnostic>,
    },

    /// Plan 规范化序列化失败。
    #[error("Plan 序列化失败: {0}")]
    Serialization(String),

    /// compiler 内部不变量被破坏。
    #[error("compiler 内部错误: {0}")]
    Internal(String),
}

impl CompilerError {
    /// 从诊断集合构造校验失败。
    #[must_use]
    pub fn validation(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Validation { diagnostics }
    }

    /// 返回校验失败中的完整诊断；其他错误返回空切片。
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Self::Validation { diagnostics } => diagnostics,
            Self::SyntaxError(_)
            | Self::UnsupportedSelector(_)
            | Self::UnsupportedVersion(_)
            | Self::Serialization(_)
            | Self::Internal(_) => &[],
        }
    }
}
