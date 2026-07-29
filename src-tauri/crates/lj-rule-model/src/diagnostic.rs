//! 编译与执行诊断的稳定 wire primitive。
//!
//! 展示文案由调用方按稳定 `code` 提供，避免把原文、credential 或 effect material 混入
//! 跨层诊断。

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

/// 编译/执行阶段使用的可定位诊断。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
