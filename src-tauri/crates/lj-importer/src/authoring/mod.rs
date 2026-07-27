//! Legado authoring 的正式 catalog、严格文档 validation、credential codec 与 raw importer。
//!
//! `text` 仍是作者保存真相；本模块不会改写未知/保留字段，也不会把 blocked/unknown 字段塞入
//! `RuleDefinition`。常规 runtime 与 tests 只读取已提交生成物或 Rust 常量，不依赖 `.tmp`。

pub mod catalog;
mod credential;
mod document;

use std::fmt;

use lj_rule_model::{
    AuthoringDiagnostic, CredentialSlotManifest, CredentialTargetIdentity, DiagnosticSeverity,
    SupportClass,
};

use crate::legado::{LegadoDefinition, LegadoImporter, LegadoSourceJson};

pub use credential::{
    CredentialCodecError, CredentialRebase, CredentialRebaseResolution, CredentialSecret,
    CredentialSlotCodec, CredentialSplit,
};
pub use document::{Utf8ByteSpan, locate_json_pointer, validate_legado_document};

/// 完整 raw authoring 文档成功进入既有 Legado adapter 的结果。
pub struct LegadoAuthoringImport {
    /// 只含 executable 字段投影与分离 credential snapshot 的既有 adapter 结果。
    pub adapted: LegadoDefinition,
    /// preserved/unknown 的非阻断 diagnostics；原文字段不会进入 Definition/Plan。
    pub diagnostics: Vec<AuthoringDiagnostic>,
}

impl fmt::Debug for LegadoAuthoringImport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LegadoAuthoringImport")
            .field(
                "source_identity",
                &self.adapted.definition.source_identity().id,
            )
            .field("has_credentials", &self.adapted.has_credentials())
            .field("diagnostics", &self.diagnostics)
            .finish()
    }
}

/// raw Legado 文档在进入 importer 前的阻断失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegadoAuthoringImportError {
    diagnostics: Vec<AuthoringDiagnostic>,
}

impl LegadoAuthoringImportError {
    /// 返回不含文案或原文的稳定 diagnostics。
    #[must_use]
    pub fn diagnostics(&self) -> &[AuthoringDiagnostic] {
        &self.diagnostics
    }

    fn one(diagnostic: AuthoringDiagnostic) -> Self {
        Self {
            diagnostics: vec![diagnostic],
        }
    }
}

impl fmt::Display for LegadoAuthoringImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            self.diagnostics
                .first()
                .map_or("legado_authoring_invalid", |diagnostic| {
                    diagnostic.code.as_str()
                }),
        )
    }
}

impl std::error::Error for LegadoAuthoringImportError {}

impl LegadoImporter {
    /// 严格验证完整 raw JSON，再把 executable 字段映射到既有安全 runtime。
    ///
    /// preserved/unknown 只留在原文与返回 diagnostics；blocked、duplicate、limits、invalid root、
    /// ambiguous credential header 或任意 sentinel 都不会进入 typed source、Definition 或 Plan。
    ///
    /// # Errors
    ///
    /// authoring validation 存在 error、文档仍含 sentinel、typed Legado 反序列化或 adapter 失败时
    /// 返回 [`LegadoAuthoringImportError`]。
    pub fn import_document(
        &self,
        text: &str,
    ) -> Result<LegadoAuthoringImport, LegadoAuthoringImportError> {
        let outcome = document::analyze_legado_document(text);
        if let Some(diagnostic) = outcome
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        {
            return Err(LegadoAuthoringImportError::one(diagnostic.clone()));
        }
        let parsed = outcome.document.ok_or_else(|| {
            LegadoAuthoringImportError::one(blocked_import_diagnostic(
                "invalid_json",
                0,
                text.len().min(1),
            ))
        })?;
        credential::reject_existing_sentinel(&parsed)
            .map_err(|error| LegadoAuthoringImportError::one(error.diagnostic))?;
        credential::validate_legado_header(&parsed)
            .map_err(|error| LegadoAuthoringImportError::one(error.diagnostic))?;
        let source = serde_json::from_str::<LegadoSourceJson>(text).map_err(|_| {
            LegadoAuthoringImportError::one(blocked_import_diagnostic(
                "invalid_field_type",
                parsed.nodes[parsed.root].span.byte_offset,
                parsed.nodes[parsed.root].span.byte_length,
            ))
        })?;
        let adapted = self.adapt(&source).map_err(|_| {
            LegadoAuthoringImportError::one(blocked_import_diagnostic(
                "legado_definition_invalid",
                parsed.nodes[parsed.root].span.byte_offset,
                parsed.nodes[parsed.root].span.byte_length,
            ))
        })?;
        Ok(LegadoAuthoringImport {
            adapted,
            diagnostics: outcome.diagnostics,
        })
    }

    /// 严格恢复一个 target-bound masked 文档后立即走 raw importer。
    ///
    /// # Errors
    ///
    /// manifest/slot/sentinel/target 不匹配，或恢复后的 Legado 文档无法导入时返回
    /// [`LegadoAuthoringImportError`]。
    pub fn import_masked_document(
        &self,
        masked_text: &str,
        manifest: &CredentialSlotManifest,
        secrets: &[CredentialSecret],
        expected_target: &CredentialTargetIdentity,
    ) -> Result<LegadoAuthoringImport, LegadoAuthoringImportError> {
        let text =
            CredentialSlotCodec::reconstitute(masked_text, manifest, secrets, expected_target)
                .map_err(|error| LegadoAuthoringImportError::one(error.diagnostic))?;
        self.import_document(&text)
    }
}

fn blocked_import_diagnostic(
    code: &str,
    byte_offset: usize,
    byte_length: usize,
) -> AuthoringDiagnostic {
    AuthoringDiagnostic::new(
        DiagnosticSeverity::Error,
        code,
        "",
        byte_offset,
        byte_length,
        SupportClass::Blocked,
    )
}
