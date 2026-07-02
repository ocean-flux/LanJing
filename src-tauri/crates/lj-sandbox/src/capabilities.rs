//! 沙箱能力管理 — Capabilities 默认值 + 合并逻辑(ADR-0008)。
//!
//! 首刀只验证全局默认(network=true, fs/env/process=false)。
//! 源级 partial override 推迟到切通后 grill 边界再补。

use lj_core::sandbox::{Capability, Sandbox, SystemCapabilities};

/// 默认能力配置：network=true, fs/env/process=false。
#[must_use]
pub fn default_capabilities() -> Sandbox {
    Sandbox {
        network: true,
        system: SystemCapabilities {
            fs: false,
            env: false,
            process: false,
        },
    }
}

/// 合并全局能力和源级能力（首刀：源级只能收紧不能放宽）。
///
/// 即取交集——两边都允许才允许。
/// 首刀无 partial override，merge 逻辑简单。
#[must_use]
pub fn merge(global: &Sandbox, source: &Sandbox) -> Sandbox {
    Sandbox {
        network: global.network && source.network,
        system: SystemCapabilities {
            fs: global.system.fs && source.system.fs,
            env: global.system.env && source.system.env,
            process: global.system.process && source.system.process,
        },
    }
}

/// 检查能力是否允许，不允许则返回 [`CapabilityError`]。
///
/// # Errors
///
/// 如果该能力被禁用，返回 [`CapabilityError::Blocked`]。
pub fn check_capability(
    caps: &Sandbox,
    cap: Capability,
) -> Result<(), lj_core::sandbox::CapabilityError> {
    let allowed = match cap {
        Capability::Network => caps.network,
        Capability::Fs => caps.system.fs,
        Capability::Env => caps.system.env,
        Capability::Process => caps.system.process,
    };
    if allowed {
        Ok(())
    } else {
        Err(lj_core::sandbox::CapabilityError::Blocked(cap))
    }
}
