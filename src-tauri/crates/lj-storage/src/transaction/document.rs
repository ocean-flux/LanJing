//! 原生规则文档生命周期 transaction service。
//!
//! create/rename/delete 各自在单个 `BEGIN IMMEDIATE` transaction 中提交；save 对
//! semantic/layout 两域分别做 expected revision 校验，只写有效域，任一底层失败整体回滚。

use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use crate::artifact::{ArtifactStore, PendingSecretArtifact};
use crate::database::DatabaseSession;
use crate::repository::document::{
    CREDENTIAL_OWNER_KIND, DOCUMENT_OWNER_KIND, DocumentRow, NATIVE_RULE_FORMAT, advance_document,
    credential_owner_id, delete_document_rows, document_row, effective_history_row,
    effective_semantic_row, insert_document, insert_effective_history, insert_layout,
    insert_provenance, insert_semantic, layout_row, release_document_revision_secret_owners,
    release_document_secret_owners, rename_row, semantic_row, source_identity_exists,
    summary_from_row, update_layout, update_semantic, upsert_effective_semantic,
};
use crate::repository::secret::{retain_existing_secret, retain_pending_secret, write_secret};
use crate::types::{
    CreateDocumentRequest, DeleteDocumentRequest, DocumentCredentialMutation,
    DocumentCredentialMutationAction, DocumentSummary, DomainSaveOutcome, LayoutSaveInput,
    LayoutSnapshot, RenameDocumentRequest, RestoreDocumentRevisionOutcome,
    RestoreDocumentRevisionRequest, RevisionConflict, SaveDocumentOutcome, SaveDocumentRequest,
    SecretArtifactId, SemanticActivation, SemanticSnapshot, StorageError,
};

const CREDENTIAL_MANIFEST_SCHEMA_VERSION: u32 = 1;
const DRAFT_CREDENTIAL_SCOPE: &str = "draft";
const EFFECTIVE_CREDENTIAL_SCOPE: &str = "effective";
const HISTORY_CREDENTIAL_SCOPE: &str = "history";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialManifest {
    schema_version: u32,
    #[serde(default)]
    slots: Vec<CredentialSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialSlot {
    logical_name: String,
    node_id: String,
    json_pointer: String,
    secret_id: SecretArtifactId,
    created_at_ms: i64,
}

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
            validate_create_request(transaction, &request).await?;
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
            if let Some(effective) = request.initial.effective_semantic.as_ref() {
                upsert_effective_semantic(
                    transaction,
                    &request.document_id,
                    effective,
                    request.occurred_at_ms,
                )
                .await?;
                let manifest = credential_manifest_from_snapshot(Some(&effective.manifest_json))?;
                let mut claimed_pending = HashSet::new();
                retain_credential_manifest_owners(
                    transaction,
                    &artifacts,
                    CredentialOwnerRetention {
                        document_id: &request.document_id,
                        scope: DRAFT_CREDENTIAL_SCOPE,
                        revision: effective.revision,
                        manifest: &manifest,
                        pending: &[],
                        occurred_at_ms: request.occurred_at_ms,
                    },
                    &mut claimed_pending,
                )
                .await?;
                retain_credential_manifest_owners(
                    transaction,
                    &artifacts,
                    CredentialOwnerRetention {
                        document_id: &request.document_id,
                        scope: EFFECTIVE_CREDENTIAL_SCOPE,
                        revision: effective.revision,
                        manifest: &manifest,
                        pending: &[],
                        occurred_at_ms: request.occurred_at_ms,
                    },
                    &mut claimed_pending,
                )
                .await?;
                retain_credential_manifest_owners(
                    transaction,
                    &artifacts,
                    CredentialOwnerRetention {
                        document_id: &request.document_id,
                        scope: HISTORY_CREDENTIAL_SCOPE,
                        revision: effective.revision,
                        manifest: &manifest,
                        pending: &[],
                        occurred_at_ms: request.occurred_at_ms,
                    },
                    &mut claimed_pending,
                )
                .await?;
                insert_effective_history(
                    transaction,
                    &request.document_id,
                    effective,
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

/// 校验 create 请求的格式、身份唯一性与各初始快照 revision；任一不成立即拒绝创建。
async fn validate_create_request(
    transaction: &mut DatabaseSession,
    request: &CreateDocumentRequest,
) -> Result<(), StorageError> {
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
    if let Some(effective) = request.initial.effective_semantic.as_ref() {
        let Some(semantic) = request.initial.semantic.as_ref() else {
            return Err(StorageError::InvalidInput(
                "有效语义快照必须同时存在草稿快照".to_string(),
            ));
        };
        if effective != semantic {
            return Err(StorageError::InvalidInput(
                "有效语义快照必须与初始草稿快照相同".to_string(),
            ));
        }
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
    Ok(())
}

/// 在单个 writer transaction 中保存文档：分别校验 semantic/layout 的 expected revision，
/// 仅有效域写 current+1，冲突域返回 expected/current 且不写；任一底层失败整体回滚。
pub(crate) async fn save(
    connection: &DatabaseSession,
    artifacts: &ArtifactStore,
    request: SaveDocumentRequest,
) -> Result<SaveDocumentOutcome, StorageError> {
    let artifacts = artifacts.clone();
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
            let (semantic, semantic_written) = match request.semantic.as_ref() {
                Some(input) => {
                    let (outcome, written) = save_semantic(
                        transaction,
                        &artifacts,
                        &document,
                        &request.document_id,
                        input,
                        request.occurred_at_ms,
                    )
                    .await?;
                    (Some(outcome), written)
                }
                None => (None, None),
            };
            let (layout, layout_written) = match request.layout.as_ref() {
                Some(input) => {
                    let (outcome, written) = save_layout(
                        transaction,
                        &document,
                        &request.document_id,
                        input,
                        request.occurred_at_ms,
                    )
                    .await?;
                    (Some(outcome), written)
                }
                None => (None, None),
            };
            if semantic_written.is_some() || layout_written.is_some() {
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
                semantic,
                layout,
            })
        })
    })
    .await
}

/// 从不可变 Effective 历史复制新的 Draft；历史行与 Effective 当前行均保持不变。
pub(crate) async fn restore_native_revision(
    connection: &DatabaseSession,
    artifacts: &ArtifactStore,
    request: RestoreDocumentRevisionRequest,
) -> Result<RestoreDocumentRevisionOutcome, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            let Some(document) = document_row(transaction, &request.document_id).await? else {
                return Err(StorageError::DocumentMissing);
            };
            if request.revision <= 0 || request.expected_revision < 0 {
                return Err(StorageError::InvalidInput(
                    "规则历史 revision 必须为正，expected revision 不能为负".to_string(),
                ));
            }
            if request.occurred_at_ms < 0 {
                return Err(StorageError::InvalidInput(
                    "occurred_at_ms 不能为负".to_string(),
                ));
            }
            if request.expected_revision != document.semantic_revision {
                return Ok(RestoreDocumentRevisionOutcome {
                    document_id: request.document_id,
                    revision: document.semantic_revision,
                    conflict: Some(RevisionConflict {
                        expected: request.expected_revision,
                        current: document.semantic_revision,
                    }),
                });
            }
            let Some(history) =
                effective_history_row(transaction, &request.document_id, request.revision).await?
            else {
                return Err(StorageError::RuleRevisionMissing);
            };
            if history.revision != request.revision {
                return Err(StorageError::Database(
                    "规则历史查询返回了错误 revision".to_string(),
                ));
            }
            let manifest = credential_manifest_from_snapshot(Some(&history.manifest_json))?;
            let new_revision = document.semantic_revision + 1;
            let mut claimed_pending = HashSet::new();
            retain_credential_manifest_owners(
                transaction,
                &artifacts,
                CredentialOwnerRetention {
                    document_id: &request.document_id,
                    scope: DRAFT_CREDENTIAL_SCOPE,
                    revision: new_revision,
                    manifest: &manifest,
                    pending: &[],
                    occurred_at_ms: request.occurred_at_ms,
                },
                &mut claimed_pending,
            )
            .await?;
            if let Some(current) = semantic_row(transaction, &request.document_id).await? {
                release_document_revision_secret_owners(
                    transaction,
                    &request.document_id,
                    DRAFT_CREDENTIAL_SCOPE,
                    current.revision,
                )
                .await?;
                update_semantic(
                    transaction,
                    &request.document_id,
                    new_revision,
                    &history.definition_hash,
                    &history.definition_json,
                    &history.manifest_json,
                    request.occurred_at_ms,
                )
                .await?;
            } else {
                insert_semantic(
                    transaction,
                    &request.document_id,
                    &SemanticSnapshot {
                        revision: new_revision,
                        definition_json: history.definition_json,
                        definition_hash: history.definition_hash,
                        manifest_json: history.manifest_json,
                    },
                    request.occurred_at_ms,
                )
                .await?;
            }
            advance_document(
                transaction,
                &request.document_id,
                Some(new_revision),
                None,
                request.occurred_at_ms,
            )
            .await?;
            Ok(RestoreDocumentRevisionOutcome {
                document_id: request.document_id,
                revision: new_revision,
                conflict: None,
            })
        })
    })
    .await
}

async fn save_semantic(
    transaction: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    document: &DocumentRow,
    document_id: &str,
    input: &crate::types::SemanticSaveInput,
    occurred_at_ms: i64,
) -> Result<(DomainSaveOutcome, Option<i64>), StorageError> {
    if input.expected_revision != document.semantic_revision {
        return Ok((
            DomainSaveOutcome {
                revision: document.semantic_revision,
                conflict: Some(RevisionConflict {
                    expected: input.expected_revision,
                    current: document.semantic_revision,
                }),
                activation: None,
            },
            None,
        ));
    }

    let new_revision = document.semantic_revision + 1;
    let current_semantic = semantic_row(transaction, document_id).await?;
    let current_effective = if input.activation == SemanticActivation::Effective {
        effective_semantic_row(transaction, document_id).await?
    } else {
        None
    };
    let mut manifest = credential_manifest_from_snapshot(
        current_semantic
            .as_ref()
            .map(|snapshot| snapshot.manifest_json.as_str()),
    )?;
    let pending = apply_credential_mutations(
        transaction,
        artifacts,
        &mut manifest,
        &input.credential_mutations,
        occurred_at_ms,
    )
    .await?;
    let snapshot = SemanticSnapshot {
        revision: new_revision,
        definition_json: input.definition_json.clone(),
        definition_hash: input.definition_hash.clone(),
        manifest_json: serialize_credential_manifest(&manifest)?,
    };
    let mut claimed_pending = HashSet::new();
    retain_credential_manifest_owners(
        transaction,
        artifacts,
        CredentialOwnerRetention {
            document_id,
            scope: DRAFT_CREDENTIAL_SCOPE,
            revision: new_revision,
            manifest: &manifest,
            pending: &pending,
            occurred_at_ms,
        },
        &mut claimed_pending,
    )
    .await?;
    if input.activation == SemanticActivation::Effective {
        retain_credential_manifest_owners(
            transaction,
            artifacts,
            CredentialOwnerRetention {
                document_id,
                scope: EFFECTIVE_CREDENTIAL_SCOPE,
                revision: new_revision,
                manifest: &manifest,
                pending: &pending,
                occurred_at_ms,
            },
            &mut claimed_pending,
        )
        .await?;
        retain_credential_manifest_owners(
            transaction,
            artifacts,
            CredentialOwnerRetention {
                document_id,
                scope: HISTORY_CREDENTIAL_SCOPE,
                revision: new_revision,
                manifest: &manifest,
                pending: &pending,
                occurred_at_ms,
            },
            &mut claimed_pending,
        )
        .await?;
    }
    if let Some(current) = current_effective.as_ref() {
        let current_manifest = credential_manifest_from_snapshot(Some(&current.manifest_json))?;
        retain_credential_manifest_owners(
            transaction,
            artifacts,
            CredentialOwnerRetention {
                document_id,
                scope: HISTORY_CREDENTIAL_SCOPE,
                revision: current.revision,
                manifest: &current_manifest,
                pending: &[],
                occurred_at_ms,
            },
            &mut claimed_pending,
        )
        .await?;
    }
    if let Some(current) = current_semantic.as_ref() {
        release_document_revision_secret_owners(
            transaction,
            document_id,
            DRAFT_CREDENTIAL_SCOPE,
            current.revision,
        )
        .await?;
    }
    if let Some(current) = current_effective.as_ref() {
        release_document_revision_secret_owners(
            transaction,
            document_id,
            EFFECTIVE_CREDENTIAL_SCOPE,
            current.revision,
        )
        .await?;
    }
    if current_semantic.is_some() {
        update_semantic(
            transaction,
            document_id,
            new_revision,
            &input.definition_hash,
            &input.definition_json,
            &snapshot.manifest_json,
            occurred_at_ms,
        )
        .await?;
    } else {
        insert_semantic(transaction, document_id, &snapshot, occurred_at_ms).await?;
    }
    if input.activation == SemanticActivation::Effective {
        upsert_effective_semantic(transaction, document_id, &snapshot, occurred_at_ms).await?;
        insert_effective_history(transaction, document_id, &snapshot, occurred_at_ms).await?;
    }
    Ok((
        DomainSaveOutcome {
            revision: new_revision,
            conflict: None,
            activation: Some(input.activation),
        },
        Some(new_revision),
    ))
}

async fn save_layout(
    transaction: &mut DatabaseSession,
    document: &DocumentRow,
    document_id: &str,
    input: &LayoutSaveInput,
    occurred_at_ms: i64,
) -> Result<(DomainSaveOutcome, Option<i64>), StorageError> {
    if input.expected_revision != document.layout_revision {
        return Ok((
            DomainSaveOutcome {
                revision: document.layout_revision,
                conflict: Some(RevisionConflict {
                    expected: input.expected_revision,
                    current: document.layout_revision,
                }),
                activation: None,
            },
            None,
        ));
    }
    let new_revision = document.layout_revision + 1;
    let snapshot = LayoutSnapshot {
        revision: new_revision,
        layout_json: input.layout_json.clone(),
    };
    if layout_row(transaction, document_id).await?.is_some() {
        update_layout(
            transaction,
            document_id,
            new_revision,
            &input.layout_json,
            occurred_at_ms,
        )
        .await?;
    } else {
        insert_layout(transaction, document_id, &snapshot, occurred_at_ms).await?;
    }
    Ok((
        DomainSaveOutcome {
            revision: new_revision,
            conflict: None,
            activation: None,
        },
        Some(new_revision),
    ))
}

fn credential_manifest_from_snapshot(
    manifest_json: Option<&str>,
) -> Result<CredentialManifest, StorageError> {
    let manifest = match manifest_json {
        Some(json) => serde_json::from_str(json).map_err(|error| {
            StorageError::InvalidInput(format!("credential manifest 无法解析：{error}"))
        })?,
        None => CredentialManifest {
            schema_version: CREDENTIAL_MANIFEST_SCHEMA_VERSION,
            slots: Vec::new(),
        },
    };
    validate_credential_manifest(&manifest)?;
    Ok(manifest)
}

fn serialize_credential_manifest(manifest: &CredentialManifest) -> Result<String, StorageError> {
    serde_json::to_string(manifest).map_err(|error| {
        StorageError::InvalidInput(format!("credential manifest 无法序列化：{error}"))
    })
}

fn validate_credential_manifest(manifest: &CredentialManifest) -> Result<(), StorageError> {
    if manifest.schema_version != CREDENTIAL_MANIFEST_SCHEMA_VERSION {
        return Err(StorageError::InvalidInput(
            "credential manifest schema_version 不受支持".to_string(),
        ));
    }
    let mut locations = BTreeSet::new();
    for slot in &manifest.slots {
        if slot.logical_name.is_empty() || slot.node_id.is_empty() || slot.json_pointer.is_empty() {
            return Err(StorageError::InvalidInput(
                "credential manifest 槽位字段不能为空".to_string(),
            ));
        }
        if !locations.insert((&slot.node_id, &slot.json_pointer)) {
            return Err(StorageError::InvalidInput(
                "credential manifest 含重复槽位".to_string(),
            ));
        }
    }
    Ok(())
}

async fn apply_credential_mutations(
    transaction: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    manifest: &mut CredentialManifest,
    mutations: &[DocumentCredentialMutation],
    occurred_at_ms: i64,
) -> Result<Vec<PendingSecretArtifact>, StorageError> {
    let mut pending = Vec::with_capacity(mutations.len());
    for mutation in mutations {
        if mutation.node_id.is_empty()
            || mutation.json_pointer.is_empty()
            || mutation.logical_name.is_empty()
        {
            return Err(StorageError::InvalidInput(
                "credential mutation 槽位字段不能为空".to_string(),
            ));
        }
        match mutation.action {
            DocumentCredentialMutationAction::Replace => {
                let value = mutation.value.as_deref().ok_or_else(|| {
                    StorageError::InvalidInput("replace credential mutation 缺少值".to_string())
                })?;
                if value.is_empty() {
                    return Err(StorageError::InvalidInput("凭证值不能为空".to_string()));
                }
                let secret =
                    write_secret(transaction, artifacts, value.as_bytes(), occurred_at_ms).await?;
                let slot = CredentialSlot {
                    logical_name: mutation.logical_name.clone(),
                    node_id: mutation.node_id.clone(),
                    json_pointer: mutation.json_pointer.clone(),
                    secret_id: secret.secret_id,
                    created_at_ms: occurred_at_ms,
                };
                if let Some(index) = manifest.slots.iter().position(|existing| {
                    existing.node_id == mutation.node_id
                        && existing.json_pointer == mutation.json_pointer
                }) {
                    manifest.slots[index] = slot;
                } else {
                    manifest.slots.push(slot);
                }
                pending.push(secret);
            }
            DocumentCredentialMutationAction::Clear => {
                if mutation.value.is_some() {
                    return Err(StorageError::InvalidInput(
                        "clear credential mutation 不能携带值".to_string(),
                    ));
                }
                manifest.slots.retain(|slot| {
                    slot.node_id != mutation.node_id || slot.json_pointer != mutation.json_pointer
                });
            }
        }
    }
    validate_credential_manifest(manifest)?;
    Ok(pending)
}

struct CredentialOwnerRetention<'a> {
    document_id: &'a str,
    scope: &'a str,
    revision: i64,
    manifest: &'a CredentialManifest,
    pending: &'a [PendingSecretArtifact],
    occurred_at_ms: i64,
}

async fn retain_credential_manifest_owners(
    transaction: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    retention: CredentialOwnerRetention<'_>,
    claimed_pending: &mut HashSet<SecretArtifactId>,
) -> Result<(), StorageError> {
    for slot in &retention.manifest.slots {
        let owner_id = credential_owner_id(
            retention.document_id,
            retention.scope,
            retention.revision,
            &slot.node_id,
            &slot.json_pointer,
        );
        if let Some(secret) = retention
            .pending
            .iter()
            .find(|candidate| candidate.secret_id == slot.secret_id)
            && claimed_pending.insert(slot.secret_id)
        {
            retain_pending_secret(
                transaction,
                secret,
                CREDENTIAL_OWNER_KIND,
                &owner_id,
                retention.occurred_at_ms,
            )
            .await?;
        } else {
            retain_existing_secret(
                transaction,
                artifacts,
                slot.secret_id,
                CREDENTIAL_OWNER_KIND,
                &owner_id,
                retention.occurred_at_ms,
            )
            .await?;
        }
    }
    Ok(())
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
