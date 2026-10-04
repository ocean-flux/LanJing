//! effect registry 注册、冻结与 lookup 的公开行为契约。
//!
//! 覆盖重复 effect kind 的拒绝、批量注册的原子性, 以及 frozen registry 只解析已注册 kind。
//! handler 执行行为不在这里断言, 由 `plan_runtime_test` 通过 `RuleSystem`/runtime 的可观察
//! 执行结果覆盖。

use std::sync::Arc;

use async_trait::async_trait;
use lj_rule_model::EffectKind;
use lj_runtime::effect_registry::{EffectHandler, EffectRegistry, EffectRegistryError, builtin};
use lj_runtime::{
    CapturedEffectOutput, EffectCancellation, EffectError, EffectErrorCode, ExtractEffectHandler,
    ExtractEffectRequest, HttpEffectHandler, HttpEffectRequest, QuickJsEffectHandler,
    QuickJsEffectRequest,
};

/// 只用于注册契约的 HTTP handler；注册与冻结路径不会调用它。
struct RegistrationOnlyHttp;

#[async_trait]
impl HttpEffectHandler for RegistrationOnlyHttp {
    async fn execute_http(
        &self,
        _request: HttpEffectRequest,
        _cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError> {
        Err(EffectError::new(
            EffectErrorCode::Internal,
            "注册契约 fixture 不执行 effect",
        ))
    }
}

/// 只用于注册契约的 `QuickJS` handler；注册与冻结路径不会调用它。
struct RegistrationOnlyQuickJs;

#[async_trait]
impl QuickJsEffectHandler for RegistrationOnlyQuickJs {
    async fn execute_quickjs(
        &self,
        _request: QuickJsEffectRequest,
        _cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError> {
        Err(EffectError::new(
            EffectErrorCode::Internal,
            "注册契约 fixture 不执行 effect",
        ))
    }
}

/// 只用于注册契约的 Extract handler；注册与冻结路径不会调用它。
struct RegistrationOnlyExtract;

#[async_trait]
impl ExtractEffectHandler for RegistrationOnlyExtract {
    async fn execute_extract(
        &self,
        _request: ExtractEffectRequest,
        _cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError> {
        Err(EffectError::new(
            EffectErrorCode::Internal,
            "注册契约 fixture 不执行 effect",
        ))
    }
}

fn http_effect(kind: EffectKind) -> (EffectKind, EffectHandler) {
    (kind, EffectHandler::Http(Arc::new(RegistrationOnlyHttp)))
}

#[test]
fn registry_rejects_already_registered_effect_kind() {
    let mut registry = EffectRegistry::new();
    registry
        .register_all(vec![http_effect(EffectKind::Http)])
        .expect("首个 effect kind 必须注册成功");

    let error = registry
        .register_all(vec![http_effect(EffectKind::Http)])
        .expect_err("同一 effect kind 不能注册两次");
    assert_eq!(
        error,
        EffectRegistryError::DuplicateEffectKind(EffectKind::Http)
    );
    assert_eq!(error.code(), "duplicate_effect_kind");
}

#[test]
fn registry_rejects_duplicate_effect_kind_within_one_batch() {
    let mut registry = EffectRegistry::new();
    let error = registry
        .register_all(vec![
            http_effect(EffectKind::Http),
            http_effect(EffectKind::Http),
        ])
        .expect_err("同一批内的重复 effect kind 必须整体拒绝");
    assert_eq!(
        error,
        EffectRegistryError::DuplicateEffectKind(EffectKind::Http)
    );

    let registry = registry.freeze();
    assert!(
        registry.effect_handler(&EffectKind::Http).is_none(),
        "失败的批量注册不能留下已注册的 capability"
    );
}

#[test]
fn failed_registration_does_not_leave_partial_capability() {
    let mut registry = EffectRegistry::new();
    registry
        .register_all(vec![http_effect(EffectKind::Http)])
        .expect("首个 effect kind 必须注册成功");

    let error = registry
        .register_all(vec![
            http_effect(EffectKind::QuickJs),
            http_effect(EffectKind::Http),
        ])
        .expect_err("批量注册中出现重复 effect kind 必须整体失败");
    assert_eq!(
        error,
        EffectRegistryError::DuplicateEffectKind(EffectKind::Http)
    );

    let registry = registry.freeze();
    assert!(
        registry.effect_handler(&EffectKind::QuickJs).is_none(),
        "失败的批量注册不能留下已注册的 capability"
    );
    assert!(
        registry.effect_handler(&EffectKind::Http).is_some(),
        "先前成功的注册必须保留"
    );
}

#[test]
fn frozen_registry_resolves_only_registered_kinds() {
    let mut registry = EffectRegistry::new();
    registry
        .register_all(vec![http_effect(EffectKind::Http)])
        .expect("内置 HTTP effect 必须注册成功");

    let registry = registry.freeze();
    assert!(matches!(
        registry.effect_handler(&EffectKind::Http),
        Some(EffectHandler::Http(_))
    ));
    assert!(
        registry.effect_handler(&EffectKind::QuickJs).is_none(),
        "未注册的 effect kind 必须返回 None"
    );
    assert!(
        registry.effect_handler(&EffectKind::Extract).is_none(),
        "未注册的 effect kind 必须返回 None"
    );
}

#[test]
fn builtin_effects_cover_the_three_effect_kinds() {
    let effects = builtin::effects(
        Arc::new(RegistrationOnlyHttp),
        Arc::new(RegistrationOnlyQuickJs),
        Arc::new(RegistrationOnlyExtract),
    );
    let kinds: Vec<EffectKind> = effects.iter().map(|(kind, _)| kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![EffectKind::Http, EffectKind::QuickJs, EffectKind::Extract]
    );
}
