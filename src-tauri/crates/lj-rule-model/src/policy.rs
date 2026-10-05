//! 能力与策略 DTO — 可序列化 policy 合同。

use serde::{Deserialize, Serialize};

/// 系统级沙箱能力（文件系统/环境变量/进程）。
///
/// 网络不属于受控能力：应用总是允许联网，只有我们提供的系统 API 走 capability 控制。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemCapabilities {
    /// 是否允许文件系统访问。
    pub fs: bool,
    /// 是否允许环境变量访问。
    pub env: bool,
    /// 是否允许进程操作。
    pub process: bool,
}

/// 能力类别枚举（用于 `CapabilityBlocked` 错误）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capability {
    /// 文件系统。
    Fs,
    /// 环境变量。
    Env,
    /// 进程。
    Process,
}

/// 能力被阻止时返回的错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CapabilityError {
    /// 能力被阻止。
    #[error("能力被阻止: {0:?}")]
    Blocked(Capability),
}
