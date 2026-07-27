//! keyring-core 默认 store 安装。
//!
//! 生产路径安装平台原生 credential store；测试可在打开 storage 前用
//! `keyring_core::mock::Store` 抢占 `set_default_store`。

use std::sync::LazyLock;

use keyring_core::{Error as KeyringError, get_default_store, set_default_store};

use crate::types::StorageError;

#[derive(Clone, Copy)]
enum PlatformInstallError {
    Unavailable,
    Locked,
}

static PLATFORM_INSTALL: LazyLock<Result<(), PlatformInstallError>> = LazyLock::new(|| {
    if get_default_store().is_some() {
        Ok(())
    } else {
        install_platform_store()
    }
});

/// 确保进程内已有可用的 keyring-core 默认 store。
///
/// 若测试或其他启动路径已设置 store，则直接复用；否则安装平台 store。
///
/// # Errors
///
/// 平台 store 不可用时返回 [`StorageError::KeyringUnavailable`]；secure store 暂时锁定时返回
/// [`StorageError::KeyringLocked`]。
pub(crate) fn ensure_default_keyring_store() -> Result<(), StorageError> {
    if get_default_store().is_some() {
        return Ok(());
    }

    let result = &*PLATFORM_INSTALL;
    match result {
        Ok(()) => Ok(()),
        Err(PlatformInstallError::Unavailable) => Err(StorageError::KeyringUnavailable),
        Err(PlatformInstallError::Locked) => Err(StorageError::KeyringLocked),
    }
}

fn install_platform_store() -> Result<(), PlatformInstallError> {
    #[cfg(target_os = "windows")]
    {
        let store = windows_native_keyring_store::Store::new()
            .map_err(|error| map_install_error(&error))?;
        set_default_store(store);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let store = apple_native_keyring_store::keychain::Store::new()
            .map_err(|error| map_install_error(&error))?;
        set_default_store(store);
        Ok(())
    }

    #[cfg(target_os = "ios")]
    {
        // iOS 使用 Protected Data 且默认不启用 iCloud 同步；锁屏访问由 store 返回
        // NoStorageAccess，并由上层稳定映射为 KeyringLocked。
        let store = apple_native_keyring_store::protected::Store::new()
            .map_err(|error| map_install_error(&error))?;
        set_default_store(store);
        Ok(())
    }

    #[cfg(target_os = "android")]
    {
        // Tauri/Android application 必须先初始化 ndk-context；Store::new 会使用 Android
        // SharedPreferences，并由 Android Keystore 加密其数据 key。
        let store = android_native_keyring_store::Store::new()
            .map_err(|error| map_install_error(&error))?;
        set_default_store(store);
        Ok(())
    }

    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ))]
    {
        let store = zbus_secret_service_keyring_store::Store::new()
            .map_err(|error| map_install_error(&error))?;
        set_default_store(store);
        Ok(())
    }

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android",
        all(
            unix,
            not(any(target_os = "macos", target_os = "ios", target_os = "android"))
        )
    )))]
    {
        Err(PlatformInstallError::Unavailable)
    }
}

fn map_install_error(error: &KeyringError) -> PlatformInstallError {
    match error {
        KeyringError::NoStorageAccess(_) => PlatformInstallError::Locked,
        _ => PlatformInstallError::Unavailable,
    }
}
