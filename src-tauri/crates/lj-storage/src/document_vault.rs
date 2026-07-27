//! 来源文档 current revision、masked query 与 credential ownership 的 writer/read 实现。
//!
//! storage 只校验 byte limit、document/revision/slot ownership 与 authoring owner 已给出的 issues；
//! 不解析 Legado/Maccms 字段。正文、manifest 和 slot value 都通过随机 secret artifact owner 原子
//! 认领，Event 只记录 document ID/revision/masked hash/schema。

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Nullable, Text};
use diesel::sqlite::SqliteConnection;
use lj_rule_model::{
    CREDENTIAL_SCHEMA_VERSION, CredentialSlotId, CredentialSlotManifest, EventType,
    SourceDocumentFormat,
};
use uuid::Uuid;

use crate::artifact::{ArtifactStore, PendingSecretArtifact};
use crate::event_store::{
    EventDraft, append_event_transaction, database_error, from_i64, stream_version, to_i64,
};
use crate::secret_artifact::{
    ensure_vault_key, read_owned_secret, release_secret_owner, retain_existing_secret,
    retain_pending_secret,
};
use crate::types::{
    CreateSourceDocumentInput, CredentialSlotMaterial, CredentialSlotSummary,
    DeleteSourceDocumentInput, DocumentMutationOutcome, DocumentRef, DocumentValidationIssue,
    EditSourceDocumentCredentialInput, LoadSourceDocumentRebaseMaterialInput,
    MAX_SOURCE_DOCUMENT_BYTES, MaskedSourceDocument, PinSourceDocumentRevisionInput,
    RebaseSourceDocumentInput, RenameSourceDocumentInput, RevealedSourceDocumentCredential,
    SOURCE_DOCUMENT_SCHEMA_VERSION, SaveSourceDocumentInput, SecretArtifactId,
    SourceDocumentCredentialTarget, SourceDocumentId, SourceDocumentMaterial,
    SourceDocumentRebaseCommitMode, SourceDocumentRebaseInvalidReason,
    SourceDocumentRebaseMaterialOutcome, SourceDocumentRebaseMaterials,
    SourceDocumentRevisionInput, SourceDocumentRevisionPin, SourceDocumentState,
    SourceDocumentSummary, StorageError,
};

const TITLE_MAX_BYTES: usize = 512;
const DOCUMENT_EVENT_SCHEMA_VERSION: u32 = 1;

struct PreparedSlot {
    slot_id: CredentialSlotId,
    path: String,
    name: String,
    secret: PendingSecretArtifact,
}

struct PreparedRevision {
    format: SourceDocumentFormat,
    masked_text: String,
    masked_hash: String,
    masked: PendingSecretArtifact,
    raw: PendingSecretArtifact,
    manifest: PendingSecretArtifact,
    slots: Vec<PreparedSlot>,
}

struct MaskedDocumentMetadata {
    document_id: SourceDocumentId,
    title: String,
    state: SourceDocumentState,
    source_identity: Option<String>,
    revision: u64,
    installed_revision: Option<u64>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

/// 按更新时间倒序读取安全 document summaries；不解密任何 artifact。
pub(crate) fn list_source_documents_sync(
    conn: &mut SqliteConnection,
) -> Result<Vec<SourceDocumentSummary>, StorageError> {
    sql_query(
        "SELECT document_id, format, title, state, source_identity, installed_revision, current_document_revision, masked_secret_id, raw_secret_id, manifest_secret_id, masked_hash, credential_slot_count, schema_version, created_at_ms, updated_at_ms FROM source_document_projection ORDER BY updated_at_ms DESC, document_id ASC",
    )
    .load::<DocumentRow>(conn)
    .map_err(database_error)?
    .into_iter()
    .map(summary_from_row)
    .collect()
}

/// 默认读取只解密 masked text，并从安全 slot rows 构造摘要。
pub(crate) fn get_source_document_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    document_id: SourceDocumentId,
) -> Result<Option<MaskedSourceDocument>, StorageError> {
    let Some(row) = document_row(conn, document_id)? else {
        return Ok(None);
    };
    masked_document_from_row(conn, artifacts, row).map(Some)
}

/// 读取 current exact revision 的完整 secret material；普通 stale revision 返回 `Ok(None)`。
pub(crate) fn load_source_document_material_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    document_ref: DocumentRef,
) -> Result<Option<SourceDocumentMaterial>, StorageError> {
    let Some(row) = document_row(conn, document_ref.document_id)? else {
        return Ok(None);
    };
    let actual_revision = from_i64(row.current_document_revision, "document revision")?;
    if document_ref.document_revision == 0 || document_ref.document_revision != actual_revision {
        return Ok(None);
    }
    let format = format_from_db(&row.format)?;
    let masked_text = read_utf8_secret(
        conn,
        artifacts,
        SecretArtifactId::from_str(&row.masked_secret_id)?,
        "document_masked",
        &revision_owner_id(
            document_ref.document_id,
            document_ref.document_revision,
            "masked",
        ),
    )?;
    if blake3::hash(masked_text.as_bytes()).to_hex().as_str() != row.masked_hash {
        return Err(StorageError::ArtifactCorrupt);
    }
    let raw_text = read_utf8_secret(
        conn,
        artifacts,
        SecretArtifactId::from_str(&row.raw_secret_id)?,
        "document_raw",
        &revision_owner_id(
            document_ref.document_id,
            document_ref.document_revision,
            "raw",
        ),
    )?;
    let manifest_bytes = read_owned_secret(
        conn,
        artifacts,
        SecretArtifactId::from_str(&row.manifest_secret_id)?,
        "document_manifest",
        &revision_owner_id(
            document_ref.document_id,
            document_ref.document_revision,
            "manifest",
        ),
    )?;
    let manifest: CredentialSlotManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| StorageError::ArtifactCorrupt)?;
    validate_manifest_target(
        &manifest,
        document_ref.document_id,
        document_ref.document_revision,
        format,
    )
    .map_err(|_| StorageError::ArtifactCorrupt)?;

    let slot_rows = document_slot_rows(conn, document_ref)?;
    if slot_rows.len() != manifest.slots.len() {
        return Err(StorageError::ArtifactCorrupt);
    }
    let mut credentials = Vec::with_capacity(slot_rows.len());
    for slot in slot_rows {
        let slot_id = parse_slot_id(&slot.slot_id)?;
        let manifest_slot = manifest
            .slots
            .iter()
            .find(|candidate| candidate.slot_id == slot_id)
            .ok_or(StorageError::ArtifactCorrupt)?;
        if manifest_slot.path != slot.path || manifest_slot.name != slot.name {
            return Err(StorageError::ArtifactCorrupt);
        }
        let value = read_utf8_secret(
            conn,
            artifacts,
            SecretArtifactId::from_str(&slot.secret_id)?,
            "document_slot",
            &slot_owner_id(
                document_ref.document_id,
                document_ref.document_revision,
                slot_id,
            ),
        )?;
        credentials.push(crate::types::CredentialSlotMaterial::new(slot_id, value));
    }
    Ok(Some(SourceDocumentMaterial::new(
        document_ref,
        format,
        masked_text,
        raw_text,
        manifest,
        credentials,
    )))
}

/// 固定 current exact revision 的全部 secret refs 和 slot metadata。
pub(crate) fn process_pin_source_document_revision(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    input: PinSourceDocumentRevisionInput,
) -> Result<SourceDocumentRevisionPin, StorageError> {
    if input.document_ref.document_revision == 0 || input.expires_at_ms <= input.created_at_ms {
        return Err(StorageError::InvalidInput(
            "来源文档 revision pin 参数无效".to_string(),
        ));
    }
    release_expired_source_document_revision_pins(conn, input.created_at_ms)?;
    let row =
        document_row(conn, input.document_ref.document_id)?.ok_or(StorageError::DocumentMissing)?;
    let actual_revision = from_i64(row.current_document_revision, "document revision")?;
    if actual_revision != input.document_ref.document_revision {
        return Err(StorageError::VersionConflict {
            stream_id: document_stream_id(input.document_ref.document_id),
            expected: input.document_ref.document_revision,
            actual: actual_revision,
        });
    }
    // 完整读取一次 current material，先证明 projection、owner、manifest 与密文都一致。
    let material = load_source_document_material_sync(conn, artifacts, input.document_ref)?
        .ok_or(StorageError::DocumentMissing)?;
    drop(material);
    let slots = document_slot_rows(conn, input.document_ref)?;
    let pin = SourceDocumentRevisionPin {
        pin_id: input.pin_id,
        document_id: input.document_ref.document_id,
        document_revision: input.document_ref.document_revision,
        expires_at_ms: input.expires_at_ms,
    };
    conn.immediate_transaction::<_, StorageError, _>(|conn| {
        sql_query(
            "INSERT INTO source_document_revision_pins (pin_id, document_id, document_revision, format, masked_secret_id, raw_secret_id, manifest_secret_id, masked_hash, schema_version, expires_at_ms, created_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(input.pin_id.to_string())
        .bind::<Text, _>(input.document_ref.document_id.to_string())
        .bind::<BigInt, _>(to_i64(input.document_ref.document_revision)?)
        .bind::<Text, _>(&row.format)
        .bind::<Text, _>(&row.masked_secret_id)
        .bind::<Text, _>(&row.raw_secret_id)
        .bind::<Text, _>(&row.manifest_secret_id)
        .bind::<Text, _>(&row.masked_hash)
        .bind::<BigInt, _>(row.schema_version)
        .bind::<BigInt, _>(input.expires_at_ms)
        .bind::<BigInt, _>(input.created_at_ms)
        .execute(conn)
        .map_err(database_error)?;
        let owner_id = pin_owner_id(input.pin_id);
        for (owner_kind, secret_id) in [
            ("document_revision_pin_masked", row.masked_secret_id.as_str()),
            ("document_revision_pin_raw", row.raw_secret_id.as_str()),
            (
                "document_revision_pin_manifest",
                row.manifest_secret_id.as_str(),
            ),
        ] {
            retain_existing_secret(
                conn,
                artifacts,
                SecretArtifactId::from_str(secret_id)?,
                owner_kind,
                &owner_id,
                input.created_at_ms,
            )?;
        }
        for slot in &slots {
            sql_query(
                "INSERT INTO source_document_revision_pin_slots (pin_id, slot_id, path, name, secret_id, schema_version, created_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<Text, _>(input.pin_id.to_string())
            .bind::<Text, _>(&slot.slot_id)
            .bind::<Text, _>(&slot.path)
            .bind::<Text, _>(&slot.name)
            .bind::<Text, _>(&slot.secret_id)
            .bind::<BigInt, _>(i64::from(CREDENTIAL_SCHEMA_VERSION))
            .bind::<BigInt, _>(input.created_at_ms)
            .execute(conn)
            .map_err(database_error)?;
            let slot_id = parse_slot_id(&slot.slot_id)?;
            retain_existing_secret(
                conn,
                artifacts,
                SecretArtifactId::from_str(&slot.secret_id)?,
                "document_revision_pin_slot",
                &pin_slot_owner_id(input.pin_id, slot_id),
                input.created_at_ms,
            )?;
        }
        Ok(())
    })?;
    Ok(pin)
}

/// 幂等释放 revision pin 及其全部 secret owners；未知 pin 同样成功。
pub(crate) fn process_release_source_document_revision_pin(
    conn: &mut SqliteConnection,
    pin_id: Uuid,
) -> Result<(), StorageError> {
    if revision_pin_row(conn, pin_id)?.is_none() {
        return Ok(());
    }
    let slots = revision_pin_slot_rows(conn, pin_id)?;
    conn.immediate_transaction::<_, StorageError, _>(|conn| {
        let owner_id = pin_owner_id(pin_id);
        for owner_kind in [
            "document_revision_pin_masked",
            "document_revision_pin_raw",
            "document_revision_pin_manifest",
        ] {
            release_secret_owner(conn, owner_kind, &owner_id)?;
        }
        for slot in &slots {
            release_secret_owner(
                conn,
                "document_revision_pin_slot",
                &pin_slot_owner_id(pin_id, parse_slot_id(&slot.slot_id)?),
            )?;
        }
        sql_query("DELETE FROM source_document_revision_pins WHERE pin_id = ?")
            .bind::<Text, _>(pin_id.to_string())
            .execute(conn)
            .map_err(database_error)?;
        Ok(())
    })
}

/// 释放所有到期 pin；每个 pin 的 owner/ref-count 变化均在独立事务内幂等完成。
pub(crate) fn release_expired_source_document_revision_pins(
    conn: &mut SqliteConnection,
    now_ms: i64,
) -> Result<usize, StorageError> {
    let rows = sql_query(
        "SELECT pin_id FROM source_document_revision_pins WHERE expires_at_ms <= ? ORDER BY pin_id ASC",
    )
    .bind::<BigInt, _>(now_ms)
    .load::<PinIdRow>(conn)
    .map_err(database_error)?;
    let mut released = 0_usize;
    for row in rows {
        process_release_source_document_revision_pin(conn, parse_pin_id(&row.pin_id)?)?;
        released = released.saturating_add(1);
    }
    Ok(released)
}

/// 从 pin 恢复可信 base material，并在同一 writer 顺序点固定 current material。
pub(crate) fn process_load_source_document_rebase_material(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    input: LoadSourceDocumentRebaseMaterialInput,
) -> Result<SourceDocumentRebaseMaterialOutcome, StorageError> {
    let Some(pin) = revision_pin_row(conn, input.pin_id)? else {
        return Ok(SourceDocumentRebaseMaterialOutcome::Invalid(
            SourceDocumentRebaseInvalidReason::PinNotFound,
        ));
    };
    if pin.expires_at_ms <= input.now_ms {
        process_release_source_document_revision_pin(conn, input.pin_id)?;
        return Ok(SourceDocumentRebaseMaterialOutcome::Invalid(
            SourceDocumentRebaseInvalidReason::PinExpired,
        ));
    }
    let pinned_document_id = SourceDocumentId::from_str(&pin.document_id)?;
    if pinned_document_id != input.document_ref.document_id {
        return Ok(SourceDocumentRebaseMaterialOutcome::Invalid(
            SourceDocumentRebaseInvalidReason::PinOwnerMismatch,
        ));
    }
    let pinned_revision = from_i64(pin.document_revision, "pinned document revision")?;
    if pinned_revision != input.document_ref.document_revision {
        return Ok(SourceDocumentRebaseMaterialOutcome::Invalid(
            SourceDocumentRebaseInvalidReason::PinRevisionMismatch,
        ));
    }
    let Some(current_row) = document_row(conn, input.document_ref.document_id)? else {
        return Ok(SourceDocumentRebaseMaterialOutcome::NotFound);
    };
    let actual_revision = from_i64(current_row.current_document_revision, "document revision")?;
    if actual_revision != input.current_revision {
        return Ok(SourceDocumentRebaseMaterialOutcome::Conflict {
            expected_revision: input.current_revision,
            actual_revision,
            current: masked_document_from_row(conn, artifacts, current_row)?,
        });
    }
    let base = load_pinned_source_document_material(conn, artifacts, &pin)?;
    let current = load_source_document_material_sync(
        conn,
        artifacts,
        DocumentRef {
            document_id: input.document_ref.document_id,
            document_revision: input.current_revision,
        },
    )?
    .ok_or(StorageError::DocumentMissing)?;
    Ok(SourceDocumentRebaseMaterialOutcome::Ready(Box::new(
        SourceDocumentRebaseMaterials::new(base, current),
    )))
}

/// 在 single writer 中重新验证 pin/current 后原子 merge 或 fork。
pub(crate) fn process_rebase_source_document(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    input: RebaseSourceDocumentInput,
) -> Result<DocumentMutationOutcome, StorageError> {
    let RebaseSourceDocumentInput {
        pin_id,
        document_id,
        base_revision,
        current_revision,
        mode,
        revision,
        saved_at_ms,
    } = input;
    if let Some(reason) = validate_active_revision_pin(
        conn,
        pin_id,
        DocumentRef {
            document_id,
            document_revision: base_revision,
        },
        saved_at_ms,
    )? {
        return Ok(rebase_pin_invalid_outcome(reason));
    }
    let Some(current_row) = document_row(conn, document_id)? else {
        return Ok(DocumentMutationOutcome::NotFound);
    };
    let actual_revision = from_i64(current_row.current_document_revision, "document revision")?;
    if actual_revision != current_revision {
        return Ok(DocumentMutationOutcome::Conflict {
            expected_revision: current_revision,
            actual_revision,
            current: masked_document_from_row(conn, artifacts, current_row)?,
        });
    }
    match mode {
        SourceDocumentRebaseCommitMode::Merge => process_save_source_document(
            conn,
            artifacts,
            SaveSourceDocumentInput {
                document_id,
                expected_revision: current_revision,
                revision,
                saved_at_ms,
            },
        ),
        SourceDocumentRebaseCommitMode::Fork {
            document_id: fork_document_id,
            title,
        } => process_create_source_document(
            conn,
            artifacts,
            CreateSourceDocumentInput {
                document_id: fork_document_id,
                title,
                revision,
                created_at_ms: saved_at_ms,
            },
        ),
    }
}

/// 创建 revision 1 draft；limits/issues/revision ownership 在任何 artifact 写入前检查。
pub(crate) fn process_create_source_document(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    input: CreateSourceDocumentInput,
) -> Result<DocumentMutationOutcome, StorageError> {
    if let Some(existing) = document_row(conn, input.document_id)? {
        let current = masked_document_from_row(conn, artifacts, existing)?;
        return Ok(DocumentMutationOutcome::Conflict {
            expected_revision: 0,
            actual_revision: current.summary.revision,
            current,
        });
    }
    let issues = validate_revision_input(&input.revision, input.document_id, 1, &input.title);
    if !issues.is_empty() {
        return Ok(DocumentMutationOutcome::Invalid { issues });
    }
    let prepared = match prepare_revision(conn, artifacts, input.revision, input.created_at_ms) {
        Ok(prepared) => prepared,
        Err(error) => return map_document_secret_error(error),
    };
    let event = EventDraft {
        stream_id: document_stream_id(input.document_id),
        expected_version: 0,
        event_id: input.document_id.as_uuid(),
        event_type: EventType::Other("source_document".to_string()),
        schema_version: DOCUMENT_EVENT_SCHEMA_VERSION,
        correlation_id: None,
        causation_id: None,
        trace_id: "source-document/create".to_string(),
        occurred_at_ms: input.created_at_ms,
        payload: serde_json::json!({
            "kind": "created",
            "document_id": input.document_id,
            "document_revision": 1,
            "format": prepared.format,
            "state": SourceDocumentState::Draft,
            "masked_hash": &prepared.masked_hash,
            "schema_version": SOURCE_DOCUMENT_SCHEMA_VERSION,
        }),
        source_identity: None,
    };
    let document_id = input.document_id;
    let title = input.title;
    let created_at_ms = input.created_at_ms;
    append_event_transaction(conn, &event, &[], |conn, _global_seq, _| {
        retain_prepared_revision(conn, &prepared, document_id, 1, created_at_ms)?;
        sql_query(
            "INSERT INTO source_document_projection (document_id, format, title, state, source_identity, installed_revision, current_document_revision, masked_secret_id, raw_secret_id, manifest_secret_id, masked_hash, credential_slot_count, schema_version, created_at_ms, updated_at_ms) VALUES (?, ?, ?, 'draft', NULL, NULL, 1, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(document_id.to_string())
        .bind::<Text, _>(format_as_db(prepared.format))
        .bind::<Text, _>(&title)
        .bind::<Text, _>(prepared.masked.secret_id.to_string())
        .bind::<Text, _>(prepared.raw.secret_id.to_string())
        .bind::<Text, _>(prepared.manifest.secret_id.to_string())
        .bind::<Text, _>(&prepared.masked_hash)
        .bind::<BigInt, _>(usize_to_i64(prepared.slots.len())?)
        .bind::<BigInt, _>(i64::from(SOURCE_DOCUMENT_SCHEMA_VERSION))
        .bind::<BigInt, _>(created_at_ms)
        .bind::<BigInt, _>(created_at_ms)
        .execute(conn)
        .map_err(database_error)?;
        insert_prepared_slot_rows(conn, &prepared, document_id, 1, created_at_ms)
    })?;
    Ok(DocumentMutationOutcome::Saved {
        document: Some(masked_document_from_prepared(
            MaskedDocumentMetadata {
                document_id,
                title,
                state: SourceDocumentState::Draft,
                source_identity: None,
                revision: 1,
                installed_revision: None,
                created_at_ms,
                updated_at_ms: created_at_ms,
            },
            &prepared,
        )?),
    })
}

/// 保存 `expected + 1` revision；冲突在写 artifact 前返回 current masked document。
pub(crate) fn process_save_source_document(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    input: SaveSourceDocumentInput,
) -> Result<DocumentMutationOutcome, StorageError> {
    let Some(current_row) = document_row(conn, input.document_id)? else {
        return Ok(DocumentMutationOutcome::NotFound);
    };
    let actual_revision = from_i64(current_row.current_document_revision, "document revision")?;
    if actual_revision != input.expected_revision {
        let current = masked_document_from_row(conn, artifacts, current_row)?;
        return Ok(DocumentMutationOutcome::Conflict {
            expected_revision: input.expected_revision,
            actual_revision,
            current,
        });
    }
    let next_revision = input
        .expected_revision
        .checked_add(1)
        .ok_or_else(|| StorageError::InvalidInput("document revision 溢出".to_string()))?;
    let issues = validate_revision_input(
        &input.revision,
        input.document_id,
        next_revision,
        &current_row.title,
    );
    if !issues.is_empty() {
        return Ok(DocumentMutationOutcome::Invalid { issues });
    }
    let prepared = match prepare_revision(conn, artifacts, input.revision, input.saved_at_ms) {
        Ok(prepared) => prepared,
        Err(error) => return map_document_secret_error(error),
    };
    let expected_event_version = stream_version(conn, &document_stream_id(input.document_id))?;
    let event = EventDraft {
        stream_id: document_stream_id(input.document_id),
        expected_version: expected_event_version,
        event_id: Uuid::new_v4(),
        event_type: EventType::Other("source_document".to_string()),
        schema_version: DOCUMENT_EVENT_SCHEMA_VERSION,
        correlation_id: None,
        causation_id: None,
        trace_id: "source-document/save".to_string(),
        occurred_at_ms: input.saved_at_ms,
        payload: serde_json::json!({
            "kind": "saved",
            "document_id": input.document_id,
            "document_revision": next_revision,
            "format": prepared.format,
            "state": SourceDocumentState::from_db(&current_row.state)?,
            "masked_hash": &prepared.masked_hash,
            "schema_version": SOURCE_DOCUMENT_SCHEMA_VERSION,
        }),
        source_identity: current_row.source_identity.clone(),
    };
    let old_ref = DocumentRef {
        document_id: input.document_id,
        document_revision: input.expected_revision,
    };
    let old_slots = document_slot_rows(conn, old_ref)?;
    append_event_transaction(conn, &event, &[], |conn, _global_seq, _| {
        retain_prepared_revision(
            conn,
            &prepared,
            input.document_id,
            next_revision,
            input.saved_at_ms,
        )?;
        let changed = sql_query(
            "UPDATE source_document_projection SET format = ?, current_document_revision = ?, masked_secret_id = ?, raw_secret_id = ?, manifest_secret_id = ?, masked_hash = ?, credential_slot_count = ?, schema_version = ?, updated_at_ms = ? WHERE document_id = ? AND current_document_revision = ?",
        )
        .bind::<Text, _>(format_as_db(prepared.format))
        .bind::<BigInt, _>(to_i64(next_revision)?)
        .bind::<Text, _>(prepared.masked.secret_id.to_string())
        .bind::<Text, _>(prepared.raw.secret_id.to_string())
        .bind::<Text, _>(prepared.manifest.secret_id.to_string())
        .bind::<Text, _>(&prepared.masked_hash)
        .bind::<BigInt, _>(usize_to_i64(prepared.slots.len())?)
        .bind::<BigInt, _>(i64::from(SOURCE_DOCUMENT_SCHEMA_VERSION))
        .bind::<BigInt, _>(input.saved_at_ms)
        .bind::<Text, _>(input.document_id.to_string())
        .bind::<BigInt, _>(to_i64(input.expected_revision)?)
        .execute(conn)
        .map_err(database_error)?;
        if changed != 1 {
            return Err(StorageError::VersionConflict {
                stream_id: document_stream_id(input.document_id),
                expected: input.expected_revision,
                actual: actual_revision,
            });
        }
        insert_prepared_slot_rows(
            conn,
            &prepared,
            input.document_id,
            next_revision,
            input.saved_at_ms,
        )?;
        release_revision_owners(conn, old_ref, &old_slots)?;
        sql_query(
            "DELETE FROM source_document_credential_slots WHERE document_id = ? AND document_revision = ?",
        )
        .bind::<Text, _>(input.document_id.to_string())
        .bind::<BigInt, _>(to_i64(input.expected_revision)?)
        .execute(conn)
        .map_err(database_error)?;
        Ok(())
    })?;
    Ok(DocumentMutationOutcome::Saved {
        document: Some(masked_document_from_prepared(
            MaskedDocumentMetadata {
                document_id: input.document_id,
                title: current_row.title,
                state: SourceDocumentState::from_db(&current_row.state)?,
                source_identity: current_row.source_identity,
                revision: next_revision,
                installed_revision: optional_u64(
                    current_row.installed_revision,
                    "installed revision",
                )?,
                created_at_ms: current_row.created_at_ms,
                updated_at_ms: input.saved_at_ms,
            },
            &prepared,
        )?),
    })
}

/// optimistic rename；正文 revision 不变，save 的 revision ownership 不被伪造。
pub(crate) fn process_rename_source_document(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    input: &RenameSourceDocumentInput,
) -> Result<DocumentMutationOutcome, StorageError> {
    let Some(row) = document_row(conn, input.document_id)? else {
        return Ok(DocumentMutationOutcome::NotFound);
    };
    let actual_revision = from_i64(row.current_document_revision, "document revision")?;
    if input.expected_revision != actual_revision {
        let current = masked_document_from_row(conn, artifacts, row)?;
        return Ok(DocumentMutationOutcome::Conflict {
            expected_revision: input.expected_revision,
            actual_revision,
            current,
        });
    }
    if input.title.trim().is_empty() || input.title.len() > TITLE_MAX_BYTES {
        return Ok(DocumentMutationOutcome::Invalid {
            issues: vec![DocumentValidationIssue::new(
                "document_title_invalid",
                "来源文档标题不能为空且不得超过 512 UTF-8 bytes",
            )],
        });
    }
    let expected_event_version = stream_version(conn, &document_stream_id(input.document_id))?;
    let event = EventDraft {
        stream_id: document_stream_id(input.document_id),
        expected_version: expected_event_version,
        event_id: Uuid::new_v4(),
        event_type: EventType::Other("source_document".to_string()),
        schema_version: DOCUMENT_EVENT_SCHEMA_VERSION,
        correlation_id: None,
        causation_id: None,
        trace_id: "source-document/rename".to_string(),
        occurred_at_ms: input.renamed_at_ms,
        payload: serde_json::json!({
            "kind": "renamed",
            "document_id": input.document_id,
            "document_revision": actual_revision,
            "schema_version": SOURCE_DOCUMENT_SCHEMA_VERSION,
        }),
        source_identity: row.source_identity.clone(),
    };
    append_event_transaction(conn, &event, &[], |conn, _, _| {
        let changed = sql_query(
            "UPDATE source_document_projection SET title = ?, updated_at_ms = ? WHERE document_id = ? AND current_document_revision = ?",
        )
        .bind::<Text, _>(&input.title)
        .bind::<BigInt, _>(input.renamed_at_ms)
        .bind::<Text, _>(input.document_id.to_string())
        .bind::<BigInt, _>(to_i64(input.expected_revision)?)
        .execute(conn)
        .map_err(database_error)?;
        if changed != 1 {
            return Err(StorageError::VersionConflict {
                stream_id: document_stream_id(input.document_id),
                expected: input.expected_revision,
                actual: actual_revision,
            });
        }
        Ok(())
    })?;
    let updated = document_row(conn, input.document_id)?.ok_or(StorageError::DocumentMissing)?;
    Ok(DocumentMutationOutcome::Saved {
        document: Some(masked_document_from_row(conn, artifacts, updated)?),
    })
}

/// 只删除未关联、未被 snapshot/candidate pin 的 draft，并释放 current owners。
pub(crate) fn process_delete_source_document(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    input: DeleteSourceDocumentInput,
) -> Result<DocumentMutationOutcome, StorageError> {
    let Some(row) = document_row(conn, input.document_id)? else {
        return Ok(DocumentMutationOutcome::NotFound);
    };
    let actual_revision = from_i64(row.current_document_revision, "document revision")?;
    if actual_revision != input.expected_revision {
        let current = masked_document_from_row(conn, artifacts, row)?;
        return Ok(DocumentMutationOutcome::Conflict {
            expected_revision: input.expected_revision,
            actual_revision,
            current,
        });
    }
    release_expired_source_document_revision_pins(conn, input.deleted_at_ms)?;
    let pinned = sql_query(
        "SELECT COUNT(*) AS value FROM source_document_snapshots WHERE document_id = ? UNION ALL SELECT COUNT(*) AS value FROM candidate_projection WHERE document_id = ? AND status = 'staged' UNION ALL SELECT COUNT(*) AS value FROM source_document_revision_pins WHERE document_id = ?",
    )
    .bind::<Text, _>(input.document_id.to_string())
    .bind::<Text, _>(input.document_id.to_string())
    .bind::<Text, _>(input.document_id.to_string())
    .load::<CountRow>(conn)
    .map_err(database_error)?
    .into_iter()
    .any(|count| count.value != 0);
    if row.state != SourceDocumentState::Draft.as_db()
        || row.source_identity.is_some()
        || row.installed_revision.is_some()
        || pinned
    {
        return Ok(DocumentMutationOutcome::Invalid {
            issues: vec![DocumentValidationIssue::new(
                "document_delete_unsafe",
                "已关联或仍被 candidate/snapshot/revision pin 固定的来源文档不能删除",
            )],
        });
    }
    let document_ref = DocumentRef {
        document_id: input.document_id,
        document_revision: actual_revision,
    };
    let slots = document_slot_rows(conn, document_ref)?;
    let expected_event_version = stream_version(conn, &document_stream_id(input.document_id))?;
    let event = EventDraft {
        stream_id: document_stream_id(input.document_id),
        expected_version: expected_event_version,
        event_id: Uuid::new_v4(),
        event_type: EventType::Other("source_document".to_string()),
        schema_version: DOCUMENT_EVENT_SCHEMA_VERSION,
        correlation_id: None,
        causation_id: None,
        trace_id: "source-document/delete".to_string(),
        occurred_at_ms: input.deleted_at_ms,
        payload: serde_json::json!({
            "kind": "deleted",
            "document_id": input.document_id,
            "document_revision": actual_revision,
            "schema_version": SOURCE_DOCUMENT_SCHEMA_VERSION,
        }),
        source_identity: None,
    };
    append_event_transaction(conn, &event, &[], |conn, _, _| {
        release_revision_owners(conn, document_ref, &slots)?;
        sql_query("DELETE FROM source_document_credential_slots WHERE document_id = ?")
            .bind::<Text, _>(input.document_id.to_string())
            .execute(conn)
            .map_err(database_error)?;
        let deleted = sql_query(
            "DELETE FROM source_document_projection WHERE document_id = ? AND current_document_revision = ? AND state = 'draft' AND source_identity IS NULL",
        )
        .bind::<Text, _>(input.document_id.to_string())
        .bind::<BigInt, _>(to_i64(input.expected_revision)?)
        .execute(conn)
        .map_err(database_error)?;
        if deleted != 1 {
            return Err(StorageError::DocumentDeleteUnsafe);
        }
        Ok(())
    })?;
    Ok(DocumentMutationOutcome::Saved { document: None })
}

/// 显式 reveal 单个 current revision slot。
pub(crate) fn reveal_source_document_credential_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    target: SourceDocumentCredentialTarget,
) -> Result<Option<RevealedSourceDocumentCredential>, StorageError> {
    let Some(row) = document_row(conn, target.document_id)? else {
        return Ok(None);
    };
    let actual_revision = from_i64(row.current_document_revision, "document revision")?;
    if target.document_revision != actual_revision {
        return Err(StorageError::CredentialOwnershipMismatch);
    }
    let slot = sql_query(
        "SELECT slot_id, path, name, secret_id FROM source_document_credential_slots WHERE document_id = ? AND document_revision = ? AND slot_id = ?",
    )
    .bind::<Text, _>(target.document_id.to_string())
    .bind::<BigInt, _>(to_i64(target.document_revision)?)
    .bind::<Text, _>(target.slot_id.to_string())
    .get_result::<DocumentSlotRow>(conn)
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::CredentialOwnershipMismatch)?;
    let value = read_utf8_secret(
        conn,
        artifacts,
        SecretArtifactId::from_str(&slot.secret_id)?,
        "document_slot",
        &slot_owner_id(target.document_id, target.document_revision, target.slot_id),
    )?;
    Ok(Some(RevealedSourceDocumentCredential::new(target, value)))
}

/// replace/clear 共用的 slot ownership + 下一 revision 原子保存。
pub(crate) fn process_edit_source_document_credential(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    input: EditSourceDocumentCredentialInput,
) -> Result<DocumentMutationOutcome, StorageError> {
    let Some(row) = document_row(conn, input.target.document_id)? else {
        return Ok(DocumentMutationOutcome::NotFound);
    };
    let actual_revision = from_i64(row.current_document_revision, "document revision")?;
    if input.target.document_revision != actual_revision {
        let current = masked_document_from_row(conn, artifacts, row)?;
        return Ok(DocumentMutationOutcome::Conflict {
            expected_revision: input.target.document_revision,
            actual_revision,
            current,
        });
    }
    let owns_slot = sql_query(
        "SELECT COUNT(*) AS value FROM source_document_credential_slots WHERE document_id = ? AND document_revision = ? AND slot_id = ?",
    )
    .bind::<Text, _>(input.target.document_id.to_string())
    .bind::<BigInt, _>(to_i64(input.target.document_revision)?)
    .bind::<Text, _>(input.target.slot_id.to_string())
    .get_result::<CountRow>(conn)
    .map_err(database_error)?
    .value
        == 1;
    if !owns_slot {
        return Ok(DocumentMutationOutcome::Invalid {
            issues: vec![DocumentValidationIssue::new(
                "credential_owner_mismatch",
                "credential slot 不属于当前 document revision",
            )],
        });
    }
    process_save_source_document(
        conn,
        artifacts,
        SaveSourceDocumentInput {
            document_id: input.target.document_id,
            expected_revision: input.target.document_revision,
            revision: input.next_revision,
            saved_at_ms: input.saved_at_ms,
        },
    )
}

fn prepare_revision(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    revision: SourceDocumentRevisionInput,
    created_at_ms: i64,
) -> Result<PreparedRevision, StorageError> {
    let manifest_bytes =
        serde_json::to_vec(&revision.manifest).map_err(|_| StorageError::Serialization)?;
    let key = ensure_vault_key(conn, artifacts, created_at_ms)?;
    let masked = artifacts.write_secret(&key.key_id, &key.key, revision.masked_text.as_bytes())?;
    let raw = artifacts.write_secret(&key.key_id, &key.key, revision.raw_text.as_bytes())?;
    let manifest = artifacts.write_secret(&key.key_id, &key.key, &manifest_bytes)?;
    let values = revision
        .credentials
        .iter()
        .map(|material| (material.slot_id(), material.expose_value()))
        .collect::<HashMap<_, _>>();
    let mut slots = Vec::with_capacity(revision.manifest.slots.len());
    for slot in &revision.manifest.slots {
        let value = values
            .get(&slot.slot_id)
            .ok_or(StorageError::CredentialOwnershipMismatch)?;
        slots.push(PreparedSlot {
            slot_id: slot.slot_id,
            path: slot.path.clone(),
            name: slot.name.clone(),
            secret: artifacts.write_secret(&key.key_id, &key.key, value.as_bytes())?,
        });
    }
    Ok(PreparedRevision {
        format: revision.format,
        masked_hash: blake3::hash(revision.masked_text.as_bytes())
            .to_hex()
            .to_string(),
        masked_text: revision.masked_text,
        masked,
        raw,
        manifest,
        slots,
    })
}

fn retain_prepared_revision(
    conn: &mut SqliteConnection,
    prepared: &PreparedRevision,
    document_id: SourceDocumentId,
    revision: u64,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    retain_pending_secret(
        conn,
        &prepared.masked,
        "document_masked",
        &revision_owner_id(document_id, revision, "masked"),
        created_at_ms,
    )?;
    retain_pending_secret(
        conn,
        &prepared.raw,
        "document_raw",
        &revision_owner_id(document_id, revision, "raw"),
        created_at_ms,
    )?;
    retain_pending_secret(
        conn,
        &prepared.manifest,
        "document_manifest",
        &revision_owner_id(document_id, revision, "manifest"),
        created_at_ms,
    )?;
    for slot in &prepared.slots {
        let owner_id = slot_owner_id(document_id, revision, slot.slot_id);
        retain_pending_secret(
            conn,
            &slot.secret,
            "document_slot",
            &owner_id,
            created_at_ms,
        )?;
    }
    Ok(())
}

fn insert_prepared_slot_rows(
    conn: &mut SqliteConnection,
    prepared: &PreparedRevision,
    document_id: SourceDocumentId,
    revision: u64,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    for slot in &prepared.slots {
        sql_query(
            "INSERT INTO source_document_credential_slots (document_id, document_revision, slot_id, path, name, secret_id, schema_version, created_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(document_id.to_string())
        .bind::<BigInt, _>(to_i64(revision)?)
        .bind::<Text, _>(slot.slot_id.to_string())
        .bind::<Text, _>(&slot.path)
        .bind::<Text, _>(&slot.name)
        .bind::<Text, _>(slot.secret.secret_id.to_string())
        .bind::<BigInt, _>(i64::from(CREDENTIAL_SCHEMA_VERSION))
        .bind::<BigInt, _>(created_at_ms)
        .execute(conn)
        .map_err(database_error)?;
    }
    Ok(())
}

fn release_revision_owners(
    conn: &mut SqliteConnection,
    document_ref: DocumentRef,
    slots: &[DocumentSlotRow],
) -> Result<(), StorageError> {
    release_secret_owner(
        conn,
        "document_masked",
        &revision_owner_id(
            document_ref.document_id,
            document_ref.document_revision,
            "masked",
        ),
    )?;
    release_secret_owner(
        conn,
        "document_raw",
        &revision_owner_id(
            document_ref.document_id,
            document_ref.document_revision,
            "raw",
        ),
    )?;
    release_secret_owner(
        conn,
        "document_manifest",
        &revision_owner_id(
            document_ref.document_id,
            document_ref.document_revision,
            "manifest",
        ),
    )?;
    for slot in slots {
        release_secret_owner(
            conn,
            "document_slot",
            &slot_owner_id(
                document_ref.document_id,
                document_ref.document_revision,
                parse_slot_id(&slot.slot_id)?,
            ),
        )?;
    }
    Ok(())
}

fn validate_active_revision_pin(
    conn: &mut SqliteConnection,
    pin_id: Uuid,
    document_ref: DocumentRef,
    now_ms: i64,
) -> Result<Option<SourceDocumentRebaseInvalidReason>, StorageError> {
    let Some(pin) = revision_pin_row(conn, pin_id)? else {
        return Ok(Some(SourceDocumentRebaseInvalidReason::PinNotFound));
    };
    if pin.expires_at_ms <= now_ms {
        process_release_source_document_revision_pin(conn, pin_id)?;
        return Ok(Some(SourceDocumentRebaseInvalidReason::PinExpired));
    }
    if SourceDocumentId::from_str(&pin.document_id)? != document_ref.document_id {
        return Ok(Some(SourceDocumentRebaseInvalidReason::PinOwnerMismatch));
    }
    if from_i64(pin.document_revision, "pinned document revision")?
        != document_ref.document_revision
    {
        return Ok(Some(SourceDocumentRebaseInvalidReason::PinRevisionMismatch));
    }
    Ok(None)
}

fn load_pinned_source_document_material(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    pin: &RevisionPinRow,
) -> Result<SourceDocumentMaterial, StorageError> {
    let schema_version =
        u32::try_from(pin.schema_version).map_err(|_| StorageError::ArtifactCorrupt)?;
    if schema_version != SOURCE_DOCUMENT_SCHEMA_VERSION {
        return Err(StorageError::ArtifactCorrupt);
    }
    let pin_id = parse_pin_id(&pin.pin_id)?;
    let document_id = SourceDocumentId::from_str(&pin.document_id)?;
    let document_revision = from_i64(pin.document_revision, "pinned document revision")?;
    let document_ref = DocumentRef {
        document_id,
        document_revision,
    };
    let format = format_from_db(&pin.format)?;
    let owner_id = pin_owner_id(pin_id);
    let masked_text = read_utf8_secret(
        conn,
        artifacts,
        SecretArtifactId::from_str(&pin.masked_secret_id)?,
        "document_revision_pin_masked",
        &owner_id,
    )?;
    if blake3::hash(masked_text.as_bytes()).to_hex().as_str() != pin.masked_hash {
        return Err(StorageError::ArtifactCorrupt);
    }
    let raw_text = read_utf8_secret(
        conn,
        artifacts,
        SecretArtifactId::from_str(&pin.raw_secret_id)?,
        "document_revision_pin_raw",
        &owner_id,
    )?;
    let manifest_bytes = read_owned_secret(
        conn,
        artifacts,
        SecretArtifactId::from_str(&pin.manifest_secret_id)?,
        "document_revision_pin_manifest",
        &owner_id,
    )?;
    let manifest: CredentialSlotManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| StorageError::ArtifactCorrupt)?;
    validate_manifest_target(&manifest, document_id, document_revision, format)
        .map_err(|_| StorageError::ArtifactCorrupt)?;
    let slot_rows = revision_pin_slot_rows(conn, pin_id)?;
    if slot_rows.len() != manifest.slots.len() {
        return Err(StorageError::ArtifactCorrupt);
    }
    let mut credentials = Vec::with_capacity(slot_rows.len());
    for slot in slot_rows {
        if u32::try_from(slot.schema_version).ok() != Some(CREDENTIAL_SCHEMA_VERSION) {
            return Err(StorageError::ArtifactCorrupt);
        }
        let slot_id = parse_slot_id(&slot.slot_id)?;
        let manifest_slot = manifest
            .slots
            .iter()
            .find(|candidate| candidate.slot_id == slot_id)
            .ok_or(StorageError::ArtifactCorrupt)?;
        if manifest_slot.path != slot.path || manifest_slot.name != slot.name {
            return Err(StorageError::ArtifactCorrupt);
        }
        let value = read_utf8_secret(
            conn,
            artifacts,
            SecretArtifactId::from_str(&slot.secret_id)?,
            "document_revision_pin_slot",
            &pin_slot_owner_id(pin_id, slot_id),
        )?;
        credentials.push(CredentialSlotMaterial::new(slot_id, value));
    }
    Ok(SourceDocumentMaterial::new(
        document_ref,
        format,
        masked_text,
        raw_text,
        manifest,
        credentials,
    ))
}

fn rebase_pin_invalid_outcome(
    reason: SourceDocumentRebaseInvalidReason,
) -> DocumentMutationOutcome {
    let code = match reason {
        SourceDocumentRebaseInvalidReason::PinNotFound => "revision_pin_not_found",
        SourceDocumentRebaseInvalidReason::PinExpired => "revision_pin_expired",
        SourceDocumentRebaseInvalidReason::PinOwnerMismatch => "revision_pin_owner_mismatch",
        SourceDocumentRebaseInvalidReason::PinRevisionMismatch => "revision_pin_revision_mismatch",
    };
    DocumentMutationOutcome::Invalid {
        issues: vec![DocumentValidationIssue::new(
            code,
            "来源文档 revision pin 无效或已失效",
        )],
    }
}

fn revision_pin_row(
    conn: &mut SqliteConnection,
    pin_id: Uuid,
) -> Result<Option<RevisionPinRow>, StorageError> {
    sql_query(
        "SELECT pin_id, document_id, document_revision, format, masked_secret_id, raw_secret_id, manifest_secret_id, masked_hash, schema_version, expires_at_ms FROM source_document_revision_pins WHERE pin_id = ?",
    )
    .bind::<Text, _>(pin_id.to_string())
    .get_result::<RevisionPinRow>(conn)
    .optional()
    .map_err(database_error)
}

fn revision_pin_slot_rows(
    conn: &mut SqliteConnection,
    pin_id: Uuid,
) -> Result<Vec<RevisionPinSlotRow>, StorageError> {
    sql_query(
        "SELECT slot_id, path, name, secret_id, schema_version FROM source_document_revision_pin_slots WHERE pin_id = ? ORDER BY path ASC, slot_id ASC",
    )
    .bind::<Text, _>(pin_id.to_string())
    .load::<RevisionPinSlotRow>(conn)
    .map_err(database_error)
}

fn parse_pin_id(value: &str) -> Result<Uuid, StorageError> {
    let parsed = Uuid::parse_str(value)
        .map_err(|_| StorageError::InvalidInput("revision pin ID 无效".to_string()))?;
    if parsed.hyphenated().to_string() != value {
        return Err(StorageError::InvalidInput(
            "revision pin ID 必须是 canonical UUID".to_string(),
        ));
    }
    Ok(parsed)
}

fn pin_owner_id(pin_id: Uuid) -> String {
    pin_id.hyphenated().to_string()
}

fn pin_slot_owner_id(pin_id: Uuid, slot_id: CredentialSlotId) -> String {
    format!("{pin_id}:{slot_id}")
}

fn validate_revision_input(
    revision: &SourceDocumentRevisionInput,
    document_id: SourceDocumentId,
    expected_revision: u64,
    title: &str,
) -> Vec<DocumentValidationIssue> {
    let mut issues = revision.issues.clone();
    if title.trim().is_empty() || title.len() > TITLE_MAX_BYTES {
        issues.push(DocumentValidationIssue::new(
            "document_title_invalid",
            "来源文档标题不能为空且不得超过 512 UTF-8 bytes",
        ));
    }
    for (code, text) in [
        ("document_masked_too_large", revision.masked_text.as_str()),
        ("document_raw_too_large", revision.raw_text.as_str()),
    ] {
        if text.len() > MAX_SOURCE_DOCUMENT_BYTES {
            issues.push(DocumentValidationIssue::new(
                code,
                "来源文档正文不得超过 2 MiB UTF-8 bytes",
            ));
        }
    }
    if expected_revision == 0 {
        issues.push(DocumentValidationIssue::new(
            "document_revision_invalid",
            "来源文档 revision 必须大于 0",
        ));
    }
    if validate_manifest_target(
        &revision.manifest,
        document_id,
        expected_revision,
        revision.format,
    )
    .is_err()
    {
        issues.push(DocumentValidationIssue::new(
            "credential_owner_mismatch",
            "credential manifest 与 document/revision/format 不匹配",
        ));
    }
    let manifest_ids = revision
        .manifest
        .slots
        .iter()
        .map(|slot| slot.slot_id)
        .collect::<HashSet<_>>();
    let material_ids = revision
        .credentials
        .iter()
        .map(crate::types::CredentialSlotMaterial::slot_id)
        .collect::<HashSet<_>>();
    if manifest_ids.len() != revision.manifest.slots.len()
        || material_ids.len() != revision.credentials.len()
        || manifest_ids != material_ids
    {
        issues.push(DocumentValidationIssue::new(
            "credential_slot_set_mismatch",
            "credential manifest 与 plaintext slot 集合必须一一对应",
        ));
    }
    issues
}

fn validate_manifest_target(
    manifest: &CredentialSlotManifest,
    document_id: SourceDocumentId,
    revision: u64,
    format: SourceDocumentFormat,
) -> Result<(), StorageError> {
    if manifest.schema_version != CREDENTIAL_SCHEMA_VERSION
        || manifest.target.format != format
        || manifest.target.document_id != document_id.to_string()
        || manifest.target.revision != revision
    {
        return Err(StorageError::CredentialOwnershipMismatch);
    }
    let mut slot_ids = HashSet::with_capacity(manifest.slots.len());
    let mut paths = HashSet::with_capacity(manifest.slots.len());
    for slot in &manifest.slots {
        if slot.schema_version != CREDENTIAL_SCHEMA_VERSION
            || slot.target != manifest.target
            || slot.path.is_empty()
            || slot.name.is_empty()
            || !slot_ids.insert(slot.slot_id)
            || !paths.insert(slot.path.as_str())
        {
            return Err(StorageError::CredentialOwnershipMismatch);
        }
    }
    Ok(())
}

fn masked_document_from_row(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    row: DocumentRow,
) -> Result<MaskedSourceDocument, StorageError> {
    let document_id = SourceDocumentId::from_str(&row.document_id)?;
    let revision = from_i64(row.current_document_revision, "document revision")?;
    let masked_text = read_utf8_secret(
        conn,
        artifacts,
        SecretArtifactId::from_str(&row.masked_secret_id)?,
        "document_masked",
        &revision_owner_id(document_id, revision, "masked"),
    )?;
    if blake3::hash(masked_text.as_bytes()).to_hex().as_str() != row.masked_hash {
        return Err(StorageError::ArtifactCorrupt);
    }
    let slots = document_slot_rows(
        conn,
        DocumentRef {
            document_id,
            document_revision: revision,
        },
    )?;
    let credential_slots = slots
        .into_iter()
        .map(|slot| {
            Ok(CredentialSlotSummary {
                slot_id: parse_slot_id(&slot.slot_id)?,
                path: slot.path,
                name: slot.name,
                has_value: true,
            })
        })
        .collect::<Result<Vec<_>, StorageError>>()?;
    let summary = summary_from_row(row)?;
    if usize::try_from(summary.credential_slot_count)
        .ok()
        .is_none_or(|expected| expected != credential_slots.len())
    {
        return Err(StorageError::ArtifactCorrupt);
    }
    Ok(MaskedSourceDocument {
        summary,
        masked_text,
        credential_slots,
    })
}

fn masked_document_from_prepared(
    metadata: MaskedDocumentMetadata,
    prepared: &PreparedRevision,
) -> Result<MaskedSourceDocument, StorageError> {
    Ok(MaskedSourceDocument {
        summary: SourceDocumentSummary {
            document_id: metadata.document_id,
            format: prepared.format,
            title: metadata.title,
            state: metadata.state,
            source_identity: metadata.source_identity,
            revision: metadata.revision,
            installed_revision: metadata.installed_revision,
            masked_hash: prepared.masked_hash.clone(),
            credential_slot_count: u32::try_from(prepared.slots.len()).map_err(|_| {
                StorageError::InvalidInput("credential slot 数量超出 u32".to_string())
            })?,
            schema_version: SOURCE_DOCUMENT_SCHEMA_VERSION,
            created_at_ms: metadata.created_at_ms,
            updated_at_ms: metadata.updated_at_ms,
        },
        masked_text: prepared.masked_text.clone(),
        credential_slots: prepared
            .slots
            .iter()
            .map(|slot| CredentialSlotSummary {
                slot_id: slot.slot_id,
                path: slot.path.clone(),
                name: slot.name.clone(),
                has_value: true,
            })
            .collect(),
    })
}

fn summary_from_row(row: DocumentRow) -> Result<SourceDocumentSummary, StorageError> {
    let schema_version = u32::try_from(row.schema_version)
        .map_err(|_| StorageError::InvalidInput("document schema version 无效".to_string()))?;
    if schema_version != SOURCE_DOCUMENT_SCHEMA_VERSION {
        return Err(StorageError::InvalidInput(
            "document schema version 不兼容".to_string(),
        ));
    }
    Ok(SourceDocumentSummary {
        document_id: SourceDocumentId::from_str(&row.document_id)?,
        format: format_from_db(&row.format)?,
        title: row.title,
        state: SourceDocumentState::from_db(&row.state)?,
        source_identity: row.source_identity,
        revision: from_i64(row.current_document_revision, "document revision")?,
        installed_revision: optional_u64(row.installed_revision, "installed revision")?,
        masked_hash: row.masked_hash,
        credential_slot_count: u32::try_from(row.credential_slot_count)
            .map_err(|_| StorageError::InvalidInput("credential slot count 无效".to_string()))?,
        schema_version,
        created_at_ms: row.created_at_ms,
        updated_at_ms: row.updated_at_ms,
    })
}

fn document_row(
    conn: &mut SqliteConnection,
    document_id: SourceDocumentId,
) -> Result<Option<DocumentRow>, StorageError> {
    sql_query(
        "SELECT document_id, format, title, state, source_identity, installed_revision, current_document_revision, masked_secret_id, raw_secret_id, manifest_secret_id, masked_hash, credential_slot_count, schema_version, created_at_ms, updated_at_ms FROM source_document_projection WHERE document_id = ?",
    )
    .bind::<Text, _>(document_id.to_string())
    .get_result::<DocumentRow>(conn)
    .optional()
    .map_err(database_error)
}

fn document_slot_rows(
    conn: &mut SqliteConnection,
    document_ref: DocumentRef,
) -> Result<Vec<DocumentSlotRow>, StorageError> {
    sql_query(
        "SELECT slot_id, path, name, secret_id FROM source_document_credential_slots WHERE document_id = ? AND document_revision = ? ORDER BY path ASC, slot_id ASC",
    )
    .bind::<Text, _>(document_ref.document_id.to_string())
    .bind::<BigInt, _>(to_i64(document_ref.document_revision)?)
    .load::<DocumentSlotRow>(conn)
    .map_err(database_error)
}

fn read_utf8_secret(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    secret_id: SecretArtifactId,
    owner_kind: &str,
    owner_id: &str,
) -> Result<String, StorageError> {
    String::from_utf8(read_owned_secret(
        conn, artifacts, secret_id, owner_kind, owner_id,
    )?)
    .map_err(|_| StorageError::ArtifactCorrupt)
}

fn map_document_secret_error(error: StorageError) -> Result<DocumentMutationOutcome, StorageError> {
    match error {
        StorageError::KeyringLocked => Ok(DocumentMutationOutcome::Locked),
        StorageError::KeyringUnavailable => Ok(DocumentMutationOutcome::KeyUnavailable),
        StorageError::KeyLost | StorageError::MasterKeyUnavailable => {
            Ok(DocumentMutationOutcome::KeyLost)
        }
        StorageError::ArtifactCorrupt
        | StorageError::ArtifactUnavailable(_)
        | StorageError::SecretUnavailable => Ok(DocumentMutationOutcome::Corrupt),
        other => Err(other),
    }
}

pub(crate) fn format_as_db(format: SourceDocumentFormat) -> &'static str {
    match format {
        SourceDocumentFormat::Legado => "legado",
        SourceDocumentFormat::Maccms10Endpoint => "maccms10_endpoint",
    }
}

pub(crate) fn format_from_db(value: &str) -> Result<SourceDocumentFormat, StorageError> {
    match value {
        "legado" => Ok(SourceDocumentFormat::Legado),
        "maccms10_endpoint" => Ok(SourceDocumentFormat::Maccms10Endpoint),
        _ => Err(StorageError::InvalidInput("未知来源文档格式".to_string())),
    }
}

fn parse_slot_id(value: &str) -> Result<CredentialSlotId, StorageError> {
    let parsed = Uuid::parse_str(value)
        .map_err(|_| StorageError::InvalidInput("credential slot ID 无效".to_string()))?;
    if parsed.hyphenated().to_string() != value {
        return Err(StorageError::InvalidInput(
            "credential slot ID 必须是 canonical UUID".to_string(),
        ));
    }
    serde_json::from_value(serde_json::Value::String(value.to_string()))
        .map_err(|_| StorageError::InvalidInput("credential slot ID 无效".to_string()))
}

fn optional_u64(value: Option<i64>, field: &str) -> Result<Option<u64>, StorageError> {
    value.map(|value| from_i64(value, field)).transpose()
}

fn usize_to_i64(value: usize) -> Result<i64, StorageError> {
    i64::try_from(value)
        .map_err(|_| StorageError::InvalidInput("集合大小超出 SQLite 范围".to_string()))
}

pub(crate) fn revision_owner_id(
    document_id: SourceDocumentId,
    revision: u64,
    suffix: &str,
) -> String {
    format!("{document_id}:{revision}:{suffix}")
}

fn slot_owner_id(
    document_id: SourceDocumentId,
    revision: u64,
    slot_id: CredentialSlotId,
) -> String {
    format!("{document_id}:{revision}:{slot_id}")
}

pub(crate) fn document_stream_id(document_id: SourceDocumentId) -> String {
    format!("source-document/{document_id}")
}

#[derive(QueryableByName)]
struct DocumentRow {
    #[diesel(sql_type = Text)]
    document_id: String,
    #[diesel(sql_type = Text)]
    format: String,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    state: String,
    #[diesel(sql_type = Nullable<Text>)]
    source_identity: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    installed_revision: Option<i64>,
    #[diesel(sql_type = BigInt)]
    current_document_revision: i64,
    #[diesel(sql_type = Text)]
    masked_secret_id: String,
    #[diesel(sql_type = Text)]
    raw_secret_id: String,
    #[diesel(sql_type = Text)]
    manifest_secret_id: String,
    #[diesel(sql_type = Text)]
    masked_hash: String,
    #[diesel(sql_type = BigInt)]
    credential_slot_count: i64,
    #[diesel(sql_type = BigInt)]
    schema_version: i64,
    #[diesel(sql_type = BigInt)]
    created_at_ms: i64,
    #[diesel(sql_type = BigInt)]
    updated_at_ms: i64,
}

#[derive(QueryableByName)]
struct DocumentSlotRow {
    #[diesel(sql_type = Text)]
    slot_id: String,
    #[diesel(sql_type = Text)]
    path: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    secret_id: String,
}

#[derive(QueryableByName)]
struct RevisionPinRow {
    #[diesel(sql_type = Text)]
    pin_id: String,
    #[diesel(sql_type = Text)]
    document_id: String,
    #[diesel(sql_type = BigInt)]
    document_revision: i64,
    #[diesel(sql_type = Text)]
    format: String,
    #[diesel(sql_type = Text)]
    masked_secret_id: String,
    #[diesel(sql_type = Text)]
    raw_secret_id: String,
    #[diesel(sql_type = Text)]
    manifest_secret_id: String,
    #[diesel(sql_type = Text)]
    masked_hash: String,
    #[diesel(sql_type = BigInt)]
    schema_version: i64,
    #[diesel(sql_type = BigInt)]
    expires_at_ms: i64,
}

#[derive(QueryableByName)]
struct RevisionPinSlotRow {
    #[diesel(sql_type = Text)]
    slot_id: String,
    #[diesel(sql_type = Text)]
    path: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    secret_id: String,
    #[diesel(sql_type = BigInt)]
    schema_version: i64,
}

#[derive(QueryableByName)]
struct PinIdRow {
    #[diesel(sql_type = Text)]
    pin_id: String,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}
