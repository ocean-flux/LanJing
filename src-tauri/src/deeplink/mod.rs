//! 原生 deep-link 装配辅助。
//!
//! URL 缓存与热启动投递由 `tauri-plugin-deep-link` 的 `getCurrent`/`onOpenUrl` 合同拥有；
//! 本模块不建立第二个 queue 或 event，只保留 desktop single-instance 的窗口聚焦行为。

/// 第二实例被拒绝后聚焦既有主窗口。
///
/// `tauri-plugin-single-instance` 的 `deep-link` feature 会先把已配置 scheme 的 argv 转交给
/// deep-link plugin；本函数不解析或记录 argv，避免泄漏 URL query。
#[cfg(desktop)]
pub(crate) fn focus_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    use tauri::Manager;

    if let Some(window) = app.get_webview_window("main")
        && let Err(error) = window.set_focus()
    {
        tracing::warn!(error = %error, "deep link 打开时无法聚焦主窗口");
    }
}
