//! HTTP Plan effect adapter 与受限导入拉取 crate。
//!
//! 负责受能力约束的网络请求、敏感凭据注入、取消和目标解析(DNS pin)；导入 façade 复用同一
//! URL/redirect/DNS pin 边界，但使用更严格的 body 与总超时上限。
//!
//! 目标不做白名单限制：任意可解析的 http(s) 主机都可请求，风险由用户承担(见 `SECURITY.md`)。
pub mod processor;
pub mod target;
pub mod util;

pub use processor::{IMPORT_BODY_MAX_BYTES, ImportFetchError, fetch_import_source};
