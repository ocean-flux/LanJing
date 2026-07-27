//! 来源作者诊断的稳定 wire primitive。
//!
//! 本模块只承载可序列化的严重级别、支持分类与 UTF-8 字节定位；展示文案由调用方按稳定
//! `code` 提供，避免把原文、credential 或 effect material 混入跨层诊断。

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

/// 作者字段在当前运行时中的支持分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportClass {
    /// 已安全映射到现有 importer/runtime。
    Executable,
    /// 原文保留，但不进入 `RuleDefinition` 或 `ExecutionPlan`。
    Preserved,
    /// 已知字段会触发当前未支持或不安全的行为，必须阻止导入。
    Blocked,
    /// catalog 未声明的字段；作者文档保留，但内部模型不会消费。
    Unknown,
}

/// Rust/TypeScript 共享的作者诊断 wire。
///
/// `byte_offset` 与 `byte_length` 均以原始 UTF-8 字节计数，定位半开区间
/// `[byte_offset, byte_offset + byte_length)`；`path` 使用 RFC 6901 JSON Pointer。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringDiagnostic {
    /// 严重级别。
    pub severity: DiagnosticSeverity,
    /// 稳定机器码；不得包含输入片段。
    pub code: String,
    /// RFC 6901 JSON Pointer，根为 `""`。
    pub path: String,
    /// 原始 UTF-8 文档中的起始字节。
    pub byte_offset: usize,
    /// 原始 UTF-8 文档中的字节长度。
    pub byte_length: usize,
    /// 当前字段或失败条件的支持分类。
    pub support: SupportClass,
}

impl AuthoringDiagnostic {
    /// 构造不含展示文案和输入内容的稳定诊断。
    #[must_use]
    pub fn new(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        path: impl Into<String>,
        byte_offset: usize,
        byte_length: usize,
        support: SupportClass,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            path: path.into(),
            byte_offset,
            byte_length,
            support,
        }
    }
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
