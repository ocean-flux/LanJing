//! 导入器 crate。
//!
//! 负责将外部来源转换为可验证的 `RuleDefinition`：Legado 书源 JSON 与 Maccms。

mod imported_rule;
pub mod legado;
pub mod maccms;
mod strict_json;

pub use imported_rule::{
    ImportCredentialMaterial, ImportDiagnostic, ImportError, ImportFormat, ImportProvenance,
    ImportSupport, ImportedNativeRule,
};
