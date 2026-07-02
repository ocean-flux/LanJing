//! JavaScript 节点 crate。
//!
//! 基于 QuickJS（rquickjs）实现 JS 脚本执行节点，
//! 提供宿主 API 绑定，支持在沙箱中安全执行用户脚本。

pub mod error;
pub mod host_api;
pub mod processor;

pub use processor::execute_js_blocking;
