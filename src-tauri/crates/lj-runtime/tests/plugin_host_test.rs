//! plugin host 注册、冻结与 registry lookup 的公开行为契约。
//!
//! 覆盖 namespaced identity 的重复拒绝、manifest 声明一致性、批量注册的原子性, 以及 frozen
//! registry 只解析已注册 operation。handler 执行行为不在这里断言, 由 `plan_runtime_test` 通过
//! `RuleSystem`/runtime 的可观察执行结果覆盖。

use std::sync::Arc;

use async_trait::async_trait;
use lj_plugin_contract::{HOST_CONTRACT_VERSION, OperationId, PluginId, PluginManifest, Version};
use lj_runtime::plugin::{EffectHandler, PluginHost, builtin};
use lj_runtime::{
    CapturedEffectOutput, EffectCancellation, EffectError, EffectErrorCode, HttpEffectHandler,
    HttpEffectRequest,
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

const HTTP: &str = "lanjing.effect.http";
const QUICKJS: &str = "lanjing.effect.quickjs";

fn operation(value: &str) -> OperationId {
    OperationId::parse(value).expect("fixture operation identity 必须合法")
}

fn manifest(plugin: &str, provided: &[&str]) -> PluginManifest {
    PluginManifest::new(
        PluginId::parse(plugin).expect("fixture plugin identity 必须合法"),
        Version::parse("0.1.0").expect("fixture 版本必须合法"),
        Version::parse(HOST_CONTRACT_VERSION).expect("host contract version 必须合法"),
        provided.iter().map(|value| operation(value)).collect(),
    )
}

fn http_effect(value: &str) -> (OperationId, EffectHandler) {
    (
        operation(value),
        EffectHandler::Http(Arc::new(RegistrationOnlyHttp)),
    )
}

#[test]
fn host_rejects_duplicate_operation_identity() {
    let mut host = PluginHost::new();
    host.register_plugin(
        manifest("lanjing.builtin", &[HTTP]),
        vec![http_effect(HTTP)],
    )
    .expect("首个 plugin 必须注册成功");

    let error = host
        .register_plugin(manifest("example.other", &[HTTP]), vec![http_effect(HTTP)])
        .expect_err("同一 operation identity 不能由两个 plugin 提供");
    assert_eq!(error.code(), "duplicate_operation");
}

#[test]
fn host_rejects_duplicate_plugin_identity() {
    let mut host = PluginHost::new();
    host.register_plugin(
        manifest("lanjing.builtin", &[HTTP]),
        vec![http_effect(HTTP)],
    )
    .expect("首个 plugin 必须注册成功");

    let error = host
        .register_plugin(
            manifest("lanjing.builtin", &[QUICKJS]),
            vec![http_effect(QUICKJS)],
        )
        .expect_err("同一 plugin identity 不能注册两次");
    assert_eq!(error.code(), "duplicate_plugin");
}

#[test]
fn host_rejects_operation_missing_from_manifest() {
    let mut host = PluginHost::new();
    let error = host
        .register_plugin(
            manifest("lanjing.builtin", &[HTTP]),
            vec![http_effect(QUICKJS)],
        )
        .expect_err("未在 manifest 中声明的 operation 不能注册");
    assert_eq!(error.code(), "undeclared_operation");
}

#[test]
fn failed_registration_does_not_leave_partial_capability() {
    let mut host = PluginHost::new();
    host.register_plugin(
        manifest("lanjing.builtin", &[HTTP]),
        vec![http_effect(HTTP)],
    )
    .expect("首个 plugin 必须注册成功");

    let error = host
        .register_plugin(
            manifest("example.other", &[QUICKJS, HTTP]),
            vec![http_effect(QUICKJS), http_effect(HTTP)],
        )
        .expect_err("批量注册中出现重复 identity 必须整体失败");
    assert_eq!(error.code(), "duplicate_operation");

    let registry = host.freeze();
    assert!(
        registry.effect_handler(QUICKJS).is_none(),
        "失败的批量注册不能留下已注册的 capability"
    );
    assert!(
        registry.effect_handler(HTTP).is_some(),
        "先前成功的注册必须保留"
    );
}

#[test]
fn frozen_registry_resolves_only_registered_operations() {
    let mut host = PluginHost::new();
    host.register_plugin(
        manifest("lanjing.builtin", &[HTTP]),
        vec![http_effect(HTTP)],
    )
    .expect("内置 HTTP operation 必须注册成功");

    let registry = host.freeze();
    assert!(matches!(
        registry.effect_handler(HTTP),
        Some(EffectHandler::Http(_))
    ));
    assert!(registry.effect_handler(QUICKJS).is_none());
    assert!(
        registry
            .effect_handler("example.absent.operation")
            .is_none()
    );
}

#[test]
fn builtin_manifest_declares_the_three_effect_operations() {
    let manifest = builtin::manifest().expect("内置 manifest 必须合法");
    assert_eq!(manifest.plugin_id().as_str(), builtin::PLUGIN_ID);
    assert_eq!(manifest.contract_version().as_str(), HOST_CONTRACT_VERSION);
    for value in [
        builtin::HTTP_OPERATION,
        builtin::QUICKJS_OPERATION,
        builtin::EXTRACT_OPERATION,
    ] {
        assert!(
            manifest.provides(&operation(value)),
            "内置 manifest 必须声明 {value}"
        );
    }
    assert_eq!(manifest.provided().len(), 3);
}
