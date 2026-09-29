//! 原生规则文档行读写与查询。
//!
//! 写函数只在 writer transaction 内被 `transaction/document.rs` 调用；读函数同时服务
//! writer transaction（save 校验）与只读 lane（list/get/provenance text）。

use std::str::FromStr;

use sea_orm::FromQueryResult;

use crate::artifact::ArtifactStore;
use crate::database::{DatabaseSession, OptionalResultExt, statement};
use crate::repository::event::database_error;
use crate::repository::secret::read_owned_secret;
use crate::types::{
    CreateDocumentRequest, DocumentDetail, DocumentSummary, LayoutSnapshot, ProvenanceCreateInput,
    ProvenanceSummary, RuleRevisionHistoryRecord, SecretArtifactId, SemanticSnapshot, StorageError,
};

/// provenance 原文 secret 的 owner kind。
pub(crate) const DOCUMENT_OWNER_KIND: &str = "native_rule_document";
/// 文档凭证槽位 secret 的 owner kind。
pub(crate) const CREDENTIAL_OWNER_KIND: &str = "native_rule_document_credential";
/// current 唯一支持的文档 format。
pub(crate) const NATIVE_RULE_FORMAT: &str = "native_rule";

#[derive(FromQueryResult)]
pub(crate) struct DocumentRow {
    pub(crate) document_id: String,
    pub(crate) format: String,
    pub(crate) title: String,
    pub(crate) source_identity: String,
    pub(crate) state: String,
    pub(crate) semantic_revision: i64,
    pub(crate) layout_revision: i64,
    pub(crate) link_revision: i64,
    pub(crate) created_at_ms: i64,
    pub(crate) updated_at_ms: i64,
}

#[derive(FromQueryResult)]
pub(crate) struct SemanticRow {
    pub(crate) revision: i64,
    pub(crate) definition_hash: String,
    pub(crate) definition_json: String,
    pub(crate) manifest_json: String,
    pub(crate) updated_at_ms: i64,
}

#[derive(FromQueryResult)]
pub(crate) struct EffectiveHistoryRow {
    pub(crate) revision: i64,
    pub(crate) definition_hash: String,
    pub(crate) definition_json: String,
    pub(crate) manifest_json: String,
}

#[derive(FromQueryResult)]
struct EffectiveHistorySummaryRow {
    revision: i64,
    definition_hash: String,
    effective_at_ms: i64,
}

#[derive(FromQueryResult)]
pub(crate) struct LayoutRow {
    pub(crate) revision: i64,
    pub(crate) layout_json: String,
}

#[derive(FromQueryResult)]
pub(crate) struct ProvenanceRow {
    pub(crate) format: String,
    pub(crate) adapter_version: String,
    pub(crate) input_hash: String,
    pub(crate) source_text_secret_id: Option<String>,
    pub(crate) diagnostics_json: String,
    pub(crate) imported_at_ms: i64,
}

#[derive(FromQueryResult)]
struct OwnerIdRow {
    owner_id: String,
}

#[derive(FromQueryResult)]
struct DocumentIdRow {
    document_id: String,
}

const DOCUMENT_COLUMNS: &str = "document_id, format, title, source_identity, state, semantic_revision, layout_revision, link_revision, created_at_ms, updated_at_ms";

/// 按 `document_id` 读取文档主行。
pub(crate) async fn document_row(
    conn: &mut DatabaseSession,
    document_id: &str,
) -> Result<Option<DocumentRow>, StorageError> {
    statement(format!(
        "SELECT {DOCUMENT_COLUMNS} FROM rule_documents WHERE document_id = ?"
    ))
    .bind(document_id)
    .get_result::<DocumentRow>(conn)
    .await
    .optional()
    .map_err(database_error)
}

/// 按创建时刻升序列出全部文档摘要。
pub(crate) async fn list_document_summaries(
    conn: &mut DatabaseSession,
) -> Result<Vec<DocumentSummary>, StorageError> {
    let rows = statement(format!(
        "SELECT {DOCUMENT_COLUMNS} FROM rule_documents ORDER BY created_at_ms ASC"
    ))
    .load::<DocumentRow>(conn)
    .await
    .map_err(database_error)?;
    Ok(rows.into_iter().map(summary_from_row).collect())
}

/// 读取文档详情：主行 + 各域当前快照，同一只读 transaction 内一致。
pub(crate) async fn document_detail(
    conn: &mut DatabaseSession,
    document_id: &str,
) -> Result<Option<DocumentDetail>, StorageError> {
    let Some(row) = document_row(conn, document_id).await? else {
        return Ok(None);
    };
    let semantic = semantic_row(conn, document_id)
        .await?
        .map(|value| SemanticSnapshot {
            revision: value.revision,
            definition_json: value.definition_json,
            definition_hash: value.definition_hash,
            manifest_json: value.manifest_json,
        });
    let effective_row = effective_semantic_row(conn, document_id).await?;
    let effective_summary = effective_row
        .as_ref()
        .map(|value| RuleRevisionHistoryRecord {
            revision: value.revision,
            definition_hash: value.definition_hash.clone(),
            effective_at_ms: value.updated_at_ms,
        });
    let effective_semantic = effective_row.map(|value| SemanticSnapshot {
        revision: value.revision,
        definition_json: value.definition_json,
        definition_hash: value.definition_hash,
        manifest_json: value.manifest_json,
    });
    let layout = layout_row(conn, document_id)
        .await?
        .map(|value| LayoutSnapshot {
            revision: value.revision,
            layout_json: value.layout_json,
        });
    let provenance = provenance_row(conn, document_id)
        .await?
        .map(|value| ProvenanceSummary {
            format: value.format,
            adapter_version: value.adapter_version,
            input_hash: value.input_hash,
            diagnostics_json: value.diagnostics_json,
            imported_at_ms: value.imported_at_ms,
        });
    Ok(Some(DocumentDetail {
        summary: summary_from_row(row),
        semantic,
        effective_semantic,
        effective_summary,
        layout,
        provenance,
    }))
}

pub(crate) async fn semantic_row(
    conn: &mut DatabaseSession,
    document_id: &str,
) -> Result<Option<SemanticRow>, StorageError> {
    statement(
        "SELECT revision, definition_hash, definition_json, manifest_json, updated_at_ms FROM rule_document_semantics WHERE document_id = ?",
    )
    .bind(document_id)
    .get_result::<SemanticRow>(conn)
    .await
    .optional()
    .map_err(database_error)
}

/// 读取最近一次有效 semantic 快照。
pub(crate) async fn effective_semantic_row(
    conn: &mut DatabaseSession,
    document_id: &str,
) -> Result<Option<SemanticRow>, StorageError> {
    statement(
        "SELECT revision, definition_hash, definition_json, manifest_json, updated_at_ms FROM rule_document_effective_semantics WHERE document_id = ?",
    )
    .bind(document_id)
    .get_result::<SemanticRow>(conn)
    .await
    .optional()
    .map_err(database_error)
}

/// 按最新 effective 时间读取规则历史安全摘要。
pub(crate) async fn list_effective_history(
    conn: &mut DatabaseSession,
    document_id: &str,
) -> Result<Vec<RuleRevisionHistoryRecord>, StorageError> {
    let rows = statement(
        "SELECT revision, definition_hash, updated_at_ms AS effective_at_ms FROM rule_document_effective_semantic_history WHERE document_id = ? ORDER BY effective_at_ms DESC, revision DESC",
    )
    .bind(document_id)
    .load::<EffectiveHistorySummaryRow>(conn)
    .await
    .map_err(database_error)?;
    Ok(rows
        .into_iter()
        .map(|row| RuleRevisionHistoryRecord {
            revision: row.revision,
            definition_hash: row.definition_hash,
            effective_at_ms: row.effective_at_ms,
        })
        .collect())
}

/// 读取指定 effective 历史的完整内部快照；完整 Definition 不直接暴露给读 API。
pub(crate) async fn effective_history_row(
    conn: &mut DatabaseSession,
    document_id: &str,
    revision: i64,
) -> Result<Option<EffectiveHistoryRow>, StorageError> {
    statement(
        "SELECT revision, definition_hash, definition_json, manifest_json FROM rule_document_effective_semantic_history WHERE document_id = ? AND revision = ?",
    )
    .bind(document_id)
    .bind(revision)
    .get_result::<EffectiveHistoryRow>(conn)
    .await
    .optional()
    .map_err(database_error)
}
pub(crate) async fn layout_row(
    conn: &mut DatabaseSession,
    document_id: &str,
) -> Result<Option<LayoutRow>, StorageError> {
    statement("SELECT revision, layout_json FROM rule_document_layouts WHERE document_id = ?")
        .bind(document_id)
        .get_result::<LayoutRow>(conn)
        .await
        .optional()
        .map_err(database_error)
}

pub(crate) async fn provenance_row(
    conn: &mut DatabaseSession,
    document_id: &str,
) -> Result<Option<ProvenanceRow>, StorageError> {
    statement(
        "SELECT format, adapter_version, input_hash, source_text_secret_id, diagnostics_json, imported_at_ms FROM rule_document_provenances WHERE document_id = ?",
    )
    .bind(document_id)
    .get_result::<ProvenanceRow>(conn)
    .await
    .optional()
    .map_err(database_error)
}

/// 解密并返回 provenance 原文；无 provenance 行或未保存原文时返回 `None`。
///
/// 显式只读；只接受 owner 行恰好认领该随机 secret 的解密（防跨记录 ID 替换）。
pub(crate) async fn provenance_text(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    document_id: &str,
) -> Result<Option<String>, StorageError> {
    let Some(row) = provenance_row(conn, document_id).await? else {
        return Ok(None);
    };
    let Some(secret_id) = row.source_text_secret_id else {
        return Ok(None);
    };
    let secret_id = SecretArtifactId::from_str(&secret_id)?;
    let plaintext =
        read_owned_secret(conn, artifacts, secret_id, DOCUMENT_OWNER_KIND, document_id).await?;
    String::from_utf8(plaintext)
        .map(Some)
        .map_err(|_| StorageError::Serialization)
}

/// 在 writer transaction 内插入文档主行；state 固定 `draft`。
pub(crate) async fn insert_document(
    conn: &mut DatabaseSession,
    request: &CreateDocumentRequest,
) -> Result<(), StorageError> {
    let semantic_revision = request
        .initial
        .semantic
        .as_ref()
        .map_or(0, |value| value.revision);
    let layout_revision = request
        .initial
        .layout
        .as_ref()
        .map_or(0, |value| value.revision);
    statement(
        "INSERT INTO rule_documents (document_id, format, title, source_identity, state, semantic_revision, layout_revision, link_revision, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, 'draft', ?, ?, 0, ?, ?)",
    )
    .bind(&request.document_id)
    .bind(&request.format)
    .bind(&request.title)
    .bind(&request.source_identity)
    .bind(semantic_revision)
    .bind(layout_revision)
    .bind(request.occurred_at_ms)
    .bind(request.occurred_at_ms)
    .execute(conn)
    .await
    .map_err(database_error)?;
    Ok(())
}

/// 在 writer transaction 内插入 semantic 快照行。
pub(crate) async fn insert_semantic(
    conn: &mut DatabaseSession,
    document_id: &str,
    snapshot: &SemanticSnapshot,
    updated_at_ms: i64,
) -> Result<(), StorageError> {
    statement(
        "INSERT INTO rule_document_semantics (document_id, revision, definition_hash, definition_json, manifest_json, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(document_id)
    .bind(snapshot.revision)
    .bind(&snapshot.definition_hash)
    .bind(&snapshot.definition_json)
    .bind(&snapshot.manifest_json)
    .bind(updated_at_ms)
    .execute(conn)
    .await
    .map_err(database_error)?;
    Ok(())
}

/// 在 writer transaction 内替换最近一次 Effective Rule Revision。
pub(crate) async fn upsert_effective_semantic(
    conn: &mut DatabaseSession,
    document_id: &str,
    snapshot: &SemanticSnapshot,
    updated_at_ms: i64,
) -> Result<(), StorageError> {
    statement(
        "INSERT INTO rule_document_effective_semantics (document_id, revision, definition_hash, definition_json, manifest_json, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(document_id) DO UPDATE SET revision = excluded.revision, definition_hash = excluded.definition_hash, definition_json = excluded.definition_json, manifest_json = excluded.manifest_json, updated_at_ms = excluded.updated_at_ms",
    )
    .bind(document_id)
    .bind(snapshot.revision)
    .bind(&snapshot.definition_hash)
    .bind(&snapshot.definition_json)
    .bind(&snapshot.manifest_json)
    .bind(updated_at_ms)
    .execute(conn)
    .await
    .map_err(database_error)?;
    Ok(())
}

/// 在 writer transaction 内追加一个不可变 Effective Rule Revision 历史行。
pub(crate) async fn insert_effective_history(
    conn: &mut DatabaseSession,
    document_id: &str,
    snapshot: &SemanticSnapshot,
    effective_at_ms: i64,
) -> Result<(), StorageError> {
    statement(
        "INSERT INTO rule_document_effective_semantic_history (document_id, revision, definition_hash, definition_json, manifest_json, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(document_id)
    .bind(snapshot.revision)
    .bind(&snapshot.definition_hash)
    .bind(&snapshot.definition_json)
    .bind(&snapshot.manifest_json)
    .bind(effective_at_ms)
    .execute(conn)
    .await
    .map_err(database_error)?;
    Ok(())
}
/// 在 writer transaction 内插入 layout 快照行。
pub(crate) async fn insert_layout(
    conn: &mut DatabaseSession,
    document_id: &str,
    snapshot: &LayoutSnapshot,
    updated_at_ms: i64,
) -> Result<(), StorageError> {
    statement(
        "INSERT INTO rule_document_layouts (document_id, revision, layout_json, updated_at_ms) VALUES (?, ?, ?, ?)",
    )
    .bind(document_id)
    .bind(snapshot.revision)
    .bind(&snapshot.layout_json)
    .bind(updated_at_ms)
    .execute(conn)
    .await
    .map_err(database_error)?;
    Ok(())
}

/// 在 writer transaction 内插入 provenance 行；`source_text_secret_id` 来自已认领的 secret。
pub(crate) async fn insert_provenance(
    conn: &mut DatabaseSession,
    document_id: &str,
    input: &ProvenanceCreateInput,
    source_text_secret_id: Option<SecretArtifactId>,
) -> Result<(), StorageError> {
    statement(
        "INSERT INTO rule_document_provenances (document_id, format, adapter_version, input_hash, source_text_secret_id, diagnostics_json, imported_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(document_id)
    .bind(&input.format)
    .bind(&input.adapter_version)
    .bind(&input.input_hash)
    .bind(source_text_secret_id.map(|value| value.to_string()).as_deref())
    .bind(&input.diagnostics_json)
    .bind(input.imported_at_ms)
    .execute(conn)
    .await
    .map_err(database_error)?;
    Ok(())
}

/// 在 writer transaction 内替换 semantic 行；行缺失视为文档状态损坏。
pub(crate) async fn update_semantic(
    conn: &mut DatabaseSession,
    document_id: &str,
    revision: i64,
    definition_hash: &str,
    definition_json: &str,
    manifest_json: &str,
    updated_at_ms: i64,
) -> Result<(), StorageError> {
    let changed = statement(
        "UPDATE rule_document_semantics SET revision = ?, definition_hash = ?, definition_json = ?, manifest_json = ?, updated_at_ms = ? WHERE document_id = ?",
    )
    .bind(revision)
    .bind(definition_hash)
    .bind(definition_json)
    .bind(manifest_json)
    .bind(updated_at_ms)
    .bind(document_id)
    .execute(conn)
    .await
    .map_err(database_error)?;
    if changed != 1 {
        return Err(StorageError::Database(
            "semantic 行缺失但 revision 一致，状态损坏".to_string(),
        ));
    }
    Ok(())
}

/// 在 writer transaction 内替换 layout 行；行缺失视为文档状态损坏。
pub(crate) async fn update_layout(
    conn: &mut DatabaseSession,
    document_id: &str,
    revision: i64,
    layout_json: &str,
    updated_at_ms: i64,
) -> Result<(), StorageError> {
    let changed = statement(
        "UPDATE rule_document_layouts SET revision = ?, layout_json = ?, updated_at_ms = ? WHERE document_id = ?",
    )
    .bind(revision)
    .bind(layout_json)
    .bind(updated_at_ms)
    .bind(document_id)
    .execute(conn)
    .await
    .map_err(database_error)?;
    if changed != 1 {
        return Err(StorageError::Database(
            "layout 行缺失但 revision 一致，状态损坏".to_string(),
        ));
    }
    Ok(())
}

/// 在 writer transaction 内推进主行各域 revision 与 `updated_at_ms`；未写入的域传 `None`。
pub(crate) async fn advance_document(
    conn: &mut DatabaseSession,
    document_id: &str,
    semantic_revision: Option<i64>,
    layout_revision: Option<i64>,
    updated_at_ms: i64,
) -> Result<(), StorageError> {
    let changed = statement(
        "UPDATE rule_documents SET semantic_revision = COALESCE(?, semantic_revision), layout_revision = COALESCE(?, layout_revision), updated_at_ms = ? WHERE document_id = ?",
    )
    .bind(semantic_revision)
    .bind(layout_revision)
    .bind(updated_at_ms)
    .bind(document_id)
    .execute(conn)
    .await
    .map_err(database_error)?;
    if changed != 1 {
        return Err(StorageError::DocumentMissing);
    }
    Ok(())
}

/// 判断 `source_identity` 是否已被其他文档占用（create 预检，避免 UNIQUE 约束裸报错）。
pub(crate) async fn source_identity_exists(
    conn: &mut DatabaseSession,
    source_identity: &str,
) -> Result<bool, StorageError> {
    let row = statement("SELECT document_id FROM rule_documents WHERE source_identity = ?")
        .bind(source_identity)
        .get_result::<DocumentIdRow>(conn)
        .await
        .optional()
        .map_err(database_error)?;
    Ok(row.is_some_and(|value| !value.document_id.is_empty()))
}

/// 在 writer transaction 内重命名文档并返回最新主行；标题不推进任何 revision。
pub(crate) async fn rename_row(
    conn: &mut DatabaseSession,
    document_id: &str,
    title: &str,
    updated_at_ms: i64,
) -> Result<DocumentRow, StorageError> {
    let changed =
        statement("UPDATE rule_documents SET title = ?, updated_at_ms = ? WHERE document_id = ?")
            .bind(title)
            .bind(updated_at_ms)
            .bind(document_id)
            .execute(conn)
            .await
            .map_err(database_error)?;
    if changed != 1 {
        return Err(StorageError::DocumentMissing);
    }
    document_row(conn, document_id)
        .await?
        .ok_or(StorageError::DocumentMissing)
}

/// 释放文档的全部 secret owner：provenance 原文 owner + 各 revision 的凭证槽位 owner。
///
/// 凭证槽位 `owner_id` 以 document ID 为前缀；文档 ID 恒为 `native:<uuid>`（不含 `::`），
/// 前缀匹配不会跨文档误删。
pub(crate) async fn release_document_secret_owners(
    conn: &mut DatabaseSession,
    document_id: &str,
) -> Result<(), StorageError> {
    crate::repository::secret::release_secret_owner(conn, DOCUMENT_OWNER_KIND, document_id).await?;
    let rows = statement(
        "SELECT owner_id FROM secret_artifact_owners WHERE owner_kind = ? AND owner_id LIKE ?",
    )
    .bind(CREDENTIAL_OWNER_KIND)
    .bind(format!("{document_id}::%"))
    .load::<OwnerIdRow>(conn)
    .await
    .map_err(database_error)?;
    for row in rows {
        crate::repository::secret::release_secret_owner(conn, CREDENTIAL_OWNER_KIND, &row.owner_id)
            .await?;
    }
    Ok(())
}

/// 删除文档主行与全部子行；调用方必须先释放 secret owner。
pub(crate) async fn delete_document_rows(
    conn: &mut DatabaseSession,
    document_id: &str,
) -> Result<(), StorageError> {
    for sql in [
        "DELETE FROM rule_document_provenances WHERE document_id = ?",
        "DELETE FROM rule_document_semantics WHERE document_id = ?",
        "DELETE FROM rule_document_layouts WHERE document_id = ?",
        "DELETE FROM rule_documents WHERE document_id = ?",
    ] {
        statement(sql)
            .bind(document_id)
            .execute(conn)
            .await
            .map_err(database_error)?;
    }
    Ok(())
}

/// 构造 revision-scoped 凭证槽位 `owner_id`；与文档删除时的前缀释放编码一致。
pub(crate) fn credential_owner_id(
    document_id: &str,
    scope: &str,
    revision: i64,
    node_id: &str,
    json_pointer: &str,
) -> String {
    format!("{document_id}::{scope}::{revision}::{node_id}::{json_pointer}")
}

/// 释放某个 Draft/Effective Rule Revision 持有的全部凭证 owner。
pub(crate) async fn release_document_revision_secret_owners(
    conn: &mut DatabaseSession,
    document_id: &str,
    scope: &str,
    revision: i64,
) -> Result<(), StorageError> {
    let rows = statement(
        "SELECT owner_id FROM secret_artifact_owners WHERE owner_kind = ? AND owner_id LIKE ?",
    )
    .bind(CREDENTIAL_OWNER_KIND)
    .bind(format!("{document_id}::{scope}::{revision}::%"))
    .load::<OwnerIdRow>(conn)
    .await
    .map_err(database_error)?;
    for row in rows {
        crate::repository::secret::release_secret_owner(conn, CREDENTIAL_OWNER_KIND, &row.owner_id)
            .await?;
    }
    Ok(())
}

pub(crate) fn summary_from_row(row: DocumentRow) -> DocumentSummary {
    DocumentSummary {
        document_id: row.document_id,
        format: row.format,
        title: row.title,
        source_identity: row.source_identity,
        state: row.state,
        semantic_revision: row.semantic_revision,
        layout_revision: row.layout_revision,
        link_revision: row.link_revision,
        created_at_ms: row.created_at_ms,
        updated_at_ms: row.updated_at_ms,
    }
}
