//! `LanJing` Tauri 库入口。
//!
//! 此处只初始化 `RuleSystem`、注册 IPC delivery 命令与桌面插件；规则生命周期的内部组合
//! 不会穿透到 Tauri 根。

mod commands;
mod deeplink;

use std::sync::Arc;

use lj_rule_system::{RuleSystem, RuleSystemConfig};
use tauri::Manager;

macro_rules! lanjing_commands {
    ($consumer:ident) => {
        $consumer! {
            fetch_import_src => commands::import_install::fetch_import_src,
            prepare_install => commands::import_install::prepare_install,
            install => commands::import_install::install,
            execute => commands::execution::execute,
            cancel_execution => commands::execution::cancel_execution,
            catch_up_execution => commands::execution::catch_up_execution,
            list_installed_sources => commands::query::list_installed_sources,
            get_library_projection => commands::query::get_library_projection,
            update_library_entry => commands::query::update_library_entry,
            get_media_item => commands::query::get_media_item,
            get_media_items => commands::query::get_media_items,
            list_media_units => commands::query::list_media_units,
            list_media_assets => commands::query::list_media_assets,
            create_native_rule_document => commands::document::create_native_rule_document,
            save_native_rule_document => commands::document::save_native_rule_document,
            validate_native_rule_document => commands::document::validate_native_rule_document,
            prepare_native_rule_document => commands::document::prepare_native_rule_document,
            list_native_rule_documents => commands::document::list_native_rule_documents,
            get_native_rule_document => commands::document::get_native_rule_document,
            rename_native_rule_document => commands::document::rename_native_rule_document,
            delete_native_rule_document => commands::document::delete_native_rule_document,
            get_native_rule_provenance => commands::document::get_native_rule_provenance,
        }
    };
}

macro_rules! generate_lanjing_handler {
    ($($name:ident => $handler:path),+ $(,)?) => {
        tauri::generate_handler![$($handler),+]
    };
}

#[cfg(test)]
macro_rules! declare_registered_command_names {
    ($($name:ident => $handler:path),+ $(,)?) => {
        const REGISTERED_COMMAND_NAMES: &[&str] = &[$(stringify!($name)),+];
    };
}

#[cfg(test)]
lanjing_commands!(declare_registered_command_names);

/// 构建并运行 Tauri 应用。
///
/// # Panics
///
/// 无法定位或创建应用数据目录，或无法初始化唯一的 `RuleSystem` durable store 时立即 panic。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(all(debug_assertions, desktop))]
    let builder = builder.plugin(tauri_plugin_mcp_bridge::init_with_config(
        tauri_plugin_mcp_bridge::Config::localhost_only(),
    ));

    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        // `deep-link` feature 已先把静态 scheme argv 转成 plugin 事件；这里只聚焦既有窗口。
        deeplink::focus_main_window(app);
    }));

    builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        // 主题/偏好：@tauri-store/svelte 后端（替换官方 plugin-store）。
        .plugin(tauri_plugin_svelte::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            #[cfg(any(windows, target_os = "linux"))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;

                app.deep_link().register_all()?;
            }

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;

            let system = tauri::async_runtime::block_on(RuleSystem::open(
                RuleSystemConfig::desktop(
                    data_dir.join("lanjing-event-store.db"),
                    data_dir.join("artifacts"),
                ),
            ))?;
            app.manage(commands::state::AppState::new(Arc::new(system)));

            Ok(())
        })
        .invoke_handler(lanjing_commands!(generate_lanjing_handler))
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| eprintln!("lanjing application terminated: {error}"));
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::REGISTERED_COMMAND_NAMES;

    #[test]
    fn root_command_registry_contains_only_current_facade_commands() {
        const EXPECTED: &[&str] = &[
            "fetch_import_src",
            "prepare_install",
            "install",
            "execute",
            "cancel_execution",
            "catch_up_execution",
            "list_installed_sources",
            "get_library_projection",
            "update_library_entry",
            "get_media_item",
            "get_media_items",
            "list_media_units",
            "list_media_assets",
            "create_native_rule_document",
            "save_native_rule_document",
            "validate_native_rule_document",
            "prepare_native_rule_document",
            "list_native_rule_documents",
            "get_native_rule_document",
            "rename_native_rule_document",
            "delete_native_rule_document",
            "get_native_rule_provenance",
        ];
        assert_eq!(REGISTERED_COMMAND_NAMES, EXPECTED);
        assert_eq!(
            REGISTERED_COMMAND_NAMES
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len(),
            EXPECTED.len(),
            "Tauri command 注册不得重复"
        );
        assert_eq!(EXPECTED.len(), 22, "facade 命令注册必须恰为 22 项");
        assert!(
            REGISTERED_COMMAND_NAMES
                .iter()
                .all(|name| !name.contains("source_document")),
            "不得恢复旧 authoring 链的 source_document 命令名"
        );
    }
}
