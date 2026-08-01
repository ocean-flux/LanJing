//! 原生规则文档生命周期 transaction service。
//!
//! create/rename/delete 各自在单个 `BEGIN IMMEDIATE` transaction 中提交；save 对
//! semantic/layout 两域分别做 expected revision 校验，只写有效域，任一底层失败整体回滚。

use crate::artifact::ArtifactStore;
use crate::database::DatabaseSession;
use crate::repository::document::{
    CREDENTIAL_OWNER_KIND, DOCUMENT_OWNER_KIND, NATIVE_RULE_FORMAT, advance_document,
    credential_owner_id, delete_document_rows, document_row, insert_document, insert_layout,
    insert_provenance, insert_semantic, layout_row, release_document_secret_owners, rename_row,
    semantic_row, source_identity_exists, summary_from_row, update_layout, update_semantic,
};
use crate::repository::secret::{release_secret_owner, retain_pending_secret, write_secret};
use crate::types::{
    ClearDocumentCredentialSecretRequest, CreateDocumentRequest, DeleteDocumentRequest,
    DocumentSummary, DomainSaveOutcome, LayoutSnapshot, RenameDocumentRequest, RevisionConflict,
    SaveDocumentOutcome, SaveDocumentRequest, SecretArtifactId, SemanticSnapshot, StorageError,
    WriteDocumentCredentialSecretRequest,
};

/// 在单个 writer transaction 中创建文档：主行 + optional semantic/layout 快照 +
/// optional provenance（原文加密为 secret artifact）。
pub(crate) async fn create(
    connection: &DatabaseSession,
    artifacts: &ArtifactStore,
    request: CreateDocumentRequest,
) -> Result<DocumentSummary, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            if request.format != NATIVE_RULE_FORMAT {
                return Err(StorageError::InvalidInput(format!(
                    "不支持的文档 format：{}",
                    request.format
                )));
            }
            if request.document_id.is_empty() || request.source_identity.is_empty() {
                return Err(StorageError::InvalidInput(
                    "document_id/source_identity 不能为空".to_string(),
                ));
            }
            if request.occurred_at_ms < 0 {
                return Err(StorageError::InvalidInput(
                    "occurred_at_ms 不能为负".to_string(),
                ));
            }
            if document_row(transaction, &request.document_id)
                .await?
                .is_some()
            {
                return Err(StorageError::InvalidInput("文档已存在".to_string()));
            }
            if source_identity_exists(transaction, &request.source_identity).await? {
                return Err(StorageError::InvalidInput(format!(
                    "source_identity 已存在：{}",
                    request.source_identity
                )));
            }
            if let Some(semantic) = request.initial.semantic.as_ref()
                && semantic.revision <= 0
            {
                return Err(StorageError::InvalidInput(
                    "初始 semantic revision 必须为正".to_string(),
                ));
            }
            if let Some(layout) = request.initial.layout.as_ref()
                && layout.revision <= 0
            {
                return Err(StorageError::InvalidInput(
                    "初始 layout revision 必须为正".to_string(),
                ));
            }
            if let Some(input) = request.provenance.as_ref()
                && input.imported_at_ms < 0
            {
                return Err(StorageError::InvalidInput(
                    "provenance imported_at_ms 不能为负".to_string(),
                ));
            }
            // provenance 原文先写 secret 文件，再在事务内认领 metadata + owner。
            let provenance_secret = match request.provenance.as_ref() {
                Some(input) => Some(
                    write_secret(
                        transaction,
                        &artifacts,
                        input.source_text.as_bytes(),
                        request.occurred_at_ms,
                    )
                    .await?,
                ),
                None => None,
            };
            insert_document(transaction, &request).await?;
            if let Some(semantic) = request.initial.semantic.as_ref() {
                insert_semantic(
                    transaction,
                    &request.document_id,
                    semantic,
                    request.occurred_at_ms,
                )
                .await?;
            }
            if let Some(layout) = request.initial.layout.as_ref() {
                insert_layout(
                    transaction,
                    &request.document_id,
                    layout,
                    request.occurred_at_ms,
                )
                .await?;
            }
            if let (Some(input), Some(pending)) =
                (request.provenance.as_ref(), provenance_secret.as_ref())
            {
                retain_pending_secret(
                    transaction,
                    pending,
                    DOCUMENT_OWNER_KIND,
                    &request.document_id,
                    request.occurred_at_ms,
                )
                .await?;
                insert_provenance(
                    transaction,
                    &request.document_id,
                    input,
                    Some(pending.secret_id),
                )
                .await?;
            }
            document_row(transaction, &request.document_id)
                .await?
                .map(summary_from_row)
                .ok_or(StorageError::DocumentMissing)
        })
    })
    .await
}

/// 在单个 writer transaction 中保存文档：分别校验 semantic/layout 的 expected revision，
/// 仅有效域写 current+1，冲突域返回 expected/current 且不写；任一底层失败整体回滚。
pub(crate) async fn save(
    connection: &DatabaseSession,
    request: SaveDocumentRequest,
) -> Result<SaveDocumentOutcome, StorageError> {
    super::run(connection, move |transaction| {
        Box::pin(async move {
            let Some(document) = document_row(transaction, &request.document_id).await? else {
                return Err(StorageError::DocumentMissing);
            };
            if request.occurred_at_ms < 0 {
                return Err(StorageError::InvalidInput(
                    "occurred_at_ms 不能为负".to_string(),
                ));
            }
            let mut semantic_outcome: Option<DomainSaveOutcome> = None;
            let mut layout_outcome: Option<DomainSaveOutcome> = None;
            let mut any_written = false;
            let mut semantic_written: Option<i64> = None;
            let mut layout_written: Option<i64> = None;
            if let Some(input) = request.semantic.as_ref() {
                if input.expected_revision == document.semantic_revision {
                    let new_revision = document.semantic_revision + 1;
                    let snapshot = SemanticSnapshot {
                        revision: new_revision,
                        definition_json: input.definition_json.clone(),
                        definition_hash: input.definition_hash.clone(),
                        manifest_json: input.manifest_json.clone(),
                    };
                    if semantic_row(transaction, &request.document_id)
                        .await?
                        .is_some()
                    {
                        update_semantic(
                            transaction,
                            &request.document_id,
                            new_revision,
                            &input.definition_hash,
                            &input.definition_json,
                            &input.manifest_json,
                            request.occurred_at_ms,
                        )
                        .await?;
                    } else {
                        insert_semantic(
                            transaction,
                            &request.document_id,
                            &snapshot,
                            request.occurred_at_ms,
                        )
                        .await?;
                    }
                    semantic_outcome = Some(DomainSaveOutcome {
                        revision: new_revision,
                        conflict: None,
                    });
                    semantic_written = Some(new_revision);
                    any_written = true;
                } else {
                    semantic_outcome = Some(DomainSaveOutcome {
                        revision: document.semantic_revision,
                        conflict: Some(RevisionConflict {
                            expected: input.expected_revision,
                            current: document.semantic_revision,
                        }),
                    });
                }
            }
            if let Some(input) = request.layout.as_ref() {
                if input.expected_revision == document.layout_revision {
                    let new_revision = document.layout_revision + 1;
                    let snapshot = LayoutSnapshot {
                        revision: new_revision,
                        layout_json: input.layout_json.clone(),
                    };
                    if layout_row(transaction, &request.document_id)
                        .await?
                        .is_some()
                    {
                        update_layout(
                            transaction,
                            &request.document_id,
                            new_revision,
                            &input.layout_json,
                            request.occurred_at_ms,
                        )
                        .await?;
                    } else {
                        insert_layout(
                            transaction,
                            &request.document_id,
                            &snapshot,
                            request.occurred_at_ms,
                        )
                        .await?;
                    }
                    layout_outcome = Some(DomainSaveOutcome {
                        revision: new_revision,
                        conflict: None,
                    });
                    layout_written = Some(new_revision);
                } else {
                    layout_outcome = Some(DomainSaveOutcome {
                        revision: document.layout_revision,
                        conflict: Some(RevisionConflict {
                            expected: input.expected_revision,
                            current: document.layout_revision,
                        }),
                    });
                }
            }
            if any_written {
                advance_document(
                    transaction,
                    &request.document_id,
                    semantic_written,
                    layout_written,
                    request.occurred_at_ms,
                )
                .await?;
            }
            Ok(SaveDocumentOutcome {
                document_id: request.document_id,
                semantic: semantic_outcome,
                layout: layout_outcome,
            })
        })
    })
    .await
}

/// 在单个 writer transaction 中重命名文档；expected revision 不匹配时拒绝，
/// 标题不推进 semantic revision。
pub(crate) async fn rename(
    connection: &DatabaseSession,
    request: RenameDocumentRequest,
) -> Result<DocumentSummary, StorageError> {
    super::run(connection, move |transaction| {
        Box::pin(async move {
            let Some(document) = document_row(transaction, &request.document_id).await? else {
                return Err(StorageError::DocumentMissing);
            };
            if request.expected_revision != document.semantic_revision {
                return Err(StorageError::InvalidInput(format!(
                    "文档 revision 冲突：期望 {}，实际为 {}",
                    request.expected_revision, document.semantic_revision
                )));
            }
            if request.title.trim().is_empty() {
                return Err(StorageError::InvalidInput("标题不能为空".to_string()));
            }
            if request.occurred_at_ms < 0 {
                return Err(StorageError::InvalidInput(
                    "occurred_at_ms 不能为负".to_string(),
                ));
            }
            let row = rename_row(
                transaction,
                &request.document_id,
                &request.title,
                request.occurred_at_ms,
            )
            .await?;
            Ok(summary_from_row(row))
        })
    })
    .await
}

/// 在单个 writer transaction 中删除文档：linked 且未确认时守卫拒绝；
/// 释放全部 secret owner（provenance 原文 + 凭证槽位），显式删除子行与主行。
pub(crate) async fn delete(
    connection: &DatabaseSession,
    request: DeleteDocumentRequest,
) -> Result<(), StorageError> {
    super::run(connection, move |transaction| {
        Box::pin(async move {
            let Some(document) = document_row(transaction, &request.document_id).await? else {
                return Err(StorageError::DocumentMissing);
            };
            if document.state == "linked" && !request.confirm_linked {
                return Err(StorageError::InvalidInput(
                    "文档已链接到已安装来源，删除需要确认".to_string(),
                ));
            }
            if request.occurred_at_ms < 0 {
                return Err(StorageError::InvalidInput(
                    "occurred_at_ms 不能为负".to_string(),
                ));
            }
            release_document_secret_owners(transaction, &request.document_id).await?;
            delete_document_rows(transaction, &request.document_id).await?;
            Ok(())
        })
    })
    .await
}

/// 在单个 writer transaction 中写入（或替换）一个文档凭证槽位 secret；
/// replace 先释放旧 owner 再写新，同一 owner 编码下天然支持多槽位。
pub(crate) async fn write_credential_secret(
    connection: &DatabaseSession,
    artifacts: &ArtifactStore,
    request: WriteDocumentCredentialSecretRequest,
) -> Result<SecretArtifactId, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            if document_row(transaction, &request.document_id)
                .await?
                .is_none()
            {
                return Err(StorageError::DocumentMissing);
            }
            if request.value.is_empty() {
                return Err(StorageError::InvalidInput("凭证值不能为空".to_string()));
            }
            if request.node_id.is_empty() || request.json_pointer.is_empty() {
                return Err(StorageError::InvalidInput(
                    "node_id/json_pointer 不能为空".to_string(),
                ));
            }
            let owner_id = credential_owner_id(
                &request.document_id,
                &request.node_id,
                &request.json_pointer,
            );
            release_secret_owner(transaction, CREDENTIAL_OWNER_KIND, &owner_id).await?;
            let pending = write_secret(
                transaction,
                &artifacts,
                request.value.as_bytes(),
                request.occurred_at_ms,
            )
            .await?;
            retain_pending_secret(
                transaction,
                &pending,
                CREDENTIAL_OWNER_KIND,
                &owner_id,
                request.occurred_at_ms,
            )
            .await?;
            Ok(pending.secret_id)
        })
    })
    .await
}

/// 在单个 writer transaction 中清除文档凭证槽位 secret owner；不存在视为幂等成功。
pub(crate) async fn clear_credential_secret(
    connection: &DatabaseSession,
    request: ClearDocumentCredentialSecretRequest,
) -> Result<(), StorageError> {
    super::run(connection, move |transaction| {
        Box::pin(async move {
            if document_row(transaction, &request.document_id)
                .await?
                .is_none()
            {
                return Err(StorageError::DocumentMissing);
            }
            let owner_id = credential_owner_id(
                &request.document_id,
                &request.node_id,
                &request.json_pointer,
            );
            release_secret_owner(transaction, CREDENTIAL_OWNER_KIND, &owner_id).await?;
            Ok(())
        })
    })
    .await
}
