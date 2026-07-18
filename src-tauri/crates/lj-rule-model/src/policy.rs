//! 能力与策略 DTO — 可序列化 policy 合同。

use serde::{Deserialize, Serialize};

/// 系统级沙箱能力（文件系统/环境变量/进程）。
///
/// 与 `PolicyCapabilities.network` 分开以不超过 clippy `struct_excessive_bools` 阈值。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SystemCapabilities {
    /// 是否允许文件系统访问。
    pub fs: bool,
    /// 是否允许环境变量访问。
    pub env: bool,
    /// 是否允许进程操作。
    pub process: bool,
}

/// 策略能力配置（安装 grant / 执行沙箱边界）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PolicyCapabilities {
    /// 是否允许网络请求。
    pub network: bool,
    /// 系统级能力（fs/env/process）。
    pub system: SystemCapabilities,
}

/// 能力类别枚举（用于 `CapabilityBlocked` 错误）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capability {
    /// 网络。
    Network,
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
