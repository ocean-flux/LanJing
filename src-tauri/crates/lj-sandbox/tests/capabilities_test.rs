//! 能力管理集成测试 — `default`/`merge`/`check_capability`。
//!
//! 覆盖全局默认值、合并逻辑（交集收紧）、检查错误路径。
//! 源级 partial override 推迟，首刀只验证全 bool 交集语义。

use lj_core::sandbox::{Capability, Sandbox, SystemCapabilities};
use lj_sandbox::{check_capability, default_capabilities, merge};

/// 全局默认：network 开，其他关。
#[test]
fn default_network_enabled() {
    let caps = default_capabilities();
    assert!(caps.network);
    assert!(!caps.system.fs);
    assert!(!caps.system.env);
    assert!(!caps.system.process);
}

/// merge 取交集：全局 true + 源级 false => false（源级收紧）。
#[test]
fn merge_source_tightens_network() {
    let global = default_capabilities();
    let source = Sandbox {
        network: false,
        system: SystemCapabilities {
            fs: false,
            env: false,
            process: false,
        },
    };
    let merged = merge(&global, &source);
    assert!(!merged.network);
}

/// merge 取交集：全局 false + 源级 true => false（全局收紧）。
#[test]
fn merge_global_tightens_fs() {
    let global = default_capabilities(); // fs = false
    let source = Sandbox {
        network: true,
        system: SystemCapabilities {
            fs: true,
            env: false,
            process: false,
        },
    };
    let merged = merge(&global, &source);
    assert!(merged.network); // 两边 true
    assert!(!merged.system.fs); // global false 收紧
}

/// `check_capability`：network 允许，fs 被阻止。
#[test]
fn check_capability_ok_for_network() {
    let caps = default_capabilities();
    assert!(check_capability(&caps, Capability::Network).is_ok());
    assert!(check_capability(&caps, Capability::Fs).is_err());
    assert!(check_capability(&caps, Capability::Env).is_err());
    assert!(check_capability(&caps, Capability::Process).is_err());
}

/// `check_capability` 返回正确的错误变体。
#[test]
fn check_capability_returns_blocked_error() {
    let caps = default_capabilities();
    let err = check_capability(&caps, Capability::Fs).unwrap_err();
    assert_eq!(
        err,
        lj_core::sandbox::CapabilityError::Blocked(Capability::Fs)
    );
}
