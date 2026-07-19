//! HTTP Plan effect adapter 的公开入口。
//!
//! 对外保留 `HttpEffectAdapter` 与 `convert_response`。内部将 request material、安全摘要及响应
//! 读取，与逐跳 redirect/SSRF 校验和 live adapter 编排分开，避免网络安全不变量重新混入
//! 一个处理器文件。

mod adapter;
mod redirect;
mod request;

pub use adapter::HttpEffectAdapter;
pub use request::convert_response;
