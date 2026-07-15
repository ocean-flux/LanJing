//! HTTP 协议节点 crate。
//!
//! 实现 HTTP 网络请求节点的处理器，负责发送 HTTP 请求、
//! 管理 Cookie、处理响应流，并提供 SSRF 防护机制。

pub mod processor;
pub mod ssrf;
pub mod util;
