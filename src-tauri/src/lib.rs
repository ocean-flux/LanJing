//! `LanJing` Tauri 库入口。
//!
//! 此处只初始化 `RuleSystem`、注册 IPC delivery 命令与桌面插件；规则生命周期的内部组合
//! 不会穿透到 Tauri 根。

mod commands;

use std::sync::Arc;

use lj_rule_system::{RuleSystem, RuleSystemConfig};
use tauri::Manager;

/// 构建并运行 Tauri 应用。
///
/// # Panics
///
/// 无法定位或创建应用数据目录，或无法初始化唯一的 `RuleSystem` durable store 时立即 panic。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        // 主题/偏好：@tauri-store/svelte 后端（替换官方 plugin-store）。
        .plugin(tauri_plugin_svelte::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("获取应用数据目录失败");
            std::fs::create_dir_all(&data_dir).expect("创建应用数据目录失败");

            let system = tauri::async_runtime::block_on(RuleSystem::open(
                RuleSystemConfig::desktop(
                    data_dir.join("lanjing-event-store.db"),
                    data_dir.join("artifacts"),
                ),
            ))
            .expect("RuleSystem 初始化失败");
            app.manage(commands::AppState::new(Arc::new(system)));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::prepare_install,
            commands::install,
            commands::execute,
            commands::cancel_execution,
            commands::catch_up_execution,
            commands::list_installed_sources,
            commands::get_library_projection,
            commands::update_library_entry,
        ])
        .run(tauri::generate_context!())
        .expect("error while running lanjing application");
}
