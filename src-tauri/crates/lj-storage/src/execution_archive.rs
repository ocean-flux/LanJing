//! Execution effect archive 的持久化与 replay 边界。
//!
//! 此模块只处理 runtime 已执行 effect 的输出、typed witness 与可选 HTTP request material。
//! **持久化不变量**：body/secret 文件先在 writer blocking lane durable 写入，随后同一
//! `BEGIN IMMEDIATE` transaction 追加 execution Event、artifact refs、global sequence 与
//! `effect_captures` 行；任何一步失败都不得返回 receipt。**加密不变量**：request body 一律
//! 是 AES-256-GCM `Secret` Artifact，Event 仅含 BLAKE3 ref/hash。**replay 不变量**：C2
//! 在返回 runtime 前验证 artifact、secret、typed witness 与 output 的完整性，绝不重新暴露原始
//! request material。

use async_trait::async_trait;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Nullable, Text};
use lj_rule_model::{EventType, SensitiveNamePolicy, canonical_json};
use lj_runtime::{
    ArchivedEffectCapture, DurableCaptureReceipt, EffectArchive, EffectArchiveError, EffectCapture,
    EffectCaptureMaterialSensitivity, EffectOutput, EffectReplayLookup, EffectWitness,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::artifact::{ArtifactStore, PendingSecretArtifact};
use crate::event_store::{
    EventDraft, append_event_transaction, database_error, deserialize, ensure_blake3_hash,
    now_millis, push_new_artifact_link, read_body_by_hash, serialize, stream_version, to_i64,
};
use crate::execution::{execution_stream_id, get_execution_sync, update_execution_revision};
use crate::secret_artifact::{read_owned_secret, retain_pending_secret, write_secret};
use crate::storage::EventProjectionStorage;
use crate::types::{ArtifactKind, SecretArtifactId, StorageError};

/// 将原始 request body 与 HTTP witness 的 BLAKE3/长度绑定。
///
/// 该校验发生在写任何 artifact 前，防止调用方以同一个 witness 偷换 live material。
fn request_body_hash(capture: &EffectCapture) -> Result<Option<&str>, StorageError> {
    let witness_body = match &capture.witness {
        EffectWitness::Http(witness) => witness.request.body.as_ref(),
        EffectWitness::QuickJs(_) | EffectWitness::Extract(_) => None,
    };
    match (capture.request_body(), witness_body) {
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => Err(StorageError::InvalidInput(
            "HTTP request body material 与 witness 不一致".to_string(),
        )),
        (Some(bytes), Some(witness)) => {
            let byte_len = u64::try_from(bytes.len()).map_err(|_| {
                StorageError::InvalidInput("HTTP request body 长度超出 u64".to_string())
            })?;
            let hash = blake3::hash(bytes).to_hex().to_string();
            if witness.hash != hash || witness.byte_len != byte_len {
                return Err(StorageError::InvalidInput(
                    "HTTP request body material 与 witness hash 不一致".to_string(),
                ));
            }
            Ok(Some(witness.hash.as_str()))
        }
    }
}

/// 以强制 secret 敏感性写入 HTTP request body。
///
/// 任何未来出现的非 secret material variant 都会被拒绝，而不是猜测请求体是否安全。
fn write_request_body_secret(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    capture: &EffectCapture,
    created_at_ms: i64,
) -> Result<Option<PendingSecretArtifact>, StorageError> {
    if request_body_hash(capture)?.is_none() {
        return Ok(None);
    }
    if capture.request_body_sensitivity() != Some(EffectCaptureMaterialSensitivity::Secret) {
        return Err(StorageError::InvalidInput(
            "HTTP request body 必须作为 Secret Artifact 持久化".to_string(),
        ));
    }
    let bytes = capture
        .request_body()
        .ok_or_else(|| StorageError::InvalidInput("HTTP request body material 缺失".to_string()))?;
    write_secret(conn, artifacts, bytes, created_at_ms).map(Some)
}

/// 在 writer blocking lane 内完成 effect archive 的文件与 Event/SQLite 原子认领。
///
/// 返回 receipt 时，output body、safe witness、可选 response header secret、可选 request body
/// secret 都已有 durable 文件并已被同一 Event transaction 建立引用。
pub(crate) fn persist_effect_capture(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    capture: EffectCapture,
) -> Result<DurableCaptureReceipt, StorageError> {
    capture.validate_replay_integrity().map_err(|_| {
        StorageError::InvalidInput("effect capture witness 不满足完整性合同".to_string())
    })?;
    ensure_blake3_hash(&capture.output_hash, "effect output hash")?;
    ensure_blake3_hash(&capture.witness_hash, "effect witness hash")?;
    let witness_payload = canonical_json(&StoredEffectWitness {
        witness: capture.witness.clone(),
    })
    .map_err(|_| StorageError::Serialization)?;
    let expected_witness_artifact_hash = blake3::hash(witness_payload.as_bytes())
        .to_hex()
        .to_string();
    let has_request_body = request_body_hash(&capture)?.is_some();
    if let Some(existing) = effect_capture_row(conn, capture.execution_id, capture.effect_id)? {
        if existing.fingerprint == capture.fingerprint
            && existing.output_hash == capture.output_hash
            && existing.witness_hash.as_deref() == Some(capture.witness_hash.as_str())
            && existing.witness_artifact_hash.as_deref()
                == Some(expected_witness_artifact_hash.as_str())
            && existing.request_body_secret_id.is_some() == has_request_body
        {
            ensure_existing_capture_durable(conn, artifacts, &existing, &capture)?;
            return Ok(DurableCaptureReceipt {
                effect_id: capture.effect_id,
                fingerprint: capture.fingerprint,
                output_hash: capture.output_hash,
                witness_hash: capture.witness_hash,
            });
        }
        return Err(StorageError::IdempotencyMismatch);
    }
    let execution =
        get_execution_sync(conn, capture.execution_id)?.ok_or(StorageError::ExecutionMissing)?;
    if execution.status.is_terminal() {
        return Err(StorageError::InvalidInput(
            "终态 execution 不能写入 effect capture".to_string(),
        ));
    }
    let occurred_at_ms = now_millis();
    let request_body_secret = write_request_body_secret(conn, artifacts, &capture, occurred_at_ms)?;
    let (safe_output, secret_headers) = split_sensitive_output(capture.output.as_ref());
    let output_payload = serialize(&StoredEffectOutput {
        output: safe_output,
    })?;
    let output_artifact = artifacts.write(ArtifactKind::Body, output_payload.as_bytes())?;
    let witness_artifact = artifacts.write(ArtifactKind::Body, witness_payload.as_bytes())?;
    if witness_artifact.hash != expected_witness_artifact_hash {
        return Err(StorageError::InvalidInput(
            "effect witness artifact hash 与内容不一致".to_string(),
        ));
    }
    let response_headers_secret = secret_headers
        .as_ref()
        .map(|headers| {
            serialize(headers)
                .and_then(|json| write_secret(conn, artifacts, json.as_bytes(), occurred_at_ms))
        })
        .transpose()?;
    let expected_version = stream_version(conn, &execution_stream_id(capture.execution_id))?;
    let event = EventDraft {
        stream_id: execution_stream_id(capture.execution_id),
        expected_version,
        event_id: capture.effect_id,
        event_type: EventType::Execution,
        schema_version: 2,
        correlation_id: None,
        causation_id: None,
        trace_id: "effect-archive".to_string(),
        occurred_at_ms,
        payload: serde_json::json!({
            "kind": "effect_captured",
            "effect_id": capture.effect_id,
            "node_id": capture.node_id,
            "effect_kind": capture.kind,
            "fingerprint": capture.fingerprint,
            "output_hash": capture.output_hash,
            "witness_hash": capture.witness_hash,
            "witness_artifact_hash": witness_artifact.hash,
            "has_request_body": has_request_body,
        }),
        source_identity: Some(execution.source_identity),
    };
    let execution_id = capture.execution_id;
    let effect_id = capture.effect_id;
    let node_id = capture.node_id;
    let effect_kind = serialize(&capture.kind)?;
    let fingerprint = capture.fingerprint.clone();
    let output_hash = capture.output_hash.clone();
    let witness_hash = capture.witness_hash.clone();
    let output_artifact_hash = output_artifact.hash.clone();
    let witness_artifact_hash = witness_artifact.hash.clone();
    let response_headers_secret_id = response_headers_secret
        .as_ref()
        .map(|value| value.secret_id);
    let request_body_secret_id = request_body_secret.as_ref().map(|value| value.secret_id);
    let mut links = Vec::with_capacity(2);
    push_new_artifact_link(&mut links, output_artifact);
    push_new_artifact_link(&mut links, witness_artifact);
    append_event_transaction(conn, &event, &links, move |conn, global_seq, revision| {
        let owner_id = format!("{execution_id}:{effect_id}");
        if let Some(secret) = &response_headers_secret {
            retain_pending_secret(
                conn,
                secret,
                "effect_response_headers",
                &owner_id,
                occurred_at_ms,
            )?;
        }
        if let Some(secret) = &request_body_secret {
            retain_pending_secret(
                conn,
                secret,
                "effect_request_body",
                &owner_id,
                occurred_at_ms,
            )?;
        }
        sql_query(
            "INSERT INTO effect_captures (execution_id, effect_id, node_id, effect_kind, fingerprint, output_hash, witness_hash, output_artifact_hash, witness_artifact_hash, response_headers_secret_id, request_body_secret_id, global_seq) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(execution_id.to_string())
        .bind::<Text, _>(effect_id.to_string())
        .bind::<Text, _>(node_id.to_string())
        .bind::<Text, _>(&effect_kind)
        .bind::<Text, _>(&fingerprint)
        .bind::<Text, _>(&output_hash)
        .bind::<Text, _>(&witness_hash)
        .bind::<Text, _>(&output_artifact_hash)
        .bind::<Text, _>(&witness_artifact_hash)
        .bind::<Nullable<Text>, _>(response_headers_secret_id.map(|value| value.to_string()).as_deref())
        .bind::<Nullable<Text>, _>(request_body_secret_id.map(|value| value.to_string()).as_deref())
        .bind::<BigInt, _>(to_i64(global_seq)?)
        .execute(conn)
        .map_err(database_error)?;
        update_execution_revision(conn, execution_id, revision, global_seq)
    })?;
    Ok(DurableCaptureReceipt {
        effect_id: capture.effect_id,
        fingerprint: capture.fingerprint,
        output_hash: capture.output_hash,
        witness_hash: capture.witness_hash,
    })
}

/// 读取 archive 并在向 runtime 返回前完成 artifact/witness/secret 完整性验证。
pub(crate) fn load_effect_capture(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    lookup: &EffectReplayLookup,
) -> Result<Option<EffectCapture>, StorageError> {
    let execution = get_execution_sync(conn, lookup.archived_execution_id)?;
    if let Some(execution) = execution
        && !execution.replayable
    {
        return Err(StorageError::ReplayUnavailable(
            "archive 已被 GC".to_string(),
        ));
    }
    let kind = serialize(&lookup.kind)?;
    let row = sql_query(
        "SELECT execution_id, effect_id, node_id, effect_kind, fingerprint, output_hash, witness_hash, output_artifact_hash, witness_artifact_hash, response_headers_secret_id, request_body_secret_id FROM effect_captures WHERE execution_id = ? AND node_id = ? AND effect_kind = ? ORDER BY global_seq ASC LIMIT 1",
    )
    .bind::<Text, _>(lookup.archived_execution_id.to_string())
    .bind::<Text, _>(lookup.node_id.to_string())
    .bind::<Text, _>(&kind)
    .get_result::<EffectCaptureRow>(conn)
    .optional()
    .map_err(database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let witness_hash = row.witness_hash.clone().ok_or_else(|| {
        StorageError::ReplayUnavailable("effect archive 缺少 witness hash".to_string())
    })?;
    let witness_artifact_hash = row.witness_artifact_hash.clone().ok_or_else(|| {
        StorageError::ReplayUnavailable("effect archive 缺少 witness artifact".to_string())
    })?;
    ensure_blake3_hash(&witness_hash, "effect witness hash").map_err(|_| {
        StorageError::ReplayUnavailable("effect archive witness hash 无效".to_string())
    })?;
    ensure_blake3_hash(&witness_artifact_hash, "effect witness artifact hash").map_err(|_| {
        StorageError::ReplayUnavailable("effect archive witness artifact hash 无效".to_string())
    })?;
    let payload = deserialize::<StoredEffectOutput>(&read_body_by_hash(
        conn,
        artifacts,
        &row.output_artifact_hash,
    )?)?;
    let witness = read_body_by_hash(conn, artifacts, &witness_artifact_hash)
        .map_err(|_| StorageError::ReplayUnavailable("effect witness artifact 不可用".to_string()))
        .and_then(|bytes| {
            deserialize::<StoredEffectWitness>(&bytes).map_err(|_| {
                StorageError::ReplayUnavailable("effect witness artifact 损坏".to_string())
            })
        })?
        .witness;
    let secret_owner_id = format!("{}:{}", row.execution_id, row.effect_id);
    verify_request_body(
        conn,
        artifacts,
        &witness,
        row.request_body_secret_id.as_deref(),
        &secret_owner_id,
    )?;
    let output = if let Some(secret_id) = row.response_headers_secret_id.as_deref() {
        let secret = deserialize::<SecretHeaderSnapshot>(&read_owned_secret(
            conn,
            artifacts,
            SecretArtifactId::from_str(secret_id)?,
            "effect_response_headers",
            &secret_owner_id,
        )?)?;
        restore_sensitive_headers(payload.output, secret.headers)
    } else {
        payload.output
    };
    EffectCapture::from_archived(ArchivedEffectCapture {
        execution_id: Uuid::parse_str(&row.execution_id)
            .map_err(|_| StorageError::InvalidInput("损坏的 execution ID".to_string()))?,
        effect_id: Uuid::parse_str(&row.effect_id)
            .map_err(|_| StorageError::InvalidInput("损坏的 effect ID".to_string()))?,
        node_id: Uuid::parse_str(&row.node_id)
            .map_err(|_| StorageError::InvalidInput("损坏的 node ID".to_string()))?,
        kind: deserialize(row.effect_kind.as_bytes())?,
        fingerprint: row.fingerprint,
        output_hash: row.output_hash,
        witness_hash,
        output: Arc::new(output),
        witness,
    })
    .map(Some)
    .map_err(|_| StorageError::ReplayUnavailable("effect archive witness 与输出不一致".to_string()))
}

/// 验证 request body secret artifact 仍可认证，且与 witness 的逻辑 hash/长度一致。
fn verify_request_body(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    witness: &EffectWitness,
    secret_id: Option<&str>,
    owner_id: &str,
) -> Result<(), StorageError> {
    let witness_body = match witness {
        EffectWitness::Http(witness) => witness.request.body.as_ref(),
        EffectWitness::QuickJs(_) | EffectWitness::Extract(_) => None,
    };
    match (witness_body, secret_id) {
        (None, None) => Ok(()),
        (None, Some(_)) | (Some(_), None) => Err(StorageError::ReplayUnavailable(
            "effect archive request body ref 与 witness 不一致".to_string(),
        )),
        (Some(witness), Some(secret_id)) => {
            let bytes = read_owned_secret(
                conn,
                artifacts,
                SecretArtifactId::from_str(secret_id)?,
                "effect_request_body",
                owner_id,
            )?;
            let byte_len = u64::try_from(bytes.len()).map_err(|_| {
                StorageError::ReplayUnavailable("effect archive request body 长度无效".to_string())
            })?;
            if byte_len != witness.byte_len
                || blake3::hash(&bytes).to_hex().as_str() != witness.hash
            {
                return Err(StorageError::ReplayUnavailable(
                    "effect archive request body 内容不一致".to_string(),
                ));
            }
            Ok(())
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredEffectOutput {
    output: EffectOutput,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredEffectWitness {
    witness: EffectWitness,
}

#[derive(Serialize, Deserialize)]
struct SecretHeaderSnapshot {
    headers: HashMap<String, String>,
}

/// 防止重复 receipt 在磁盘/SQLite 已损坏时伪装为 durable 成功。
fn ensure_existing_capture_durable(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    row: &EffectCaptureRow,
    capture: &EffectCapture,
) -> Result<(), StorageError> {
    let output_payload = deserialize::<StoredEffectOutput>(&read_body_by_hash(
        conn,
        artifacts,
        &row.output_artifact_hash,
    )?)?;
    let witness_artifact_hash = row
        .witness_artifact_hash
        .as_deref()
        .ok_or_else(|| StorageError::ArtifactUnavailable("effect witness artifact".to_string()))?;
    let stored_witness = deserialize::<StoredEffectWitness>(&read_body_by_hash(
        conn,
        artifacts,
        witness_artifact_hash,
    )?)?;
    if stored_witness.witness != capture.witness {
        return Err(StorageError::IdempotencyMismatch);
    }
    let secret_owner_id = format!("{}:{}", row.execution_id, row.effect_id);
    let output = if let Some(secret_id) = row.response_headers_secret_id.as_deref() {
        let secret = deserialize::<SecretHeaderSnapshot>(&read_owned_secret(
            conn,
            artifacts,
            SecretArtifactId::from_str(secret_id)?,
            "effect_response_headers",
            &secret_owner_id,
        )?)?;
        restore_sensitive_headers(output_payload.output, secret.headers)
    } else {
        output_payload.output
    };
    if output != *capture.output {
        return Err(StorageError::IdempotencyMismatch);
    }
    verify_request_body(
        conn,
        artifacts,
        &capture.witness,
        row.request_body_secret_id.as_deref(),
        &secret_owner_id,
    )
}

/// 将 response 中仅可加密保存的 header 从可 replay output 剥离。
fn split_sensitive_output(output: &EffectOutput) -> (EffectOutput, Option<SecretHeaderSnapshot>) {
    match output {
        EffectOutput::Http(response) => {
            let mut safe_response = response.clone();
            let mut secret_headers = HashMap::new();
            safe_response.headers.retain(|key, value| {
                if is_sensitive_header(key) {
                    secret_headers.insert(key.clone(), value.clone());
                    false
                } else {
                    true
                }
            });
            let secret = (!secret_headers.is_empty()).then_some(SecretHeaderSnapshot {
                headers: secret_headers,
            });
            (EffectOutput::Http(safe_response), secret)
        }
        _ => (output.clone(), None),
    }
}

fn restore_sensitive_headers(
    output: EffectOutput,
    secret_headers: HashMap<String, String>,
) -> EffectOutput {
    match output {
        EffectOutput::Http(mut response) => {
            response.headers.extend(secret_headers);
            EffectOutput::Http(response)
        }
        other => other,
    }
}

fn is_sensitive_header(header: &str) -> bool {
    SensitiveNamePolicy::is_sensitive_response_header(header)
}

#[derive(QueryableByName)]
struct EffectCaptureRow {
    #[diesel(sql_type = Text)]
    execution_id: String,
    #[diesel(sql_type = Text)]
    effect_id: String,
    #[diesel(sql_type = Text)]
    node_id: String,
    #[diesel(sql_type = Text)]
    effect_kind: String,
    #[diesel(sql_type = Text)]
    fingerprint: String,
    #[diesel(sql_type = Text)]
    output_hash: String,
    #[diesel(sql_type = Nullable<Text>)]
    witness_hash: Option<String>,
    #[diesel(sql_type = Text)]
    output_artifact_hash: String,
    #[diesel(sql_type = Nullable<Text>)]
    witness_artifact_hash: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    response_headers_secret_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    request_body_secret_id: Option<String>,
}

fn effect_capture_row(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
    effect_id: Uuid,
) -> Result<Option<EffectCaptureRow>, StorageError> {
    sql_query(
        "SELECT execution_id, effect_id, node_id, effect_kind, fingerprint, output_hash, witness_hash, output_artifact_hash, witness_artifact_hash, response_headers_secret_id, request_body_secret_id FROM effect_captures WHERE execution_id = ? AND effect_id = ?",
    )
    .bind::<Text, _>(execution_id.to_string())
    .bind::<Text, _>(effect_id.to_string())
    .get_result::<EffectCaptureRow>(conn)
    .optional()
    .map_err(database_error)
}

#[async_trait]
impl EffectArchive for EventProjectionStorage {
    async fn persist_durable(
        &self,
        capture: EffectCapture,
    ) -> Result<DurableCaptureReceipt, EffectArchiveError> {
        self.persist_effect_capture(capture)
            .await
            .map_err(|error| effect_archive_error(&error))
    }

    async fn load_replay(
        &self,
        lookup: EffectReplayLookup,
    ) -> Result<Option<EffectCapture>, EffectArchiveError> {
        self.replay_capture(lookup)
            .await
            .map_err(|error| effect_archive_error(&error))
    }
}

fn effect_archive_error(error: &StorageError) -> EffectArchiveError {
    match error {
        StorageError::MasterKeyUnavailable
        | StorageError::SecretUnavailable
        | StorageError::KeyringLocked
        | StorageError::KeyringUnavailable
        | StorageError::KeyLost => {
            EffectArchiveError::new("secret artifact 不可用，历史 execution 不能 replay")
        }
        StorageError::ArtifactUnavailable(_) => {
            EffectArchiveError::new("body artifact 缺失，历史 execution 不能 replay")
        }
        StorageError::ReplayUnavailable(_) => {
            EffectArchiveError::new("execution archive 不可 replay")
        }
        _ => EffectArchiveError::new("effect archive 持久化或读取失败"),
    }
}
