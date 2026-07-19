//! `QuickJS` Plan effect adapter crate。
//!
//! 在受限的 blocking lane 内执行 QuickJS，并提供安全宿主 API。

pub mod error;
pub mod host_api;
pub mod processor;

pub use processor::execute_js_blocking_cancellable;
