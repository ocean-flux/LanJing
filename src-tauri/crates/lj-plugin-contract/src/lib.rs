//! plugin contract 稳定层。
//!
//! 只承载 plugin 与 operation 的 namespaced stable identity、版本词汇、manifest 声明与稳定错误。
//! 不承载校验、依赖解析、生命周期或 handler 类型：manifest 校验与依赖图属 `#36`，plugin-facing
//! handler contract 在 `#40`/`#41` 定形后迁入本 crate。

mod error;
mod identity;
mod manifest;

pub use error::PluginError;
pub use identity::{OperationId, PluginId, Version};
pub use manifest::PluginManifest;

/// 当前 host contract 版本。
///
/// plugin 用它声明自己编译时使用的 host API generation；不匹配的 manifest 由 `#36` 拒绝。
pub const HOST_CONTRACT_VERSION: &str = "1";
