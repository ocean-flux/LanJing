//! plugin host：内置与后续静态链接 plugin 的唯一注册与冻结边界。
//!
//! 本模块只负责 namespaced identity 的注册、重复拒绝与 frozen registry snapshot。manifest 校验、
//! 依赖图与 plugin lock 属 `#36`，registration lease、activation 回滚与 drain 属 `#37`/`#38`，
//! plugin-facing handler contract 的定形与搬迁属 `#40`/`#41`。

pub mod builtin;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lj_plugin_contract::{OperationId, PluginError, PluginId, PluginManifest};

use crate::effect::{ExtractEffectHandler, HttpEffectHandler, QuickJsEffectHandler};

/// 一个注册到 plugin host 的 effect handler。
#[derive(Clone)]
pub enum EffectHandler {
    /// HTTP effect handler。
    Http(Arc<dyn HttpEffectHandler>),
    /// `QuickJS` rule node handler。
    QuickJs(Arc<dyn QuickJsEffectHandler>),
    /// Extract effect handler。
    Extract(Arc<dyn ExtractEffectHandler>),
}

/// plugin 注册边界。
///
/// 注册与冻结只发生在 application composition 阶段；冻结产物 [`FrozenRegistry`] 之后不再变化，
/// 因此一次 execution 绑定的 handler 集合不会在运行中被替换。
#[derive(Default)]
pub struct PluginHost {
    plugins: BTreeMap<PluginId, PluginManifest>,
    operations: BTreeMap<OperationId, EffectHandler>,
}

impl PluginHost {
    /// 创建空的注册边界。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个 plugin 声明的全部 effect handler。
    ///
    /// 批量注册是原子的：manifest 未声明的 operation、重复的 plugin identity、重复的 operation
    /// identity 都会在写入任何 handler 之前失败，因此失败的注册不会留下半注册的 capability。
    ///
    /// # Errors
    ///
    /// - [`PluginError::DuplicatePlugin`]：plugin identity 已注册。
    /// - [`PluginError::DuplicateOperation`]：operation identity 已由其他 plugin 提供，或同一批内重复。
    /// - [`PluginError::UndeclaredOperation`]：operation 不在该 manifest 的 `provided` 列表里。
    pub fn register_plugin(
        &mut self,
        manifest: PluginManifest,
        effects: Vec<(OperationId, EffectHandler)>,
    ) -> Result<(), PluginError> {
        if self.plugins.contains_key(manifest.plugin_id()) {
            return Err(PluginError::DuplicatePlugin(manifest.plugin_id().clone()));
        }
        let mut batch = BTreeSet::new();
        for (operation, _) in &effects {
            if !manifest.provides(operation) {
                return Err(PluginError::UndeclaredOperation {
                    plugin: manifest.plugin_id().clone(),
                    operation: operation.clone(),
                });
            }
            if self.operations.contains_key(operation) || !batch.insert(operation) {
                return Err(PluginError::DuplicateOperation(operation.clone()));
            }
        }
        for (operation, handler) in effects {
            self.operations.insert(operation, handler);
        }
        self.plugins.insert(manifest.plugin_id().clone(), manifest);
        Ok(())
    }

    /// 冻结为 immutable registry snapshot。
    #[must_use]
    pub fn freeze(self) -> FrozenRegistry {
        FrozenRegistry {
            operations: self.operations,
        }
    }
}

/// 冻结后的 immutable registry snapshot。
///
/// 只按 operation identity 解析 handler；未注册的 identity 返回 `None`，由 runtime 转换为稳定
/// 失败，不静默回退到其他 handler。
pub struct FrozenRegistry {
    operations: BTreeMap<OperationId, EffectHandler>,
}

impl FrozenRegistry {
    /// 按 operation identity 查找 handler。
    #[must_use]
    pub fn effect_handler(&self, operation: &str) -> Option<&EffectHandler> {
        self.operations.get(operation)
    }
}
