//! 导入器 crate。
//!
//! 负责从外部来源导入规则：Legado 书源 JSON 导入与转换、
//! 原生规则格式支持、导入校验、版本升级迁移。

pub mod legado;
pub mod maccms;
pub mod native;
pub mod preview;
pub mod upgrade;
pub mod validate;
