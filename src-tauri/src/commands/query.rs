//! 已安装来源、资料库与标准媒体投影查询 command。

use lj_rule_system::{
    InstalledSource, LibraryEntryUpdate, LibraryProjection, LibraryUpdateReceipt, MediaAssetPage,
    MediaItem, MediaUnitPage, RuleError, SourceId, SourceRevisionSummary,
};
use serde::Deserialize;
use tauri::State;

use super::state::AppState;

#[tauri::command]
pub(crate) async fn list_installed_sources(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledSource>, RuleError> {
    state.system.list_installed_sources().await
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListSourceRevisionsRequest {
    pub source_id: SourceId,
}

#[tauri::command]
pub(crate) async fn list_source_revisions(
    state: State<'_, AppState>,
    request: ListSourceRevisionsRequest,
) -> Result<Vec<SourceRevisionSummary>, RuleError> {
    state.system.list_source_revisions(request.source_id).await
}

#[tauri::command]
pub(crate) async fn get_library_projection(
    state: State<'_, AppState>,
) -> Result<LibraryProjection, RuleError> {
    state.system.get_library_projection().await
}

#[tauri::command]
pub(crate) async fn update_library_entry(
    state: State<'_, AppState>,
    request: LibraryEntryUpdate,
) -> Result<LibraryUpdateReceipt, RuleError> {
    state.system.update_library_entry(request).await
}

#[derive(Debug, Deserialize)]
pub(crate) struct GetMediaItemRequest {
    pub resource_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GetMediaItemsRequest {
    pub resource_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListMediaUnitsRequest {
    pub item_id: String,
    #[serde(default)]
    pub offset: u32,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListMediaAssetsRequest {
    pub unit_id: String,
    #[serde(default)]
    pub offset: u32,
    pub limit: Option<u32>,
}

#[tauri::command]
pub(crate) async fn get_media_item(
    state: State<'_, AppState>,
    request: GetMediaItemRequest,
) -> Result<Option<MediaItem>, RuleError> {
    state.system.get_media_item(request.resource_id).await
}

#[tauri::command]
pub(crate) async fn get_media_items(
    state: State<'_, AppState>,
    request: GetMediaItemsRequest,
) -> Result<Vec<MediaItem>, RuleError> {
    state.system.get_media_items(request.resource_ids).await
}

#[tauri::command]
pub(crate) async fn list_media_units(
    state: State<'_, AppState>,
    request: ListMediaUnitsRequest,
) -> Result<MediaUnitPage, RuleError> {
    state
        .system
        .list_media_units(request.item_id, request.offset, request.limit)
        .await
}

#[tauri::command]
pub(crate) async fn list_media_assets(
    state: State<'_, AppState>,
    request: ListMediaAssetsRequest,
) -> Result<MediaAssetPage, RuleError> {
    state
        .system
        .list_media_assets(request.unit_id, request.offset, request.limit)
        .await
}
