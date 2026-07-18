//! 诊断 DTO — 校验/编译/执行共享。

use serde::{Deserialize, Serialize};

use crate::definition::SourceSpan;

/// 诊断严重级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    /// 提示。
    Info,
    /// 警告。
    Warning,
    /// 错误。
    Error,
}

/// 可定位诊断。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// 稳定错误码。
    pub code: String,
    /// 严重级别。
    pub severity: DiagnosticSeverity,
    /// 安全可读消息（不含 secret/body）。
    pub message: String,
    /// 可选源码定位。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
}
