//! 来源导入、candidate 准备与安装 command。

use lj_rule_system::{
    CandidateId, InstallCandidate, InstalledSource, RuleError, RuleInput, RuleSystem, SourceId,
};
use serde::Deserialize;
use tauri::State;

use super::state::AppState;

#[derive(Debug, Deserialize)]
pub(crate) struct FetchImportSrcRequest {
    pub url: String,
}

/// 安装请求。
///
/// 应用不再携带 grant：网络不受 capability 控制，系统能力（fs/env/process）永不授予。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InstallRequest {
    pub candidate_id: CandidateId,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceRollbackRequest {
    pub source_id: SourceId,
    pub revision: u64,
}

#[tauri::command]
pub(crate) async fn fetch_import_src(request: FetchImportSrcRequest) -> Result<String, RuleError> {
    RuleSystem::fetch_import_source(&request.url).await
}

#[tauri::command]
pub(crate) async fn prepare_install(
    state: State<'_, AppState>,
    request: RuleInput,
) -> Result<InstallCandidate, RuleError> {
    state.system.prepare_install(request).await
}

#[tauri::command]
pub(crate) async fn prepare_source_rollback(
    state: State<'_, AppState>,
    request: SourceRollbackRequest,
) -> Result<InstallCandidate, RuleError> {
    state
        .system
        .prepare_source_rollback(request.source_id, request.revision)
        .await
}

#[tauri::command]
pub(crate) async fn install(
    state: State<'_, AppState>,
    request: InstallRequest,
) -> Result<InstalledSource, RuleError> {
    state.system.install(request.candidate_id).await
}
