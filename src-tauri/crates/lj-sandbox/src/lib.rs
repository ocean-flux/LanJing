//! 沙箱层 crate。
//!
//! 定义能力隔离（Capabilities）模型，控制脚本/节点的权限边界，
//! 如网络访问、文件系统、执行时间等限制。

pub mod capabilities;

pub use capabilities::{check_capability, default_capabilities, merge};
