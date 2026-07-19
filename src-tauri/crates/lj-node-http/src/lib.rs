//! HTTP Plan effect adapter crate。
//!
//! 负责受能力约束的网络请求、敏感凭据注入、取消和 SSRF 防护。

pub mod processor;
pub mod ssrf;
pub mod util;
