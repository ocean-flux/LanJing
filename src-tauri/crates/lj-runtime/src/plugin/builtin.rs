//! 内置 capability 的 namespaced identity 与 manifest。
//!
//! 内置 HTTP、QuickJS rule node 与 Extract adapter 以这里的 identity 注册到 plugin host。`#40`
//! 把闭集 node/effect 配置替换为 operation identity 后，这些常量成为内置 operation descriptor
//! 的 identity，不再是 dispatch 与 identity 之间的映射。

use std::sync::Arc;

use lj_plugin_contract::{
    HOST_CONTRACT_VERSION, OperationId, PluginError, PluginId, PluginManifest, Version,
};
use lj_rule_model::EffectKind;

use super::EffectHandler;
use crate::effect::{ExtractEffectHandler, HttpEffectHandler, QuickJsEffectHandler};

/// 内置 plugin 的稳定 identity。
pub const PLUGIN_ID: &str = "lanjing.builtin";

/// 内置 HTTP effect 的 operation identity。
pub const HTTP_OPERATION: &str = "lanjing.effect.http";

/// 内置 `QuickJS` rule node 的 operation identity。
pub const QUICKJS_OPERATION: &str = "lanjing.effect.quickjs";

/// 内置 Extract effect 的 operation identity。
pub const EXTRACT_OPERATION: &str = "lanjing.effect.extract";

/// 内置 plugin 声明的 operation identity，顺序确定。
///
/// # Errors
///
/// 内置 identity 常量不是规范的 namespaced identity 时返回 [`PluginError`]。
pub fn operations() -> Result<Vec<OperationId>, PluginError> {
    [HTTP_OPERATION, QUICKJS_OPERATION, EXTRACT_OPERATION]
        .into_iter()
        .map(OperationId::parse)
        .collect()
}

/// 内置 plugin 的 manifest。
///
/// plugin version 取运行时 crate 版本：内置 capability 与 runtime 同一次构建产出，单独声明版本
/// 会产生两个会分叉的真相。
///
/// # Errors
///
/// 内置 identity 或版本常量非法时返回 [`PluginError`]。
pub fn manifest() -> Result<PluginManifest, PluginError> {
    Ok(PluginManifest::new(
        PluginId::parse(PLUGIN_ID)?,
        Version::parse(env!("CARGO_PKG_VERSION"))?,
        Version::parse(HOST_CONTRACT_VERSION)?,
        operations()?,
    ))
}

/// 内置 plugin 声明的全部 effect handler。
///
/// 与 [`manifest`] 成对：manifest 声明 operation identity，本函数把具体 handler 绑定到同一批
/// identity，由 composition root 一次注册。
///
/// # Errors
///
/// 内置 identity 常量非法时返回 [`PluginError`]。
pub fn effects(
    http: Arc<dyn HttpEffectHandler>,
    quickjs: Arc<dyn QuickJsEffectHandler>,
    extract: Arc<dyn ExtractEffectHandler>,
) -> Result<Vec<(OperationId, EffectHandler)>, PluginError> {
    Ok(vec![
        (
            OperationId::parse(HTTP_OPERATION)?,
            EffectHandler::Http(http),
        ),
        (
            OperationId::parse(QUICKJS_OPERATION)?,
            EffectHandler::QuickJs(quickjs),
        ),
        (
            OperationId::parse(EXTRACT_OPERATION)?,
            EffectHandler::Extract(extract),
        ),
    ])
}

/// Plan effect 类型对应的内置 operation identity。
#[must_use]
pub fn effect_operation(kind: &EffectKind) -> &'static str {
    match kind {
        EffectKind::Http => HTTP_OPERATION,
        EffectKind::QuickJs => QUICKJS_OPERATION,
        EffectKind::Extract => EXTRACT_OPERATION,
    }
}
