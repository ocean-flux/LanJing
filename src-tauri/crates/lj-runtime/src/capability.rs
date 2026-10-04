//! Plan runtime 的能力检查归属模块。

use lj_rule_model::{
    Capability, CapabilityError, JsBudget, JsBudgetCeiling, PolicyCapabilities, SystemCapabilities,
};

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

/// 内置 runtime 的 JS 资源预算 host policy 上限。
#[must_use]
pub const fn js_budget_ceiling() -> JsBudgetCeiling {
    JsBudgetCeiling::HOST_POLICY
}

/// 规则声明的预算与 host policy 上限取交集后的生效预算。
///
/// 规则声明只能收紧，不能抬高上限。
#[must_use]
pub fn effective_js_budget(requested: JsBudget) -> JsBudget {
    js_budget_ceiling().clamp(requested)
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
