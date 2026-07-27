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
            fetch_import_src => commands::fetch_import_src,
            list_source_documents => commands::list_source_documents,
            create_source_document => commands::create_source_document,
            get_source_document => commands::get_source_document,
            save_source_document => commands::save_source_document,
            pin_source_document_revision => commands::pin_source_document_revision,
            release_source_document_revision_pin => commands::release_source_document_revision_pin,
            rebase_source_document => commands::rebase_source_document,
            rename_source_document => commands::rename_source_document,
            delete_source_document => commands::delete_source_document,
            reveal_source_document_credential => commands::reveal_source_document_credential,
            replace_source_document_credential => commands::replace_source_document_credential,
            clear_source_document_credential => commands::clear_source_document_credential,
            prepare_install_from_document => commands::prepare_install_from_document,
            prepare_install => commands::prepare_install,
            install => commands::install,
            execute => commands::execute,
            cancel_execution => commands::cancel_execution,
            catch_up_execution => commands::catch_up_execution,
            list_installed_sources => commands::list_installed_sources,
            get_library_projection => commands::get_library_projection,
            update_library_entry => commands::update_library_entry,
            get_media_item => commands::get_media_item,
            get_media_items => commands::get_media_items,
            list_media_units => commands::list_media_units,
            list_media_assets => commands::list_media_assets,
        }
    };
}

macro_rules! declare_registered_command_names {
    ($($name:ident => $handler:path),+ $(,)?) => {
        #[cfg(test)]
        const REGISTERED_COMMAND_NAMES: &[&str] = &[$(stringify!($name)),+];
    };
}

macro_rules! generate_lanjing_handler {
    ($($name:ident => $handler:path),+ $(,)?) => {
        tauri::generate_handler![$($handler),+]
    };
}

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
            app.manage(commands::AppState::new(Arc::new(system)));

            Ok(())
        })
        .invoke_handler(lanjing_commands!(generate_lanjing_handler))
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| eprintln!("lanjing application terminated: {error}"));
}
