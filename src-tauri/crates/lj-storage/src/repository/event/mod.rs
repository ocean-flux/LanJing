//! Event aggregate repository：事务核心、artifact ref metadata 与序列工具。
//!
//! `append_event_transaction` 在 transaction service 传入的 transaction 上验证 expected stream
//! version，分配单调 global sequence，追加 envelope、认领 artifact refs，再调用
//! projection closure。它不自行 commit 或开启平行 transaction。artifact 文件必须已按
//! temp → fsync → rename durable 落盘；此处只认领 metadata/ref，DB 失败留下的孤儿由 recovery
//! sweeper 清理，绝不把半个 Event 或 projection 暴露给读取者。

use crate::database::OptionalResultExt;
use crate::database::statement;
use crate::database::{DatabaseSession, TransactionFuture};
use lj_rule_model::{ArtifactRef, EventType};
use uuid::Uuid;

use crate::artifact::{ArtifactStore, PendingArtifact};
use crate::types::{ArtifactInput, ArtifactKind, CommitReceipt, StorageError};

mod model;
pub(crate) use model::*;

pub(crate) const BODY_KIND: &str = "body";

/// 仅限 crate 内部写路径构造的 Event；外部调用者只提交稳定 DTO。
#[derive(Debug, Clone)]
pub(crate) struct EventDraft {
    pub(crate) stream_id: String,
    pub(crate) expected_version: u64,
    pub(crate) event_id: Uuid,
    pub(crate) event_type: EventType,
    pub(crate) schema_version: u32,
    pub(crate) correlation_id: Option<Uuid>,
    pub(crate) causation_id: Option<Uuid>,
    pub(crate) trace_id: String,
    pub(crate) occurred_at_ms: i64,
    pub(crate) payload: serde_json::Value,
    pub(crate) source_identity: Option<String>,
}

/// 已落盘、等待 Event transaction 认领或已有 metadata 的 artifact 引用。
#[derive(Debug, Clone)]
pub(crate) enum ArtifactLink {
    New(PendingArtifact),
    Existing { hash: String, kind: ArtifactKind },
}

/// 避免同一个 Event 因多个字段指向同一 artifact 而重复增加 ref count。
pub(crate) fn push_new_artifact_link(links: &mut Vec<ArtifactLink>, pending: PendingArtifact) {
    if links.iter().any(|link| match link {
        ArtifactLink::New(existing) => {
            existing.hash == pending.hash && existing.kind == pending.kind
        }
        ArtifactLink::Existing { hash, kind } => *hash == pending.hash && *kind == pending.kind,
    }) {
        return;
    }
    links.push(ArtifactLink::New(pending));
}

#[derive(Debug, Clone)]
struct ArtifactDescriptor {
    hash: String,
    kind: ArtifactKind,
    codec: String,
}

/// 追加不影响投影的领域 Event，供 writer 的通用 append command 使用。
pub(crate) async fn process_append(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    request: crate::types::AppendRequest,
) -> Result<CommitReceipt, StorageError> {
    let draft = EventDraft {
        stream_id: request.stream_id,
        expected_version: request.expected_version,
        event_id: request.event_id,
        event_type: request.event_type,
        schema_version: request.schema_version,
        correlation_id: request.correlation_id,
        causation_id: request.causation_id,
        trace_id: request.trace_id,
        occurred_at_ms: request.occurred_at_ms,
        payload: request.payload,
        source_identity: request.source_id,
    };
    if let Some(receipt) = idempotent_event(conn, &draft).await? {
        ensure_idempotent_artifacts(conn, artifacts, &request.artifacts, &receipt).await?;
        return Ok(receipt);
    }
    let links = write_inputs(artifacts, request.artifacts)?;
    append_event_transaction(
        conn,
        &draft,
        &links,
        |_conn, _global_seq, _stream_version| Box::pin(async { Ok(()) }),
    )
    .await
}

/// 在传入 transaction 上写入 Event、sequence、artifact refs 与 projection。
pub(crate) async fn append_event_transaction<F>(
    conn: &mut DatabaseSession,
    draft: &EventDraft,
    links: &[ArtifactLink],
    projection: F,
) -> Result<CommitReceipt, StorageError>
where
    F: for<'a> FnOnce(&'a mut DatabaseSession, u64, u64) -> TransactionFuture<'a, ()> + Send,
{
    let actual = stream_version(conn, &draft.stream_id).await?;
    if actual != draft.expected_version {
        return Err(StorageError::VersionConflict {
            stream_id: draft.stream_id.clone(),
            expected: draft.expected_version,
            actual,
        });
    }
    let stream_version = actual.saturating_add(1);
    let global_seq = next_global_seq(conn).await?;
    let descriptors = describe_links(conn, links).await?;
    let (artifact_refs, secret_refs) = refs_from_descriptors(&descriptors);
    let event_type = serialize(&draft.event_type)?;
    let payload_json = serialize(&draft.payload)?;
    let artifact_refs_json = serialize(&artifact_refs)?;
    let secret_refs_json = serialize(&secret_refs)?;
    statement(
        "INSERT INTO events (global_seq, stream_id, stream_version, event_id, source_identity, event_type, schema_version, correlation_id, causation_id, trace_id, occurred_at_ms, payload_json, artifact_refs_json, secret_refs_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(to_i64(global_seq)?)
    .bind(&draft.stream_id)
    .bind(to_i64(stream_version)?)
    .bind(draft.event_id.to_string())
    .bind(draft.source_identity.as_deref())
    .bind(&event_type)
    .bind(i32::try_from(draft.schema_version).map_err(|_| {
        StorageError::InvalidInput("schema version 超出 i32".to_string())
    })?)
    .bind(draft.correlation_id.map(|value| value.to_string()).as_deref())
    .bind(draft.causation_id.map(|value| value.to_string()).as_deref())
    .bind(&draft.trace_id)
    .bind(draft.occurred_at_ms)
    .bind(&payload_json)
    .bind(&artifact_refs_json)
    .bind(&secret_refs_json)
    .execute(conn)
    .await
    .map_err(database_error)?;
    statement(
        "INSERT INTO event_streams (stream_id, version) VALUES (?, ?) ON CONFLICT(stream_id) DO UPDATE SET version = excluded.version",
    )
    .bind(&draft.stream_id)
    .bind(to_i64(stream_version)?)
    .execute(conn)
    .await
    .map_err(database_error)?;
    attach_artifacts(conn, global_seq, links, &descriptors, draft.occurred_at_ms).await?;
    projection(conn, global_seq, stream_version).await?;
    Ok(CommitReceipt {
        global_seq,
        stream_id: draft.stream_id.clone(),
        stream_version,
        artifact_refs,
        secret_refs,
    })
}

/// 只接受 envelope 内容完全一致的 event ID 重试；expected version 不参与重复判定。
pub(crate) async fn idempotent_event(
    conn: &mut DatabaseSession,
    draft: &EventDraft,
) -> Result<Option<CommitReceipt>, StorageError> {
    let Some(row) = event_by_id(conn, draft.event_id).await? else {
        return Ok(None);
    };
    let event_type = serialize(&draft.event_type)?;
    let payload = serialize(&draft.payload)?;
    let schema_version = i32::try_from(draft.schema_version)
        .map_err(|_| StorageError::InvalidInput("schema version 超出 i32".to_string()))?;
    let correlation_id = draft.correlation_id.map(|value| value.to_string());
    let causation_id = draft.causation_id.map(|value| value.to_string());
    if row.stream_id != draft.stream_id
        || row.source_identity != draft.source_identity
        || row.event_type != event_type
        || row.schema_version != schema_version
        || row.correlation_id != correlation_id
        || row.causation_id != causation_id
        || row.trace_id != draft.trace_id
        || row.occurred_at_ms != draft.occurred_at_ms
        || row.payload_json != payload
    {
        return Err(StorageError::IdempotencyMismatch);
    }
    Ok(Some(event_receipt_from_row(&row)?))
}

async fn ensure_idempotent_artifacts(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    inputs: &[ArtifactInput],
    receipt: &CommitReceipt,
) -> Result<(), StorageError> {
    let mut expected = std::collections::HashSet::new();
    for input in inputs {
        expected.insert(blake3::hash(&input.bytes).to_hex().to_string());
    }
    let actual = receipt
        .artifact_refs
        .iter()
        .map(|reference| reference.hash.clone())
        .collect::<std::collections::HashSet<_>>();
    if expected != actual {
        return Err(StorageError::IdempotencyMismatch);
    }
    for hash in actual {
        let row = artifact_row(conn, &hash, ArtifactKind::Body)
            .await?
            .ok_or_else(|| StorageError::ArtifactUnavailable(hash.clone()))?;
        artifacts.read_body(&hash, &row.relative_path)?;
    }
    Ok(())
}

fn write_inputs(
    artifacts: &ArtifactStore,
    inputs: Vec<ArtifactInput>,
) -> Result<Vec<ArtifactLink>, StorageError> {
    let mut links = Vec::with_capacity(inputs.len());
    for input in inputs {
        let pending = artifacts.write(ArtifactKind::Body, &input.bytes)?;
        push_new_artifact_link(&mut links, pending);
    }
    Ok(links)
}

async fn describe_links(
    conn: &mut DatabaseSession,
    links: &[ArtifactLink],
) -> Result<Vec<ArtifactDescriptor>, StorageError> {
    let mut descriptors = Vec::with_capacity(links.len());
    for link in links {
        descriptors.push(describe_link(conn, link).await?);
    }
    Ok(descriptors)
}

async fn describe_link(
    conn: &mut DatabaseSession,
    link: &ArtifactLink,
) -> Result<ArtifactDescriptor, StorageError> {
    match link {
        ArtifactLink::New(pending) => Ok(ArtifactDescriptor {
            hash: pending.hash.clone(),
            kind: pending.kind,
            codec: pending.codec.clone(),
        }),
        ArtifactLink::Existing { hash, kind } => {
            let row = artifact_row(conn, hash, *kind)
                .await?
                .ok_or_else(|| StorageError::ArtifactUnavailable(hash.clone()))?;
            Ok(ArtifactDescriptor {
                hash: row.hash,
                kind: *kind,
                codec: row.codec,
            })
        }
    }
}

fn refs_from_descriptors(
    descriptors: &[ArtifactDescriptor],
) -> (Vec<ArtifactRef>, Vec<lj_rule_model::SecretRef>) {
    let bodies = descriptors
        .iter()
        .map(|descriptor| ArtifactRef {
            hash: descriptor.hash.clone(),
            codec: descriptor.codec.clone(),
        })
        .collect();
    (bodies, Vec::new())
}

async fn attach_artifacts(
    conn: &mut DatabaseSession,
    global_seq: u64,
    links: &[ArtifactLink],
    descriptors: &[ArtifactDescriptor],
    created_at_ms: i64,
) -> Result<(), StorageError> {
    for (link, descriptor) in links.iter().zip(descriptors) {
        let kind = artifact_kind_db(descriptor.kind);
        match link {
            ArtifactLink::New(pending) => {
                statement(
                    "INSERT INTO artifact_metadata (hash, artifact_kind, codec, hash_algorithm, relative_path, stored_bytes, ref_count, created_at_ms) VALUES (?, ?, ?, 'blake3', ?, ?, 1, ?) ON CONFLICT(hash, artifact_kind) DO UPDATE SET ref_count = artifact_metadata.ref_count + 1",
                )
                .bind(&pending.hash)
                .bind(kind)
                .bind(&pending.codec)
                .bind(&pending.relative_path)
                .bind(to_i64(pending.stored_bytes)?)
                .bind(created_at_ms)
                .execute(conn).await
                .map_err(database_error)?;
            }
            ArtifactLink::Existing { hash, .. } => {
                let changed = statement(
                    "UPDATE artifact_metadata SET ref_count = ref_count + 1 WHERE hash = ? AND artifact_kind = ?",
                )
                .bind(hash)
                .bind(kind)
                .execute(conn).await
                .map_err(database_error)?;
                if changed != 1 {
                    return Err(StorageError::ArtifactUnavailable(hash.clone()));
                }
            }
        }
        statement(
            "INSERT INTO event_artifact_refs (global_seq, hash, artifact_kind) VALUES (?, ?, ?)",
        )
        .bind(to_i64(global_seq)?)
        .bind(&descriptor.hash)
        .bind(kind)
        .execute(conn)
        .await
        .map_err(database_error)?;
    }
    Ok(())
}

pub(crate) async fn retain_pending_artifact(
    conn: &mut DatabaseSession,
    pending: &PendingArtifact,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    statement(
        "INSERT INTO artifact_metadata (hash, artifact_kind, codec, hash_algorithm, relative_path, stored_bytes, ref_count, created_at_ms) VALUES (?, ?, ?, 'blake3', ?, ?, 1, ?) ON CONFLICT(hash, artifact_kind) DO UPDATE SET ref_count = artifact_metadata.ref_count + 1",
    )
    .bind(&pending.hash)
    .bind(artifact_kind_db(pending.kind))
    .bind(&pending.codec)
    .bind(&pending.relative_path)
    .bind(to_i64(pending.stored_bytes)?)
    .bind(created_at_ms)
    .execute(conn).await
    .map_err(database_error)?;
    Ok(())
}

pub(crate) async fn artifact_row(
    conn: &mut DatabaseSession,
    hash: &str,
    kind: ArtifactKind,
) -> Result<Option<ArtifactRow>, StorageError> {
    statement(
        "SELECT hash, codec, relative_path FROM artifact_metadata WHERE hash = ? AND artifact_kind = ?",
    )
    .bind(hash)
    .bind(artifact_kind_db(kind))
    .get_result::<ArtifactRow>(conn).await
    .optional()
    .map_err(database_error)
}

pub(crate) async fn read_body_by_hash(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    hash: &str,
) -> Result<Vec<u8>, StorageError> {
    let row = artifact_row(conn, hash, ArtifactKind::Body)
        .await?
        .ok_or_else(|| StorageError::ArtifactUnavailable(hash.to_string()))?;
    artifacts.read_body(hash, &row.relative_path)
}

/// 删除 candidate stream 的 refs 与事件；过期 staging 不会残留不可达 artifact metadata。
pub(crate) async fn remove_candidate_event_refs(
    conn: &mut DatabaseSession,
    candidate_id: Uuid,
) -> Result<(), StorageError> {
    let stream_id = crate::repository::candidate_source::candidate_stream_id(candidate_id);
    let rows = statement(
        "SELECT event_artifact_refs.hash, event_artifact_refs.artifact_kind FROM event_artifact_refs INNER JOIN events ON events.global_seq = event_artifact_refs.global_seq WHERE events.stream_id = ?",
    )
    .bind(&stream_id)
    .load::<ArtifactReferenceRow>(conn).await
    .map_err(database_error)?;
    for row in rows {
        decrement_artifact_ref(conn, &row.hash, &row.artifact_kind).await?;
    }
    statement("DELETE FROM event_artifact_refs WHERE global_seq IN (SELECT global_seq FROM events WHERE stream_id = ?)")
        .bind(&stream_id)
        .execute(conn).await
        .map_err(database_error)?;
    statement("DELETE FROM events WHERE stream_id = ?")
        .bind(&stream_id)
        .execute(conn)
        .await
        .map_err(database_error)?;
    Ok(())
}

pub(crate) async fn decrement_artifact_ref(
    conn: &mut DatabaseSession,
    hash: &str,
    kind: &str,
) -> Result<(), StorageError> {
    let changed = statement(
        "UPDATE artifact_metadata SET ref_count = ref_count - 1 WHERE hash = ? AND artifact_kind = ? AND ref_count > 0",
    )
    .bind(hash)
    .bind(kind)
    .execute(conn).await
    .map_err(database_error)?;
    if changed == 1 {
        Ok(())
    } else {
        Err(StorageError::ArtifactUnavailable(hash.to_string()))
    }
}

async fn next_global_seq(conn: &mut DatabaseSession) -> Result<u64, StorageError> {
    statement("UPDATE event_counters SET next_global_seq = next_global_seq + 1 WHERE id = 1")
        .execute(conn)
        .await
        .map_err(database_error)?;
    current_global_seq(conn).await
}

pub(crate) async fn current_global_seq(conn: &mut DatabaseSession) -> Result<u64, StorageError> {
    let value = statement("SELECT next_global_seq AS value FROM event_counters WHERE id = 1")
        .get_result::<I64Value>(conn)
        .await
        .map_err(database_error)?;
    from_i64(value.value, "global sequence")
}

pub(crate) async fn stream_version(
    conn: &mut DatabaseSession,
    stream_id: &str,
) -> Result<u64, StorageError> {
    let row = statement("SELECT version AS value FROM event_streams WHERE stream_id = ?")
        .bind(stream_id)
        .get_result::<I64Value>(conn)
        .await
        .optional()
        .map_err(database_error)?;
    row.map_or(Ok(0), |value| from_i64(value.value, "stream version"))
}

async fn event_by_id(
    conn: &mut DatabaseSession,
    event_id: Uuid,
) -> Result<Option<EventRow>, StorageError> {
    statement(
        "SELECT global_seq, stream_id, stream_version, event_id, source_identity, event_type, schema_version, correlation_id, causation_id, trace_id, occurred_at_ms, payload_json, artifact_refs_json, secret_refs_json FROM events WHERE event_id = ?",
    )
    .bind(event_id.to_string())
    .get_result::<EventRow>(conn).await
    .optional()
    .map_err(database_error)
}

fn event_receipt_from_row(row: &EventRow) -> Result<CommitReceipt, StorageError> {
    Ok(CommitReceipt {
        global_seq: from_i64(row.global_seq, "global sequence")?,
        stream_id: row.stream_id.clone(),
        stream_version: from_i64(row.stream_version, "stream version")?,
        artifact_refs: serde_json::from_str(&row.artifact_refs_json)
            .map_err(|_| StorageError::Serialization)?,
        secret_refs: serde_json::from_str(&row.secret_refs_json)
            .map_err(|_| StorageError::Serialization)?,
    })
}

fn artifact_kind_db(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Body => BODY_KIND,
    }
}
