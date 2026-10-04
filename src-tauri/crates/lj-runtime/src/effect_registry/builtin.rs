//! 内置 capability 的 effect handler 集合。
//!
//! 内置 HTTP、QuickJS rule node 与 Extract adapter 直接以 Rule Contract 的
//! [`EffectKind`] 注册到 effect registry，不再维护 identity 与 dispatch 之间的映射。

use std::sync::Arc;

use lj_rule_model::EffectKind;

use super::EffectHandler;
use crate::effect::{ExtractEffectHandler, HttpEffectHandler, QuickJsEffectHandler};

/// 内置 capability 的全部 effect handler。
///
/// 与 [`EffectKind`] 的三个变体一一对应且互不重复，由 composition root 一次注册。
#[must_use]
pub fn effects(
    http: Arc<dyn HttpEffectHandler>,
    quickjs: Arc<dyn QuickJsEffectHandler>,
    extract: Arc<dyn ExtractEffectHandler>,
) -> Vec<(EffectKind, EffectHandler)> {
    vec![
        (EffectKind::Http, EffectHandler::Http(http)),
        (EffectKind::QuickJs, EffectHandler::QuickJs(quickjs)),
        (EffectKind::Extract, EffectHandler::Extract(extract)),
    ]
}
