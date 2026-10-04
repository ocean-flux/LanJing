//! effect registry：内置 effect handler 的唯一注册与冻结边界。
//!
//! 注册与查找键是 Rule Contract 的 [`EffectKind`]，不再维护另一套 operation identity。
//! 注册与冻结只发生在 application composition 阶段；冻结产物 [`FrozenEffectRegistry`] 之后
//! 不再变化，因此一次 execution 绑定的 handler 集合不会在运行中被替换。

pub mod builtin;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lj_rule_model::EffectKind;

use crate::effect::{ExtractEffectHandler, HttpEffectHandler, QuickJsEffectHandler};

/// 一个注册到 effect registry 的 handler。
#[derive(Clone)]
pub enum EffectHandler {
    /// HTTP effect handler。
    Http(Arc<dyn HttpEffectHandler>),
    /// `QuickJS` rule node handler。
    QuickJs(Arc<dyn QuickJsEffectHandler>),
    /// Extract effect handler。
    Extract(Arc<dyn ExtractEffectHandler>),
}

/// effect registry 的注册失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EffectRegistryError {
    /// 同一 [`EffectKind`] 已被注册，或同一批内重复。
    #[error("effect kind {0:?} 已注册")]
    DuplicateEffectKind(EffectKind),
}

impl EffectRegistryError {
    /// 稳定错误码。
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::DuplicateEffectKind(_) => "duplicate_effect_kind",
        }
    }
}

/// effect 注册边界。
///
/// 注册与冻结只发生在 application composition 阶段；冻结产物 [`FrozenEffectRegistry`] 之后
/// 不再变化，因此一次 execution 绑定的 handler 集合不会在运行中被替换。
#[derive(Default)]
pub struct EffectRegistry {
    effects: BTreeMap<EffectKind, EffectHandler>,
}

impl EffectRegistry {
    /// 创建空的注册边界。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一批 effect handler。
    ///
    /// 批量注册是原子的：重复的 effect kind 都会在写入任何 handler 之前失败，因此失败的注册
    /// 不会留下半注册的 capability。
    ///
    /// # Errors
    ///
    /// [`EffectRegistryError::DuplicateEffectKind`]：该 kind 已注册，或同一批内重复。
    pub fn register_all(
        &mut self,
        effects: Vec<(EffectKind, EffectHandler)>,
    ) -> Result<(), EffectRegistryError> {
        let mut batch = BTreeSet::new();
        for (kind, _) in &effects {
            if self.effects.contains_key(kind) || !batch.insert(kind) {
                return Err(EffectRegistryError::DuplicateEffectKind(kind.clone()));
            }
        }
        for (kind, handler) in effects {
            self.effects.insert(kind, handler);
        }
        Ok(())
    }

    /// 冻结为 immutable registry snapshot。
    #[must_use]
    pub fn freeze(self) -> FrozenEffectRegistry {
        FrozenEffectRegistry {
            effects: self.effects,
        }
    }
}

/// 冻结后的 immutable registry snapshot。
///
/// 只按 [`EffectKind`] 解析 handler；未注册的 kind 返回 `None`，由 runtime 转换为稳定失败，
/// 不静默回退到其他 handler。
pub struct FrozenEffectRegistry {
    effects: BTreeMap<EffectKind, EffectHandler>,
}

impl FrozenEffectRegistry {
    /// 按 effect kind 查找 handler。
    #[must_use]
    pub fn effect_handler(&self, kind: &EffectKind) -> Option<&EffectHandler> {
        self.effects.get(kind)
    }
}
