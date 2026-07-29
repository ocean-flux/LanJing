//! 格式 adapter 与 `RuleSystem` 共享的一次性导入结果。
//!
//! 第三方输入止于此边界，不保留原文或可编辑格式状态。provenance 仅包含格式标记与内容
//! hash；credential bytes 只能移入加密的应用事务。

use std::fmt;

use lj_rule_model::{DiagnosticSeverity, RuleDefinition};

/// 支持的单向导入格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    /// Legado 书源 JSON。
    Legado,
    /// Maccms10 endpoint JSON。
    Maccms10,
}

/// 输入字段在 current adapter 中的支持分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSupport {
    /// 映射到原生 Definition。
    Executable,
    /// 已识别但由原生 Definition 明确忽略的 metadata。
    Ignored,
    /// 已知但无法安全导入的行为。
    Blocked,
    /// current adapter 未识别的字段。
    Unknown,
}

/// 不包含来源片段或 credential value 的稳定诊断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDiagnostic {
    /// 诊断级别。
    pub severity: DiagnosticSeverity,
    /// 稳定的机器可读 code。
    pub code: String,
    /// RFC 6901 path；空字符串表示根节点。
    pub path: String,
    /// 原始 UTF-8 输入中的起始 offset。
    pub byte_offset: usize,
    /// UTF-8 byte length。
    pub byte_length: usize,
    /// current adapter 的支持分类。
    pub support: ImportSupport,
}

impl ImportDiagnostic {
    /// 创建不保留输入原文的诊断。
    #[must_use]
    pub fn new(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        path: impl Into<String>,
        byte_offset: usize,
        byte_length: usize,
        support: ImportSupport,
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

/// 只包含稳定诊断的安全导入错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportError {
    diagnostics: Vec<ImportDiagnostic>,
}

impl ImportError {
    pub(crate) fn new(diagnostics: Vec<ImportDiagnostic>) -> Self {
        Self { diagnostics }
    }

    pub(crate) fn blocked(code: impl Into<String>, byte_length: usize) -> Self {
        Self::new(vec![ImportDiagnostic::new(
            DiagnosticSeverity::Error,
            code,
            "",
            0,
            byte_length,
            ImportSupport::Blocked,
        )])
    }

    /// 返回不含来源片段或 credential value 的诊断。
    #[must_use]
    pub fn diagnostics(&self) -> &[ImportDiagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let diagnostic = self
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
            .or_else(|| self.diagnostics.first());
        formatter.write_str(diagnostic.map_or("source_import_invalid", |value| value.code.as_str()))
    }
}

impl std::error::Error for ImportError {}

/// 已导入规则的安全、去敏 provenance。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportProvenance {
    /// 接受该输入的 adapter。
    pub format: ImportFormat,
    /// 输入的 BLAKE3 hash；不保留原文。
    pub input_hash: String,
}

/// 不透明的 runtime credential bytes。
///
/// 此 carrier 故意不实现 `Debug`、`Clone` 或 serde。
pub struct ImportCredentialMaterial {
    bytes: Vec<u8>,
}

impl ImportCredentialMaterial {
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// 将 credential bytes 移入加密的应用事务。
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// 第三方格式的单向导入结果。
///
/// 此类型可能包含短生命周期 credential material，因此故意不实现 `Debug`、`Clone` 或 serde。
pub struct ImportedNativeRule {
    /// current 原生 Definition。
    pub definition: RuleDefinition,
    /// 从非敏感输入 metadata 得到的可选展示标题。
    pub display_title: Option<String>,
    /// 从非敏感输入 metadata 得到的可选展示分组。
    pub display_group: Option<String>,
    /// 已识别但忽略或未知字段的安全诊断。
    pub diagnostics: Vec<ImportDiagnostic>,
    /// 仅含 hash 的来源 provenance。
    pub provenance: ImportProvenance,
    credentials: Option<ImportCredentialMaterial>,
}

impl ImportedNativeRule {
    pub(crate) fn new(
        definition: RuleDefinition,
        display_title: Option<String>,
        display_group: Option<String>,
        diagnostics: Vec<ImportDiagnostic>,
        format: ImportFormat,
        input: &str,
        credential_bytes: Option<Vec<u8>>,
    ) -> Self {
        Self {
            definition,
            display_title,
            display_group,
            diagnostics,
            provenance: ImportProvenance {
                format,
                input_hash: blake3::hash(input.as_bytes()).to_hex().to_string(),
            },
            credentials: credential_bytes.map(ImportCredentialMaterial::new),
        }
    }

    /// 将短生命周期 credential 移出导入结果。
    #[must_use]
    pub fn take_credentials(&mut self) -> Option<ImportCredentialMaterial> {
        self.credentials.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_error_display_prefers_blocking_diagnostic() {
        let error = ImportError::new(vec![
            ImportDiagnostic::new(
                DiagnosticSeverity::Info,
                "known_field_ignored",
                "/comment",
                1,
                1,
                ImportSupport::Ignored,
            ),
            ImportDiagnostic::new(
                DiagnosticSeverity::Error,
                "known_field_blocked",
                "/loginUrl",
                2,
                1,
                ImportSupport::Blocked,
            ),
        ]);

        assert_eq!(error.to_string(), "known_field_blocked");
    }
}
