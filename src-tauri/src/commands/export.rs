//! Rule Package 导出 command：facade 产出 canonical JSON，此处只负责选路径并落盘。

use std::fmt;

use lj_rule_system::{RuleError, SourceId};
use serde::Deserialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::delivery::delivery_error;
use super::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExportRulePackageRequest {
    pub source_id: SourceId,
}

/// 导出已安装来源的 Rule Package 文件；返回落盘路径，用户取消时返回 `None`。
///
/// # Errors
///
/// 来源未安装、package 序列化失败、保存对话框不可用或写盘失败时返回 [`RuleError`]。
#[tauri::command]
pub(crate) async fn export_rule_package(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ExportRulePackageRequest,
) -> Result<Option<String>, RuleError> {
    let file_name = export_file_name(request.source_id.as_identity());
    let package_json = state.system.export_rule_package(request.source_id).await?;
    let Some(target) = app
        .dialog()
        .file()
        .set_file_name(file_name)
        .add_filter("Rule Package", &["json"])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = target.into_path().map_err(|error| export_failed(&error))?;
    std::fs::write(&path, package_json.as_bytes()).map_err(|error| export_failed(&error))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// 保存对话框的默认文件名：来源身份可能含 Windows 非法字符（如 `source:` 的冒号）。
fn export_file_name(source_identity: &str) -> String {
    let stem: String = source_identity
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    let stem = stem.trim_matches(['.', '_']);
    if stem.is_empty() {
        "rule-package.json".to_string()
    } else {
        format!("{stem}.rule-package.json")
    }
}

fn export_failed(error: &impl fmt::Display) -> RuleError {
    delivery_error(
        "RULE_PACKAGE_EXPORT_FAILED",
        &error.to_string(),
        "ipc:export",
    )
}
