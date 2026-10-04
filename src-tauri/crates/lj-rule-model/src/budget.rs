//! 受控节点的资源预算合同。
//!
//! 规则在 config 里声明的是**请求值**；host policy 持有上限，生效值取两边的交集
//! （[`JsBudgetCeiling::clamp`]）。因此规则无法通过声明把自己的资源上限抬高到 host
//! policy 之上，超限是稳定错误而不是静默降级。

use serde::{Deserialize, Serialize};

/// 受控 JS 节点的资源预算（规则声明的一部分，也是 effect 请求里的生效值）。
///
/// 三个维度都必须显式声明：规则作者看得见自己申请了多少资源，诊断也才能指认是哪一项超限。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsBudget {
    /// 单次执行 wall-clock 上限（毫秒）。
    pub timeout_ms: u32,
    /// `QuickJS` runtime 堆上限（字节）。
    pub memory_bytes: u64,
    /// 脚本输出上限（字节）；超限返回稳定错误，不静默截断。
    pub output_bytes: u64,
}

impl JsBudget {
    /// 内置 runtime 的 host policy 上限，同时是新节点的默认声明。
    ///
    /// 这是 host policy 上限与 descriptor 默认值唯一的一份数值来源：两处各写一遍必然漂移。
    pub const HOST_CEILING: Self = Self {
        timeout_ms: 5_000,
        memory_bytes: 16 * 1024 * 1024,
        output_bytes: 1024 * 1024,
    };
}

impl Default for JsBudget {
    fn default() -> Self {
        Self::HOST_CEILING
    }
}

/// host policy 允许的预算上限。
///
/// 与 [`JsBudget`] 分开是有意的：请求值和上限不能互相顶替，类型不同才不会传错方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JsBudgetCeiling {
    /// 允许的最大 wall-clock（毫秒）。
    pub timeout_ms: u32,
    /// 允许的最大堆（字节）。
    pub memory_bytes: u64,
    /// 允许的最大输出字节数。
    pub output_bytes: u64,
}

impl JsBudgetCeiling {
    /// 内置 runtime 的 host policy 上限。
    pub const HOST_POLICY: Self = Self {
        timeout_ms: JsBudget::HOST_CEILING.timeout_ms,
        memory_bytes: JsBudget::HOST_CEILING.memory_bytes,
        output_bytes: JsBudget::HOST_CEILING.output_bytes,
    };

    /// 取规则声明与上限的交集；每项至少 1，避免声明 0 造成「必然超限」的死配置。
    #[must_use]
    pub const fn clamp(&self, requested: JsBudget) -> JsBudget {
        JsBudget {
            timeout_ms: clamp_u32(requested.timeout_ms, self.timeout_ms),
            memory_bytes: clamp_u64(requested.memory_bytes, self.memory_bytes),
            output_bytes: clamp_u64(requested.output_bytes, self.output_bytes),
        }
    }
}

impl Default for JsBudgetCeiling {
    fn default() -> Self {
        Self::HOST_POLICY
    }
}

const fn clamp_u32(requested: u32, ceiling: u32) -> u32 {
    let bounded = if requested < ceiling {
        requested
    } else {
        ceiling
    };
    if bounded == 0 { 1 } else { bounded }
}

const fn clamp_u64(requested: u64, ceiling: u64) -> u64 {
    let bounded = if requested < ceiling {
        requested
    } else {
        ceiling
    };
    if bounded == 0 { 1 } else { bounded }
}

#[cfg(test)]
mod tests {
    use super::{JsBudget, JsBudgetCeiling};

    #[test]
    fn clamp_caps_every_field_at_the_host_ceiling() {
        let requested = JsBudget {
            timeout_ms: u32::MAX,
            memory_bytes: u64::MAX,
            output_bytes: u64::MAX,
        };
        let effective = JsBudgetCeiling::HOST_POLICY.clamp(requested);
        assert_eq!(effective, JsBudget::HOST_CEILING);
    }

    #[test]
    fn clamp_keeps_a_tighter_declaration() {
        let requested = JsBudget {
            timeout_ms: 250,
            memory_bytes: 1024,
            output_bytes: 2048,
        };
        assert_eq!(JsBudgetCeiling::HOST_POLICY.clamp(requested), requested);
    }

    #[test]
    fn clamp_never_yields_a_zero_budget() {
        let requested = JsBudget {
            timeout_ms: 0,
            memory_bytes: 0,
            output_bytes: 0,
        };
        assert_eq!(
            JsBudgetCeiling::HOST_POLICY.clamp(requested),
            JsBudget {
                timeout_ms: 1,
                memory_bytes: 1,
                output_bytes: 1,
            }
        );
    }
}
