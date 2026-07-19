//! Plan runtime 的能力检查归属模块。

use lj_rule_model::{Capability, CapabilityError, PolicyCapabilities, SystemCapabilities};

/// 默认能力配置：network=true, fs/env/process=false。
#[must_use]
pub fn default_capabilities() -> PolicyCapabilities {
    PolicyCapabilities {
        network: true,
        system: SystemCapabilities {
            fs: false,
            env: false,
            process: false,
        },
    }
}

/// 合并全局能力和源级能力（源级只能收紧不能放宽）。
///
/// 即取交集——两边都允许才允许。
#[must_use]
pub fn merge(global: &PolicyCapabilities, source: &PolicyCapabilities) -> PolicyCapabilities {
    PolicyCapabilities {
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
pub fn check_capability(caps: &PolicyCapabilities, cap: Capability) -> Result<(), CapabilityError> {
    let allowed = match cap {
        Capability::Network => caps.network,
        Capability::Fs => caps.system.fs,
        Capability::Env => caps.system.env,
        Capability::Process => caps.system.process,
    };
    if allowed {
        Ok(())
    } else {
        Err(CapabilityError::Blocked(cap))
    }
}
