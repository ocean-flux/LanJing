//! `LanJing` Tauri 库入口。
//!
//! 负责注册插件、命令处理器，并启动应用。
//! 命令实现在 [`commands`] 模块中。

mod commands;

use tauri::Manager;

/// 构建并运行 Tauri 应用。
///
/// # Panics
///
/// 当 Tauri 应用启动失败或 `SQLite` 初始化失败时立即 panic。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            // 在应用数据目录下创建 SQLite 数据库
            let data_dir = app.path().app_data_dir().expect("获取应用数据目录失败");
            std::fs::create_dir_all(&data_dir).expect("创建应用数据目录失败");
            let db_path = data_dir.join("lanjing.db");

            let storage = commands::AppState {
                storage: std::sync::Mutex::new(
                    lj_storage::repository::SqliteStorage::new(&db_path)
                        .expect("SQLite 存储初始化失败"),
                ),
                executor: lj_runtime::executor::GraphExecutor::new(),
            };
            app.manage(storage);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::import_rule_with_preview,
            commands::confirm_import,
            commands::list_rules,
            commands::execute_segment,
            commands::get_library_projection,
            commands::merge_media_graph_delta,
            commands::update_library_entry,
        ])
        .run(tauri::generate_context!())
        .expect("error while running lanjing application");
}
