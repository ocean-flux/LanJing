//! native rule document 作者生命周期 command。
//!
//! 全部为 `state.system.xxx(request).await` 薄封装；wire 采用既有 `request` wrapper
//! 模式（payload 为 `lj-rule-system` DTO，字段 `snake_case`）。

use lj_rule_system::{
    CreateNativeRuleDocumentRequest, DeleteNativeRuleDocumentRequest, GetNativeRuleDocumentRequest,
    GetNativeRuleProvenanceRequest, NativeRuleDocumentDetail, NativeRuleDocumentSummary,
    NativeRuleProvenanceView, NativeRuleRevisionSummary, RenameNativeRuleDocumentRequest,
    RestoreNativeRuleRevisionOutcome, RestoreNativeRuleRevisionRequest, RuleError,
    SaveNativeRuleDocumentOutcome, SaveNativeRuleDocumentRequest,
    ValidateNativeRuleDocumentPreview, ValidateNativeRuleDocumentRequest,
};
use tauri::State;

use super::state::AppState;

/// 创建 blank/template native rule document。
#[tauri::command]
pub(crate) async fn create_native_rule_document(
    state: State<'_, AppState>,
    request: CreateNativeRuleDocumentRequest,
) -> Result<NativeRuleDocumentSummary, RuleError> {
    state.system.create_native_rule_document(request).await
}

/// 分域保存 native rule document（语义/布局各自携带 `expected_revision`）。
#[tauri::command]
pub(crate) async fn save_native_rule_document(
    state: State<'_, AppState>,
    request: SaveNativeRuleDocumentRequest,
) -> Result<SaveNativeRuleDocumentOutcome, RuleError> {
    state.system.save_native_rule_document(request).await
}

/// 校验已保存语义快照并返回安全摘要。
#[tauri::command]
pub(crate) async fn validate_native_rule_document(
    state: State<'_, AppState>,
    request: ValidateNativeRuleDocumentRequest,
) -> Result<ValidateNativeRuleDocumentPreview, RuleError> {
    state.system.validate_native_rule_document(request).await
}

/// 列出全部 native rule documents。
#[tauri::command]
pub(crate) async fn list_native_rule_documents(
    state: State<'_, AppState>,
) -> Result<Vec<NativeRuleDocumentSummary>, RuleError> {
    state.system.list_native_rule_documents().await
}

/// 列出 native rule document 的 Effective 历史安全摘要。
#[tauri::command]
pub(crate) async fn list_native_rule_revision_history(
    state: State<'_, AppState>,
    request: GetNativeRuleDocumentRequest,
) -> Result<Vec<NativeRuleRevisionSummary>, RuleError> {
    state
        .system
        .list_native_rule_revision_history(request)
        .await
}

/// 从 Effective 历史创建新的 Draft Rule Revision。
#[tauri::command]
pub(crate) async fn restore_native_rule_revision(
    state: State<'_, AppState>,
    request: RestoreNativeRuleRevisionRequest,
) -> Result<RestoreNativeRuleRevisionOutcome, RuleError> {
    state.system.restore_native_rule_revision(request).await
}

/// 获取单个 native rule document 详情。
#[tauri::command]
pub(crate) async fn get_native_rule_document(
    state: State<'_, AppState>,
    request: GetNativeRuleDocumentRequest,
) -> Result<Option<NativeRuleDocumentDetail>, RuleError> {
    state.system.get_native_rule_document(request).await
}

/// 重命名 native rule document 的展示标题。
#[tauri::command]
pub(crate) async fn rename_native_rule_document(
    state: State<'_, AppState>,
    request: RenameNativeRuleDocumentRequest,
) -> Result<NativeRuleDocumentSummary, RuleError> {
    state.system.rename_native_rule_document(request).await
}

/// 删除 native rule document（linked 状态需显式确认）。
#[tauri::command]
pub(crate) async fn delete_native_rule_document(
    state: State<'_, AppState>,
    request: DeleteNativeRuleDocumentRequest,
) -> Result<(), RuleError> {
    state.system.delete_native_rule_document(request).await
}

/// 获取 native rule document 的 provenance 只读视图（原文已脱敏）。
#[tauri::command]
pub(crate) async fn get_native_rule_provenance(
    state: State<'_, AppState>,
    request: GetNativeRuleProvenanceRequest,
) -> Result<Option<NativeRuleProvenanceView>, RuleError> {
    state.system.get_native_rule_provenance(request).await
}

#[cfg(test)]
mod tests {
    use lj_rule_system::{CreateMode, CredentialMutationAction};

    use super::*;

    /// 简单字段请求的 wire roundtrip（`snake_case` 字段名）。
    #[test]
    fn document_request_wire_roundtrips() {
        let create: CreateNativeRuleDocumentRequest = serde_json::from_str(
            r#"{"mode": {"kind": "template", "title": "T", "intent": "Search", "data_type": "json", "base_url": "https://e.test"}}"#,
        )
        .expect("create 请求解析");
        assert!(matches!(create.mode, CreateMode::Template { .. }));
        let json = serde_json::to_value(&create).expect("序列化");
        assert_eq!(json["mode"]["kind"], "template");
        assert_eq!(json["mode"]["data_type"], "json");
        assert_eq!(json["mode"]["intent"], "Search");

        let blank: CreateNativeRuleDocumentRequest =
            serde_json::from_str(r#"{"mode": {"kind": "blank"}}"#).expect("blank 解析");
        assert!(matches!(blank.mode, CreateMode::Blank));

        let get: GetNativeRuleDocumentRequest =
            serde_json::from_str(r#"{"document_id": "d1"}"#).expect("get 请求解析");
        assert_eq!(get.document_id, "d1");
        let validate: ValidateNativeRuleDocumentRequest =
            serde_json::from_str(r#"{"document_id": "d1", "revision": 3}"#)
                .expect("validate 请求解析");
        assert_eq!(validate.revision, 3);
        let rename: RenameNativeRuleDocumentRequest = serde_json::from_str(
            r#"{"document_id": "d1", "title": "新标题", "expected_revision": 3}"#,
        )
        .expect("rename 请求解析");
        assert_eq!(rename.title, "新标题");
        let delete: DeleteNativeRuleDocumentRequest =
            serde_json::from_str(r#"{"document_id": "d1", "confirm_linked": true}"#)
                .expect("delete 请求解析");
        assert!(delete.confirm_linked);
        let provenance: GetNativeRuleProvenanceRequest =
            serde_json::from_str(r#"{"document_id": "d1"}"#).expect("provenance 请求解析");
        assert_eq!(provenance.document_id, "d1");
    }

    /// save 请求（含 Definition current shape 与凭证变更）的完整 wire roundtrip。
    #[test]
    fn save_request_wire_roundtrips() {
        let request: SaveNativeRuleDocumentRequest = serde_json::from_str(
            r#"{
                "document_id": "d1",
                "semantic": {
                    "expected_revision": 2,
                    "definition": {
                        "contract": "rule_definition",
                        "schema_version": 1,
                        "source_identity": {"id": "native:x"},
                        "base_url": "https://e.test",
                        "intent_exports": {},
                        "flow": {"nodes": [], "edges": []},
                        "capability_manifest": {"required": {"network": false, "system": {"fs": false, "env": false, "process": false}}},
                        "source_id_rules": []
                    },
                    "credential_mutations": [
                        {
                            "node_id": "00000000-0000-0000-0000-000000000001",
                            "json_pointer": "/headers/Authorization",
                            "logical_name": "Authorization",
                            "action": "replace",
                            "value": "one-time"
                        }
                    ]
                },
                "layout": {"expected_revision": 1, "layout_json": "{}"}
            }"#,
        )
        .expect("save 请求解析");
        let semantic = request.semantic.as_ref().expect("语义域");
        assert_eq!(semantic.expected_revision, 2);
        assert_eq!(semantic.definition.base_url(), "https://e.test");
        assert_eq!(
            semantic.credential_mutations[0].action,
            CredentialMutationAction::Replace
        );
        assert_eq!(
            semantic.credential_mutations[0].value.as_deref(),
            Some("one-time")
        );
        assert_eq!(
            request.layout.as_ref().expect("布局域").expected_revision,
            1
        );

        let json = serde_json::to_value(&request).expect("序列化");
        assert_eq!(json["document_id"], "d1");
        assert_eq!(json["layout"]["layout_json"], "{}");
        assert!(json["semantic"]["definition"]["contract"].is_string());
    }
}
