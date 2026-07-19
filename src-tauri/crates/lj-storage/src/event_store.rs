//! Event Store 的事务核心、artifact ref metadata 与序列工具。
//!
//! `append_event_transaction` 是所有写路径唯一的 `SQLite` 原子边界：它先验证 expected stream
//! version，再分配单调 global sequence，追加 envelope、认领 artifact refs，并在同一
//! `BEGIN IMMEDIATE` transaction 调用 projection closure。artifact 文件必须已按
//! temp → fsync → rename durable 落盘；此处只认领 metadata/ref，DB 失败留下的孤儿由 recovery
//! sweeper 清理，绝不把半个 Event 或 projection 暴露给读取者。

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Integer, Nullable, Text};
use diesel::sqlite::SqliteConnection;
use lj_rule_model::{ArtifactRef, EventEnvelope, EventType, SecretRef};
use serde::Serialize;
use uuid::Uuid;

use crate::artifact::{ArtifactStore, PendingArtifact};
use crate::types::{ArtifactInput, ArtifactKind, CommitReceipt, StorageError, StoredEvent};

pub(crate) const BODY_KIND: &str = "body";
pub(crate) const SECRET_KIND: &str = "secret";
pub(crate) const SECRET_ALGORITHM: &str = "aes-256-gcm";

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
    encryption: Option<String>,
}

/// 追加不影响投影的领域 Event，供 writer 的通用 append command 使用。
pub(crate) fn process_append(
    conn: &mut SqliteConnection,
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
    if let Some(receipt) = idempotent_event(conn, &draft)? {
        ensure_idempotent_artifacts(conn, artifacts, &request.artifacts, &receipt)?;
        return Ok(receipt);
    }
    let links = write_inputs(artifacts, request.artifacts)?;
    append_event_transaction(
        conn,
        &draft,
        &links,
        |_conn, _global_seq, _stream_version| Ok(()),
    )
}

/// 提交 Event、stream/global sequence、artifact refs 与调用方 projection closure。
pub(crate) fn append_event_transaction<F>(
    conn: &mut SqliteConnection,
    draft: &EventDraft,
    links: &[ArtifactLink],
    projection: F,
) -> Result<CommitReceipt, StorageError>
where
    F: FnOnce(&mut SqliteConnection, u64, u64) -> Result<(), StorageError>,
{
    conn.immediate_transaction::<_, StorageError, _>(|conn| {
        let actual = stream_version(conn, &draft.stream_id)?;
        if actual != draft.expected_version {
            return Err(StorageError::VersionConflict {
                stream_id: draft.stream_id.clone(),
                expected: draft.expected_version,
                actual,
            });
        }
        let stream_version = actual.saturating_add(1);
        let global_seq = next_global_seq(conn)?;
        let descriptors = describe_links(conn, links)?;
        let (artifact_refs, secret_refs) = refs_from_descriptors(&descriptors);
        let event_type = serialize(&draft.event_type)?;
        let payload_json = serialize(&draft.payload)?;
        let artifact_refs_json = serialize(&artifact_refs)?;
        let secret_refs_json = serialize(&secret_refs)?;
        sql_query(
            "INSERT INTO events (global_seq, stream_id, stream_version, event_id, source_identity, event_type, schema_version, correlation_id, causation_id, trace_id, occurred_at_ms, payload_json, artifact_refs_json, secret_refs_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(to_i64(global_seq)?)
        .bind::<Text, _>(&draft.stream_id)
        .bind::<BigInt, _>(to_i64(stream_version)?)
        .bind::<Text, _>(draft.event_id.to_string())
        .bind::<Nullable<Text>, _>(draft.source_identity.as_deref())
        .bind::<Text, _>(&event_type)
        .bind::<Integer, _>(i32::try_from(draft.schema_version).map_err(|_| StorageError::InvalidInput("schema version 超出 i32".to_string()))?)
        .bind::<Nullable<Text>, _>(draft.correlation_id.map(|value| value.to_string()).as_deref())
        .bind::<Nullable<Text>, _>(draft.causation_id.map(|value| value.to_string()).as_deref())
        .bind::<Text, _>(&draft.trace_id)
        .bind::<BigInt, _>(draft.occurred_at_ms)
        .bind::<Text, _>(&payload_json)
        .bind::<Text, _>(&artifact_refs_json)
        .bind::<Text, _>(&secret_refs_json)
        .execute(conn)
        .map_err(database_error)?;
        sql_query(
            "INSERT INTO event_streams (stream_id, version) VALUES (?, ?) ON CONFLICT(stream_id) DO UPDATE SET version = excluded.version",
        )
        .bind::<Text, _>(&draft.stream_id)
        .bind::<BigInt, _>(to_i64(stream_version)?)
        .execute(conn)
        .map_err(database_error)?;
        attach_artifacts(conn, global_seq, links, &descriptors, draft.occurred_at_ms)?;
        projection(conn, global_seq, stream_version)?;
        Ok(CommitReceipt {
            global_seq,
            stream_id: draft.stream_id.clone(),
            stream_version,
            artifact_refs,
            secret_refs,
        })
    })
}

/// 只接受 envelope 内容完全一致的 event ID 重试；expected version 不参与重复判定。
pub(crate) fn idempotent_event(
    conn: &mut SqliteConnection,
    draft: &EventDraft,
) -> Result<Option<CommitReceipt>, StorageError> {
    let Some(row) = event_by_id(conn, draft.event_id)? else {
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

fn ensure_idempotent_artifacts(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    inputs: &[ArtifactInput],
    receipt: &CommitReceipt,
) -> Result<(), StorageError> {
    let mut expected = std::collections::HashSet::new();
    for input in inputs {
        expected.insert((input.kind, blake3::hash(&input.bytes).to_hex().to_string()));
    }
    let actual = receipt
        .artifact_refs
        .iter()
        .map(|reference| (ArtifactKind::Body, reference.hash.clone()))
        .chain(
            receipt
                .secret_refs
                .iter()
                .map(|reference| (ArtifactKind::Secret, reference.hash.clone())),
        )
        .collect::<std::collections::HashSet<_>>();
    if expected != actual {
        return Err(StorageError::IdempotencyMismatch);
    }
    for (kind, hash) in actual {
        let row = artifact_row(conn, &hash, kind)?
            .ok_or_else(|| StorageError::ArtifactUnavailable(hash.clone()))?;
        match kind {
            ArtifactKind::Body => {
                artifacts.read_body(&hash, &row.relative_path)?;
            }
            ArtifactKind::Secret => {
                artifacts.read_secret(&hash, &row.relative_path)?;
            }
        }
    }
    Ok(())
}

fn write_inputs(
    artifacts: &ArtifactStore,
    inputs: Vec<ArtifactInput>,
) -> Result<Vec<ArtifactLink>, StorageError> {
    let mut links = Vec::with_capacity(inputs.len());
    for input in inputs {
        let pending = artifacts.write(input.kind, &input.bytes)?;
        push_new_artifact_link(&mut links, pending);
    }
    Ok(links)
}

fn describe_links(
    conn: &mut SqliteConnection,
    links: &[ArtifactLink],
) -> Result<Vec<ArtifactDescriptor>, StorageError> {
    links.iter().map(|link| describe_link(conn, link)).collect()
}

fn describe_link(
    conn: &mut SqliteConnection,
    link: &ArtifactLink,
) -> Result<ArtifactDescriptor, StorageError> {
    match link {
        ArtifactLink::New(pending) => Ok(ArtifactDescriptor {
            hash: pending.hash.clone(),
            kind: pending.kind,
            codec: pending.codec.clone(),
            encryption: pending.encryption.clone(),
        }),
        ArtifactLink::Existing { hash, kind } => {
            let row = artifact_row(conn, hash, *kind)?
                .ok_or_else(|| StorageError::ArtifactUnavailable(hash.clone()))?;
            Ok(ArtifactDescriptor {
                hash: row.hash,
                kind: *kind,
                codec: row.codec,
                encryption: row.encryption,
            })
        }
    }
}

fn refs_from_descriptors(descriptors: &[ArtifactDescriptor]) -> (Vec<ArtifactRef>, Vec<SecretRef>) {
    let mut bodies = Vec::new();
    let mut secrets = Vec::new();
    for descriptor in descriptors {
        match descriptor.kind {
            ArtifactKind::Body => bodies.push(ArtifactRef {
                hash: descriptor.hash.clone(),
                codec: descriptor.codec.clone(),
            }),
            ArtifactKind::Secret => secrets.push(SecretRef {
                hash: descriptor.hash.clone(),
                algorithm: descriptor
                    .encryption
                    .clone()
                    .unwrap_or_else(|| SECRET_ALGORITHM.to_string()),
            }),
        }
    }
    (bodies, secrets)
}

fn attach_artifacts(
    conn: &mut SqliteConnection,
    global_seq: u64,
    links: &[ArtifactLink],
    descriptors: &[ArtifactDescriptor],
    created_at_ms: i64,
) -> Result<(), StorageError> {
    for (link, descriptor) in links.iter().zip(descriptors) {
        let kind = artifact_kind_db(descriptor.kind);
        match link {
            ArtifactLink::New(pending) => {
                sql_query(
                    "INSERT INTO artifact_metadata (hash, artifact_kind, codec, hash_algorithm, encryption, relative_path, stored_bytes, ref_count, created_at_ms) VALUES (?, ?, ?, 'blake3', ?, ?, ?, 1, ?) ON CONFLICT(hash, artifact_kind) DO UPDATE SET ref_count = artifact_metadata.ref_count + 1",
                )
                .bind::<Text, _>(&pending.hash)
                .bind::<Text, _>(kind)
                .bind::<Text, _>(&pending.codec)
                .bind::<Nullable<Text>, _>(pending.encryption.as_deref())
                .bind::<Text, _>(&pending.relative_path)
                .bind::<BigInt, _>(to_i64(pending.stored_bytes)?)
                .bind::<BigInt, _>(created_at_ms)
                .execute(conn)
                .map_err(database_error)?;
            }
            ArtifactLink::Existing { hash, .. } => {
                let changed = sql_query(
                    "UPDATE artifact_metadata SET ref_count = ref_count + 1 WHERE hash = ? AND artifact_kind = ?",
                )
                .bind::<Text, _>(hash)
                .bind::<Text, _>(kind)
                .execute(conn)
                .map_err(database_error)?;
                if changed != 1 {
                    return Err(StorageError::ArtifactUnavailable(hash.clone()));
                }
            }
        }
        sql_query(
            "INSERT INTO event_artifact_refs (global_seq, hash, artifact_kind) VALUES (?, ?, ?)",
        )
        .bind::<BigInt, _>(to_i64(global_seq)?)
        .bind::<Text, _>(&descriptor.hash)
        .bind::<Text, _>(kind)
        .execute(conn)
        .map_err(database_error)?;
    }
    Ok(())
}

pub(crate) fn retain_pending_artifact(
    conn: &mut SqliteConnection,
    pending: &PendingArtifact,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    sql_query(
        "INSERT INTO artifact_metadata (hash, artifact_kind, codec, hash_algorithm, encryption, relative_path, stored_bytes, ref_count, created_at_ms) VALUES (?, ?, ?, 'blake3', ?, ?, ?, 1, ?) ON CONFLICT(hash, artifact_kind) DO UPDATE SET ref_count = artifact_metadata.ref_count + 1",
    )
    .bind::<Text, _>(&pending.hash)
    .bind::<Text, _>(artifact_kind_db(pending.kind))
    .bind::<Text, _>(&pending.codec)
    .bind::<Nullable<Text>, _>(pending.encryption.as_deref())
    .bind::<Text, _>(&pending.relative_path)
    .bind::<BigInt, _>(to_i64(pending.stored_bytes)?)
    .bind::<BigInt, _>(created_at_ms)
    .execute(conn)
    .map_err(database_error)?;
    Ok(())
}

pub(crate) fn artifact_row(
    conn: &mut SqliteConnection,
    hash: &str,
    kind: ArtifactKind,
) -> Result<Option<ArtifactRow>, StorageError> {
    sql_query(
        "SELECT hash, codec, encryption, relative_path FROM artifact_metadata WHERE hash = ? AND artifact_kind = ?",
    )
    .bind::<Text, _>(hash)
    .bind::<Text, _>(artifact_kind_db(kind))
    .get_result::<ArtifactRow>(conn)
    .optional()
    .map_err(database_error)
}

pub(crate) fn read_body_by_hash(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    hash: &str,
) -> Result<Vec<u8>, StorageError> {
    let row = artifact_row(conn, hash, ArtifactKind::Body)?
        .ok_or_else(|| StorageError::ArtifactUnavailable(hash.to_string()))?;
    artifacts.read_body(hash, &row.relative_path)
}

/// 删除 candidate stream 的 refs 与事件；过期 staging 不会残留不可达 artifact metadata。
pub(crate) fn remove_candidate_event_refs(
    conn: &mut SqliteConnection,
    candidate_id: Uuid,
) -> Result<(), StorageError> {
    let stream_id = crate::candidate_install::candidate_stream_id(candidate_id);
    let rows = sql_query(
        "SELECT event_artifact_refs.hash, event_artifact_refs.artifact_kind FROM event_artifact_refs INNER JOIN events ON events.global_seq = event_artifact_refs.global_seq WHERE events.stream_id = ?",
    )
    .bind::<Text, _>(&stream_id)
    .load::<ArtifactReferenceRow>(conn)
    .map_err(database_error)?;
    for row in rows {
        decrement_artifact_ref(conn, &row.hash, &row.artifact_kind)?;
    }
    sql_query("DELETE FROM event_artifact_refs WHERE global_seq IN (SELECT global_seq FROM events WHERE stream_id = ?)")
        .bind::<Text, _>(&stream_id)
        .execute(conn)
        .map_err(database_error)?;
    sql_query("DELETE FROM events WHERE stream_id = ?")
        .bind::<Text, _>(&stream_id)
        .execute(conn)
        .map_err(database_error)?;
    Ok(())
}

pub(crate) fn decrement_artifact_ref(
    conn: &mut SqliteConnection,
    hash: &str,
    kind: &str,
) -> Result<(), StorageError> {
    let changed = sql_query(
        "UPDATE artifact_metadata SET ref_count = ref_count - 1 WHERE hash = ? AND artifact_kind = ? AND ref_count > 0",
    )
    .bind::<Text, _>(hash)
    .bind::<Text, _>(kind)
    .execute(conn)
    .map_err(database_error)?;
    if changed == 1 {
        Ok(())
    } else {
        Err(StorageError::ArtifactUnavailable(hash.to_string()))
    }
}

fn next_global_seq(conn: &mut SqliteConnection) -> Result<u64, StorageError> {
    sql_query("UPDATE event_counters SET next_global_seq = next_global_seq + 1 WHERE id = 1")
        .execute(conn)
        .map_err(database_error)?;
    current_global_seq(conn)
}

pub(crate) fn current_global_seq(conn: &mut SqliteConnection) -> Result<u64, StorageError> {
    let value = sql_query("SELECT next_global_seq AS value FROM event_counters WHERE id = 1")
        .get_result::<I64Value>(conn)
        .map_err(database_error)?;
    from_i64(value.value, "global sequence")
}

pub(crate) fn stream_version(
    conn: &mut SqliteConnection,
    stream_id: &str,
) -> Result<u64, StorageError> {
    let row = sql_query("SELECT version AS value FROM event_streams WHERE stream_id = ?")
        .bind::<Text, _>(stream_id)
        .get_result::<I64Value>(conn)
        .optional()
        .map_err(database_error)?;
    row.map_or(Ok(0), |value| from_i64(value.value, "stream version"))
}

fn event_by_id(
    conn: &mut SqliteConnection,
    event_id: Uuid,
) -> Result<Option<EventRow>, StorageError> {
    sql_query(
        "SELECT global_seq, stream_id, stream_version, event_id, source_identity, event_type, schema_version, correlation_id, causation_id, trace_id, occurred_at_ms, payload_json, artifact_refs_json, secret_refs_json FROM events WHERE event_id = ?",
    )
    .bind::<Text, _>(event_id.to_string())
    .get_result::<EventRow>(conn)
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

/// 将数据库行还原为安全 Event envelope；时间保持 epoch milliseconds 文本以维持既有 wire。
pub(crate) fn stored_event_from_row(row: EventRow) -> Result<StoredEvent, StorageError> {
    let event_type =
        serde_json::from_str(&row.event_type).map_err(|_| StorageError::Serialization)?;
    let payload =
        serde_json::from_str(&row.payload_json).map_err(|_| StorageError::Serialization)?;
    let artifact_refs =
        serde_json::from_str(&row.artifact_refs_json).map_err(|_| StorageError::Serialization)?;
    let secret_refs =
        serde_json::from_str(&row.secret_refs_json).map_err(|_| StorageError::Serialization)?;
    Ok(StoredEvent {
        source_identity: row.source_identity.clone(),
        envelope: EventEnvelope {
            global_seq: from_i64(row.global_seq, "global sequence")?,
            stream_id: row.stream_id,
            stream_version: from_i64(row.stream_version, "stream version")?,
            event_id: Uuid::parse_str(&row.event_id)
                .map_err(|_| StorageError::InvalidInput("损坏的 event ID".to_string()))?,
            event_type,
            schema_version: u32::try_from(row.schema_version)
                .map_err(|_| StorageError::InvalidInput("损坏的 schema version".to_string()))?,
            correlation_id: parse_optional_uuid(row.correlation_id)?,
            causation_id: parse_optional_uuid(row.causation_id)?,
            trace_id: row.trace_id,
            occurred_at: row.occurred_at_ms.to_string(),
            payload,
            artifact_refs,
            secret_refs,
        },
    })
}

pub(crate) fn ensure_blake3_hash(value: &str, name: &str) -> Result<(), StorageError> {
    if value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        Ok(())
    } else {
        Err(StorageError::InvalidInput(format!(
            "{name} 必须是 BLAKE3 hex"
        )))
    }
}

fn artifact_kind_db(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Body => BODY_KIND,
        ArtifactKind::Secret => SECRET_KIND,
    }
}

pub(crate) fn serialize<T: Serialize>(value: &T) -> Result<String, StorageError> {
    serde_json::to_string(value).map_err(|_| StorageError::Serialization)
}

pub(crate) fn deserialize<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, StorageError> {
    serde_json::from_slice(bytes).map_err(|_| StorageError::Serialization)
}

pub(crate) fn database_error(error: impl std::fmt::Display) -> StorageError {
    StorageError::Database(error.to_string())
}

impl From<diesel::result::Error> for StorageError {
    fn from(error: diesel::result::Error) -> Self {
        database_error(error)
    }
}

pub(crate) fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::InvalidInput("整数超出 SQLite 范围".to_string()))
}

pub(crate) fn from_i64(value: i64, field: &str) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::InvalidInput(format!("{field} 为负数")))
}

fn parse_optional_uuid(value: Option<String>) -> Result<Option<Uuid>, StorageError> {
    value
        .map(|value| {
            Uuid::parse_str(&value)
                .map_err(|_| StorageError::InvalidInput("损坏的 UUID".to_string()))
        })
        .transpose()
}

pub(crate) fn now_millis() -> i64 {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

#[derive(QueryableByName)]
pub(crate) struct I64Value {
    #[diesel(sql_type = BigInt)]
    pub(crate) value: i64,
}

#[derive(QueryableByName)]
pub(crate) struct TextValue {
    #[diesel(sql_type = Text)]
    pub(crate) value: String,
}

#[derive(QueryableByName)]
pub(crate) struct EventRow {
    #[diesel(sql_type = BigInt)]
    global_seq: i64,
    #[diesel(sql_type = Text)]
    stream_id: String,
    #[diesel(sql_type = BigInt)]
    stream_version: i64,
    #[diesel(sql_type = Text)]
    event_id: String,
    #[diesel(sql_type = Nullable<Text>)]
    source_identity: Option<String>,
    #[diesel(sql_type = Text)]
    event_type: String,
    #[diesel(sql_type = Integer)]
    schema_version: i32,
    #[diesel(sql_type = Nullable<Text>)]
    correlation_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    causation_id: Option<String>,
    #[diesel(sql_type = Text)]
    trace_id: String,
    #[diesel(sql_type = BigInt)]
    occurred_at_ms: i64,
    #[diesel(sql_type = Text)]
    payload_json: String,
    #[diesel(sql_type = Text)]
    artifact_refs_json: String,
    #[diesel(sql_type = Text)]
    secret_refs_json: String,
}

#[derive(QueryableByName)]
pub(crate) struct ArtifactPathRow {
    #[diesel(sql_type = Text)]
    pub(crate) relative_path: String,
}

#[derive(QueryableByName)]
pub(crate) struct ArtifactRow {
    #[diesel(sql_type = Text)]
    pub(crate) hash: String,
    #[diesel(sql_type = Text)]
    pub(crate) codec: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) encryption: Option<String>,
    #[diesel(sql_type = Text)]
    pub(crate) relative_path: String,
}

#[derive(QueryableByName)]
pub(crate) struct ArtifactReferenceRow {
    #[diesel(sql_type = Text)]
    pub(crate) hash: String,
    #[diesel(sql_type = Text)]
    pub(crate) artifact_kind: String,
}
