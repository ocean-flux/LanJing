//! keyring-core 默认 store 安装。
//!
//! 生产路径安装平台原生 credential store；测试可在打开 storage 前用
//! `keyring_core::mock::Store` 抢占 `set_default_store`。

use std::sync::OnceLock;

use keyring_core::{get_default_store, set_default_store};

use crate::types::StorageError;

static PLATFORM_INSTALL: OnceLock<Result<(), String>> = OnceLock::new();

/// 确保进程内已有可用的 keyring-core 默认 store。
///
/// 若测试或其他启动路径已设置 store，则直接复用；否则安装平台 store。
///
/// # Errors
///
/// 平台 store 构造失败时返回 [`StorageError::Keyring`]。
pub(crate) fn ensure_default_keyring_store() -> Result<(), StorageError> {
    if get_default_store().is_some() {
        return Ok(());
    }

    let result = PLATFORM_INSTALL.get_or_init(|| {
        if get_default_store().is_some() {
            return Ok(());
        }
        install_platform_store()
    });

    match result {
        Ok(()) => Ok(()),
        Err(_) => Err(StorageError::Keyring),
    }
}

fn install_platform_store() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let store = windows_native_keyring_store::Store::new()
            .map_err(|error| format!("Windows keyring store 初始化失败: {error}"))?;
        set_default_store(store);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let store = apple_native_keyring_store::keychain::Store::new()
            .map_err(|error| format!("macOS keyring store 初始化失败: {error}"))?;
        set_default_store(store);
        Ok(())
    }

    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ))]
    {
        let store = zbus_secret_service_keyring_store::Store::new()
            .map_err(|error| format!("Linux Secret Service keyring store 初始化失败: {error}"))?;
        set_default_store(store);
        Ok(())
    }

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        all(
            unix,
            not(any(target_os = "macos", target_os = "ios", target_os = "android"))
        )
    )))]
    {
        Err("当前平台不支持原生 keyring store".to_string())
    }
}
