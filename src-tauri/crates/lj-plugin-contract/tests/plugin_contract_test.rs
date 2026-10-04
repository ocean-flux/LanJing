//! plugin contract 稳定层的公开行为契约。
//!
//! 覆盖 namespaced identity 的规范化、版本词汇的形状校验与 manifest 声明查询。manifest 校验、
//! 依赖解析与平台/资源限制检查属 `#36`，不在本测试范围。

use lj_plugin_contract::{
    HOST_CONTRACT_VERSION, OperationId, PluginError, PluginId, PluginManifest, Version,
};

/// 内置 plugin 使用的 plugin identity。
fn builtin_plugin_id() -> PluginId {
    PluginId::parse("lanjing.builtin").expect("内置 plugin identity 必须合法")
}

/// 内置 plugin 提供的一个 operation identity。
fn http_operation_id() -> OperationId {
    OperationId::parse("lanjing.effect.http").expect("内置 operation identity 必须合法")
}

#[test]
fn plugin_id_accepts_only_canonical_namespaced_identity() {
    assert_eq!(builtin_plugin_id().as_str(), "lanjing.builtin");
    assert_eq!(builtin_plugin_id().to_string(), "lanjing.builtin");
    assert_eq!(
        PluginId::parse("example-org.tracker_v2")
            .expect("连字符与下划线是小写身份的合法字符")
            .as_str(),
        "example-org.tracker_v2"
    );
}

#[test]
fn plugin_id_rejects_malformed_identity_with_stable_code() {
    for invalid in [
        "",
        "lanjing",
        "lanjing.",
        ".builtin",
        "lanjing..builtin",
        "LanJing.Builtin",
        "lanjing.Builtin",
        "lan jing.builtin",
    ] {
        let error = PluginId::parse(invalid).expect_err("非法 plugin identity 必须被拒绝");
        assert_eq!(error.code(), "plugin_id_invalid", "输入 {invalid:?}");
    }
}

#[test]
fn operation_id_rejects_malformed_identity_with_stable_code() {
    assert_eq!(http_operation_id().as_str(), "lanjing.effect.http");
    for invalid in ["", "http", "lanjing.effect.http.", "lanjing.effect..http"] {
        let error = OperationId::parse(invalid).expect_err("非法 operation identity 必须被拒绝");
        assert_eq!(error.code(), "operation_id_invalid", "输入 {invalid:?}");
    }
}

#[test]
fn version_rejects_empty_and_whitespace_shapes() {
    assert_eq!(
        Version::parse("0.1.0").expect("版本必须合法").as_str(),
        "0.1.0"
    );
    assert_eq!(
        Version::parse("1.2.3-beta.1+build.5")
            .expect("预发布与构建元数据是合法形状")
            .as_str(),
        "1.2.3-beta.1+build.5"
    );
    for invalid in ["", " ", " 1.0.0", "1.0.0 ", "1.0.0\n", "1.0.0 beta"] {
        let error = Version::parse(invalid).expect_err("非法版本必须被拒绝");
        assert_eq!(error.code(), "version_invalid", "输入 {invalid:?}");
    }
}

#[test]
fn manifest_reports_only_declared_operations() {
    let manifest = PluginManifest::new(
        builtin_plugin_id(),
        Version::parse("0.1.0").expect("版本必须合法"),
        Version::parse(HOST_CONTRACT_VERSION).expect("host contract version 必须合法"),
        vec![http_operation_id()],
    );
    assert_eq!(manifest.plugin_id().as_str(), "lanjing.builtin");
    assert_eq!(manifest.plugin_version().as_str(), "0.1.0");
    assert_eq!(manifest.contract_version().as_str(), HOST_CONTRACT_VERSION);
    assert_eq!(manifest.provided(), [http_operation_id()]);
    assert!(manifest.provides(&http_operation_id()));
    assert!(!manifest.provides(
        &OperationId::parse("lanjing.effect.extract").expect("内置 operation identity 必须合法")
    ));
}

#[test]
fn duplicate_identity_errors_carry_stable_codes() {
    let plugin = builtin_plugin_id();
    let operation = http_operation_id();
    assert_eq!(
        PluginError::DuplicatePlugin(plugin.clone()).code(),
        "duplicate_plugin"
    );
    assert_eq!(
        PluginError::DuplicateOperation(operation.clone()).code(),
        "duplicate_operation"
    );
    assert_eq!(
        PluginError::UndeclaredOperation { plugin, operation }.code(),
        "undeclared_operation"
    );
}
