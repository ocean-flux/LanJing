//! Plan runtime 的能力检查归属模块。

use lj_rule_model::{Capability, CapabilityError, JsBudget, JsBudgetCeiling, SystemCapabilities};

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

/// 检查能力是否允许，不允许则返回 [`CapabilityError`]。
///
/// # Errors
///
/// 如果该能力被禁用，返回 [`CapabilityError::Blocked`]。
pub fn check_capability(caps: &SystemCapabilities, cap: Capability) -> Result<(), CapabilityError> {
    let allowed = match cap {
        Capability::Fs => caps.fs,
        Capability::Env => caps.env,
        Capability::Process => caps.process,
    };
    if allowed {
        Ok(())
    } else {
        Err(CapabilityError::Blocked(cap))
    }
}
