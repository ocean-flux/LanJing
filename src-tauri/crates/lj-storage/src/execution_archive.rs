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
use lj_rule_model::{ControlTrace, EventType, InvocationPath, SensitiveNamePolicy, canonical_json};
use lj_runtime::{
    ArchivedEffectCapture, ControlReplayLookup, ControlTraceCapture, ControlTraceReceipt,
    DurableCaptureReceipt, EffectArchive, EffectArchiveError, EffectArchiveErrorCode,
    EffectCapture, EffectCaptureMaterialSensitivity, EffectOutput, EffectReplayLookup,
    EffectWitness, ReplayCompletionLookup, control_trace_hash,
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

struct PreparedEffectCapture {
    capture: EffectCapture,
    invocation_path_json: String,
    invocation_ordinal: i64,
    witness_payload: String,
    expected_witness_artifact_hash: String,
    has_request_body: bool,
}

fn prepare_effect_capture_persistence(
    capture: EffectCapture,
) -> Result<PreparedEffectCapture, StorageError> {
    capture.validate_replay_integrity().map_err(|_| {
        StorageError::InvalidInput("effect capture witness 不满足完整性合同".to_string())
    })?;
    ensure_blake3_hash(&capture.output_hash, "effect output hash")?;
    ensure_blake3_hash(&capture.witness_hash, "effect witness hash")?;
    let invocation_path_json =
        canonical_json(&capture.invocation_path).map_err(|_| StorageError::Serialization)?;
    let invocation_ordinal = to_i64(capture.invocation_path.ordinal())?;
    let witness_payload = canonical_json(&StoredEffectWitness {
        witness: capture.witness.clone(),
    })
    .map_err(|_| StorageError::Serialization)?;
    let expected_witness_artifact_hash = blake3::hash(witness_payload.as_bytes())
        .to_hex()
        .to_string();
    let has_request_body = request_body_hash(&capture)?.is_some();

    Ok(PreparedEffectCapture {
        capture,
        invocation_path_json,
        invocation_ordinal,
        witness_payload,
        expected_witness_artifact_hash,
        has_request_body,
    })
}

fn existing_effect_capture_receipt(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    prepared: &PreparedEffectCapture,
) -> Result<Option<DurableCaptureReceipt>, StorageError> {
    let capture = &prepared.capture;
    let Some(existing) = effect_capture_row(conn, capture.execution_id, capture.effect_id)? else {
        return Ok(None);
    };
    let (Some(existing_path), Some(existing_ordinal)) = (
        existing.invocation_path_json.as_deref(),
        existing.invocation_ordinal,
    ) else {
        return Err(StorageError::LegacyInvocationArchiveUnsupported);
    };
    if existing_path != prepared.invocation_path_json
        || existing_ordinal != prepared.invocation_ordinal
    {
        return Err(StorageError::IdempotencyMismatch);
    }
    validate_invocation_claim(
        conn,
        capture.execution_id,
        &capture.invocation_path,
        "effect",
        &capture.effect_id.to_string(),
    )?;
    if existing.fingerprint == capture.fingerprint
        && existing.output_hash == capture.output_hash
        && existing.witness_hash.as_deref() == Some(capture.witness_hash.as_str())
        && existing.witness_artifact_hash.as_deref()
            == Some(prepared.expected_witness_artifact_hash.as_str())
        && existing.request_body_secret_id.is_some() == prepared.has_request_body
    {
        ensure_existing_capture_durable(conn, artifacts, &existing, capture)?;
        return Ok(Some(DurableCaptureReceipt {
            effect_id: capture.effect_id,
            invocation_path: capture.invocation_path.clone(),
            fingerprint: capture.fingerprint.clone(),
            output_hash: capture.output_hash.clone(),
            witness_hash: capture.witness_hash.clone(),
        }));
    }
    Err(StorageError::IdempotencyMismatch)
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
    let prepared = prepare_effect_capture_persistence(capture)?;
    if let Some(receipt) = existing_effect_capture_receipt(conn, artifacts, &prepared)? {
        return Ok(receipt);
    }
    let PreparedEffectCapture {
        capture,
        invocation_path_json,
        invocation_ordinal,
        witness_payload,
        expected_witness_artifact_hash,
        has_request_body,
    } = prepared;

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

    let execution_id = capture.execution_id;
    let effect_id = capture.effect_id;
    let invocation_path = capture.invocation_path.clone();
    let node_id = invocation_path.node_id();
    let effect_kind = serialize(&capture.kind)?;
    let fingerprint = capture.fingerprint.clone();
    let output_hash = capture.output_hash.clone();
    let witness_hash = capture.witness_hash.clone();
    let output_artifact_hash = output_artifact.hash.clone();
    let witness_artifact_hash = witness_artifact.hash.clone();
    let response_headers_secret_id = response_headers_secret
        .as_ref()
        .map(|value| value.secret_id.to_string());
    let request_body_secret_id = request_body_secret
        .as_ref()
        .map(|value| value.secret_id.to_string());
    let receipt = DurableCaptureReceipt {
        effect_id,
        invocation_path: invocation_path.clone(),
        fingerprint: fingerprint.clone(),
        output_hash: output_hash.clone(),
        witness_hash: witness_hash.clone(),
    };
    let expected_version = stream_version(conn, &execution_stream_id(execution_id))?;
    let event = EventDraft {
        stream_id: execution_stream_id(execution_id),
        expected_version,
        event_id: effect_id,
        event_type: EventType::Execution,
        schema_version: 2,
        correlation_id: None,
        causation_id: None,
        trace_id: "effect-archive".to_string(),
        occurred_at_ms,
        payload: serde_json::json!({
            "kind": "effect_captured",
            "effect_id": effect_id,
            "node_id": node_id,
            "invocation_path": invocation_path,
            "effect_kind": capture.kind,
            "fingerprint": fingerprint,
            "output_hash": output_hash,
            "witness_hash": witness_hash,
            "witness_artifact_hash": witness_artifact_hash,
            "has_request_body": has_request_body,
        }),
        source_identity: Some(execution.source_identity),
    };
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
        claim_invocation(
            conn,
            execution_id,
            &invocation_path_json,
            invocation_ordinal,
            node_id,
            "effect",
            &effect_id.to_string(),
        )?;
        sql_query(
            "INSERT INTO effect_captures (execution_id, effect_id, node_id, invocation_path_json, invocation_ordinal, effect_kind, fingerprint, output_hash, witness_hash, output_artifact_hash, witness_artifact_hash, response_headers_secret_id, request_body_secret_id, global_seq) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(execution_id.to_string())
        .bind::<Text, _>(effect_id.to_string())
        .bind::<Text, _>(node_id.to_string())
        .bind::<Text, _>(&invocation_path_json)
        .bind::<BigInt, _>(invocation_ordinal)
        .bind::<Text, _>(&effect_kind)
        .bind::<Text, _>(&fingerprint)
        .bind::<Text, _>(&output_hash)
        .bind::<Text, _>(&witness_hash)
        .bind::<Text, _>(&output_artifact_hash)
        .bind::<Text, _>(&witness_artifact_hash)
        .bind::<Nullable<Text>, _>(response_headers_secret_id.as_deref())
        .bind::<Nullable<Text>, _>(request_body_secret_id.as_deref())
        .bind::<BigInt, _>(to_i64(global_seq)?)
        .execute(conn)
        .map_err(database_error)?;
        update_execution_revision(conn, execution_id, revision, global_seq)?;
        Ok(())
    })?;
    Ok(receipt)
}

/// 读取 archive 并在向 runtime 返回前完成 artifact/witness/secret 完整性验证。
pub(crate) fn load_effect_capture(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    lookup: &EffectReplayLookup,
) -> Result<Option<EffectCapture>, StorageError> {
    ensure_replayable_execution(conn, lookup.archived_execution_id)?;
    let invocation_path_json =
        canonical_json(&lookup.invocation_path).map_err(|_| StorageError::Serialization)?;
    let invocation_ordinal = to_i64(lookup.invocation_path.ordinal())?;
    let row = sql_query(
        "SELECT capture.execution_id, capture.effect_id, capture.node_id, capture.effect_kind, capture.invocation_path_json, capture.invocation_ordinal, capture.fingerprint, capture.output_hash, capture.witness_hash, capture.output_artifact_hash, capture.witness_artifact_hash, capture.response_headers_secret_id, capture.request_body_secret_id FROM execution_invocation_ledger AS ledger INNER JOIN effect_captures AS capture ON capture.execution_id = ledger.execution_id AND capture.invocation_ordinal = ledger.invocation_ordinal WHERE ledger.execution_id = ? AND ledger.invocation_ordinal = ? AND ledger.invocation_kind = 'effect' AND ledger.invocation_path_json = ? AND capture.invocation_path_json = ledger.invocation_path_json",
    )
    .bind::<Text, _>(lookup.archived_execution_id.to_string())
    .bind::<BigInt, _>(invocation_ordinal)
    .bind::<Text, _>(&invocation_path_json)
    .get_result::<EffectCaptureRow>(conn)
    .optional()
    .map_err(database_error)?;
    let Some(row) = row else {
        reject_legacy_invocation_archive(conn, lookup.archived_execution_id)?;
        reject_mismatched_invocation(
            conn,
            lookup.archived_execution_id,
            invocation_ordinal,
            &invocation_path_json,
            "effect",
        )?;
        return Ok(None);
    };
    let stored_path_json = row
        .invocation_path_json
        .as_deref()
        .ok_or(StorageError::LegacyInvocationArchiveUnsupported)?;
    let stored_ordinal = row
        .invocation_ordinal
        .ok_or(StorageError::LegacyInvocationArchiveUnsupported)?;
    if stored_path_json != invocation_path_json || stored_ordinal != invocation_ordinal {
        return Err(StorageError::ReplayUnavailable(
            "effect invocation identity 不匹配".to_string(),
        ));
    }
    let stored_path: InvocationPath = deserialize(stored_path_json.as_bytes())?;
    if stored_path != lookup.invocation_path
        || row.node_id != lookup.invocation_path.node_id().to_string()
    {
        return Err(StorageError::ReplayUnavailable(
            "effect invocation path 被篡改".to_string(),
        ));
    }
    validate_invocation_claim(
        conn,
        lookup.archived_execution_id,
        &stored_path,
        "effect",
        &row.effect_id,
    )?;
    let stored_kind = deserialize(row.effect_kind.as_bytes())?;
    if stored_kind != lookup.kind {
        return Err(StorageError::ReplayUnavailable(
            "effect invocation kind 不匹配".to_string(),
        ));
    }
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
        invocation_path: stored_path,
        kind: stored_kind,
        fingerprint: row.fingerprint,
        output_hash: row.output_hash,
        witness_hash,
        output: Arc::new(output),
        witness,
    })
    .map(Some)
    .map_err(|_| StorageError::ReplayUnavailable("effect archive witness 与输出不一致".to_string()))
}

/// 在 writer transaction 中 durable 保存一个 control trace。
pub(crate) fn persist_control_trace(
    conn: &mut SqliteConnection,
    capture: &ControlTraceCapture,
) -> Result<ControlTraceReceipt, StorageError> {
    let execution =
        get_execution_sync(conn, capture.execution_id)?.ok_or(StorageError::ExecutionMissing)?;
    if execution.status.is_terminal() {
        return Err(StorageError::InvalidInput(
            "终态 execution 不能写入 control trace".to_string(),
        ));
    }
    let actual_hash = control_trace_hash(&capture.trace)
        .map_err(|_| StorageError::InvalidInput("control trace 无法 hash".to_string()))?;
    ensure_blake3_hash(&capture.trace_hash, "control trace hash")?;
    if capture.trace_hash != actual_hash {
        return Err(StorageError::InvalidInput(
            "control trace hash 与内容不一致".to_string(),
        ));
    }
    let invocation_path_json =
        canonical_json(&capture.invocation_path).map_err(|_| StorageError::Serialization)?;
    let invocation_ordinal = to_i64(capture.invocation_path.ordinal())?;
    let trace_json = canonical_json(&capture.trace).map_err(|_| StorageError::Serialization)?;
    let payload_id = format!("control:{invocation_ordinal}");
    let receipt = ControlTraceReceipt {
        invocation_path: capture.invocation_path.clone(),
        trace_hash: capture.trace_hash.clone(),
    };

    conn.immediate_transaction::<_, StorageError, _>(|conn| {
        if let Some(existing) = control_trace_row(
            conn,
            capture.execution_id,
            invocation_ordinal,
        )? {
            validate_invocation_claim(
                conn,
                capture.execution_id,
                &capture.invocation_path,
                "control",
                &payload_id,
            )?;
            if existing.invocation_path_json == invocation_path_json
                && existing.trace_hash == capture.trace_hash
                && existing.trace_json == trace_json
            {
                return Ok(());
            }
            return Err(StorageError::IdempotencyMismatch);
        }
        claim_invocation(
            conn,
            capture.execution_id,
            &invocation_path_json,
            invocation_ordinal,
            capture.invocation_path.node_id(),
            "control",
            &payload_id,
        )?;
        sql_query(
            "INSERT INTO control_traces (execution_id, invocation_ordinal, invocation_path_json, trace_hash, trace_json) VALUES (?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(capture.execution_id.to_string())
        .bind::<BigInt, _>(invocation_ordinal)
        .bind::<Text, _>(&invocation_path_json)
        .bind::<Text, _>(&capture.trace_hash)
        .bind::<Text, _>(&trace_json)
        .execute(conn)
        .map_err(database_error)?;
        Ok(())
    })?;
    Ok(receipt)
}

/// exact 读取并验证 control trace。
pub(crate) fn load_control_trace(
    conn: &mut SqliteConnection,
    lookup: &ControlReplayLookup,
) -> Result<Option<ControlTraceCapture>, StorageError> {
    ensure_replayable_execution(conn, lookup.archived_execution_id)?;
    let invocation_path_json =
        canonical_json(&lookup.invocation_path).map_err(|_| StorageError::Serialization)?;
    let invocation_ordinal = to_i64(lookup.invocation_path.ordinal())?;
    let row = sql_query(
        "SELECT trace.execution_id, trace.invocation_ordinal, trace.invocation_path_json, trace.trace_hash, trace.trace_json FROM execution_invocation_ledger AS ledger INNER JOIN control_traces AS trace ON trace.execution_id = ledger.execution_id AND trace.invocation_ordinal = ledger.invocation_ordinal WHERE ledger.execution_id = ? AND ledger.invocation_ordinal = ? AND ledger.invocation_kind = 'control' AND ledger.invocation_path_json = ? AND trace.invocation_path_json = ledger.invocation_path_json",
    )
    .bind::<Text, _>(lookup.archived_execution_id.to_string())
    .bind::<BigInt, _>(invocation_ordinal)
    .bind::<Text, _>(&invocation_path_json)
    .get_result::<ControlTraceRow>(conn)
    .optional()
    .map_err(database_error)?;
    let Some(row) = row else {
        reject_legacy_invocation_archive(conn, lookup.archived_execution_id)?;
        reject_mismatched_invocation(
            conn,
            lookup.archived_execution_id,
            invocation_ordinal,
            &invocation_path_json,
            "control",
        )?;
        return Ok(None);
    };
    if row.execution_id != lookup.archived_execution_id.to_string()
        || row.invocation_ordinal != invocation_ordinal
        || row.invocation_path_json != invocation_path_json
    {
        return Err(StorageError::ReplayUnavailable(
            "control trace identity 不匹配".to_string(),
        ));
    }
    let payload_id = format!("control:{invocation_ordinal}");
    validate_invocation_claim(
        conn,
        lookup.archived_execution_id,
        &lookup.invocation_path,
        "control",
        &payload_id,
    )?;
    let trace: ControlTrace = deserialize(row.trace_json.as_bytes())?;
    let trace_hash = control_trace_hash(&trace)
        .map_err(|_| StorageError::ReplayUnavailable("control trace 无法 hash".to_string()))?;
    if trace_hash != row.trace_hash {
        return Err(StorageError::ReplayUnavailable(
            "control trace hash 不匹配".to_string(),
        ));
    }
    Ok(Some(ControlTraceCapture {
        execution_id: lookup.archived_execution_id,
        invocation_path: lookup.invocation_path.clone(),
        trace,
        trace_hash,
    }))
}

/// 验证 replay 已消费完整且连续的 invocation ledger。
pub(crate) fn validate_replay_complete(
    conn: &mut SqliteConnection,
    lookup: ReplayCompletionLookup,
) -> Result<(), StorageError> {
    ensure_replayable_execution(conn, lookup.archived_execution_id)?;
    reject_legacy_invocation_archive(conn, lookup.archived_execution_id)?;
    let rows = invocation_rows(conn, lookup.archived_execution_id)?;
    let observed = u64::try_from(rows.len()).ok();
    let payload_count = current_invocation_payload_count(conn, lookup.archived_execution_id)?;
    if observed != Some(lookup.observed_invocation_count) || observed != Some(payload_count) {
        return Err(StorageError::ReplayUnavailable(
            "replay invocation 数量不匹配".to_string(),
        ));
    }
    for (index, row) in rows.iter().enumerate() {
        let expected = i64::try_from(index + 1).map_err(|_| {
            StorageError::ReplayUnavailable("replay invocation ordinal 溢出".to_string())
        })?;
        if row.invocation_ordinal != expected {
            return Err(StorageError::ReplayUnavailable(
                "replay invocation ordinal 不连续".to_string(),
            ));
        }
        validate_ledger_row(conn, lookup.archived_execution_id, row)?;
    }
    Ok(())
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
struct InvocationLedgerRow {
    #[diesel(sql_type = BigInt)]
    invocation_ordinal: i64,
    #[diesel(sql_type = Text)]
    invocation_kind: String,
    #[diesel(sql_type = Text)]
    node_id: String,
    #[diesel(sql_type = Text)]
    invocation_path_json: String,
    #[diesel(sql_type = Text)]
    payload_id: String,
}

#[derive(QueryableByName)]
struct ControlTraceRow {
    #[diesel(sql_type = Text)]
    execution_id: String,
    #[diesel(sql_type = BigInt)]
    invocation_ordinal: i64,
    #[diesel(sql_type = Text)]
    invocation_path_json: String,
    #[diesel(sql_type = Text)]
    trace_hash: String,
    #[diesel(sql_type = Text)]
    trace_json: String,
}

#[derive(QueryableByName)]
struct OptionalOrdinalRow {
    #[diesel(sql_type = Nullable<BigInt>)]
    value: Option<i64>,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

fn ensure_replayable_execution(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
) -> Result<(), StorageError> {
    let execution =
        get_execution_sync(conn, execution_id)?.ok_or(StorageError::ExecutionMissing)?;
    if !execution.replayable {
        return Err(StorageError::ReplayUnavailable(
            "archive 已被 GC".to_string(),
        ));
    }
    Ok(())
}

fn claim_invocation(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
    invocation_path_json: &str,
    invocation_ordinal: i64,
    node_id: Uuid,
    invocation_kind: &str,
    payload_id: &str,
) -> Result<(), StorageError> {
    let path: InvocationPath = deserialize(invocation_path_json.as_bytes())?;
    if path.node_id() != node_id
        || to_i64(path.ordinal())? != invocation_ordinal
        || !matches!(invocation_kind, "effect" | "control")
    {
        return Err(StorageError::InvalidInput(
            "invocation identity 不一致".to_string(),
        ));
    }
    let maximum = sql_query(
        "SELECT MAX(invocation_ordinal) AS value FROM execution_invocation_ledger WHERE execution_id = ?",
    )
    .bind::<Text, _>(execution_id.to_string())
    .get_result::<OptionalOrdinalRow>(conn)
    .map_err(database_error)?
    .value
    .unwrap_or(0);
    let expected = maximum
        .checked_add(1)
        .ok_or_else(|| StorageError::InvalidInput("invocation ordinal 溢出".to_string()))?;
    if invocation_ordinal != expected {
        return Err(StorageError::InvalidInput(format!(
            "invocation ordinal 必须连续：期望 {expected}，实际 {invocation_ordinal}"
        )));
    }
    sql_query(
        "INSERT INTO execution_invocation_ledger (execution_id, invocation_ordinal, invocation_kind, node_id, invocation_path_json, payload_id) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(execution_id.to_string())
    .bind::<BigInt, _>(invocation_ordinal)
    .bind::<Text, _>(invocation_kind)
    .bind::<Text, _>(node_id.to_string())
    .bind::<Text, _>(invocation_path_json)
    .bind::<Text, _>(payload_id)
    .execute(conn)
    .map_err(database_error)?;
    Ok(())
}

fn validate_invocation_claim(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
    path: &InvocationPath,
    invocation_kind: &str,
    payload_id: &str,
) -> Result<(), StorageError> {
    let invocation_ordinal = to_i64(path.ordinal())?;
    let invocation_path_json = canonical_json(path).map_err(|_| StorageError::Serialization)?;
    let row = sql_query(
        "SELECT invocation_ordinal, invocation_kind, node_id, invocation_path_json, payload_id FROM execution_invocation_ledger WHERE execution_id = ? AND invocation_ordinal = ?",
    )
    .bind::<Text, _>(execution_id.to_string())
    .bind::<BigInt, _>(invocation_ordinal)
    .get_result::<InvocationLedgerRow>(conn)
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::IdempotencyMismatch)?;
    if row.invocation_kind != invocation_kind
        || row.node_id != path.node_id().to_string()
        || row.invocation_path_json != invocation_path_json
        || row.payload_id != payload_id
    {
        return Err(StorageError::IdempotencyMismatch);
    }
    Ok(())
}

fn invocation_rows(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
) -> Result<Vec<InvocationLedgerRow>, StorageError> {
    sql_query(
        "SELECT invocation_ordinal, invocation_kind, node_id, invocation_path_json, payload_id FROM execution_invocation_ledger WHERE execution_id = ? ORDER BY invocation_ordinal ASC",
    )
    .bind::<Text, _>(execution_id.to_string())
    .load::<InvocationLedgerRow>(conn)
    .map_err(database_error)
}

fn reject_legacy_invocation_archive(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
) -> Result<(), StorageError> {
    let count = sql_query(
        "SELECT COUNT(*) AS value FROM effect_captures WHERE execution_id = ? AND (invocation_path_json IS NULL OR invocation_ordinal IS NULL)",
    )
    .bind::<Text, _>(execution_id.to_string())
    .get_result::<CountRow>(conn)
    .map_err(database_error)?
    .value;
    if count != 0 {
        return Err(StorageError::LegacyInvocationArchiveUnsupported);
    }
    Ok(())
}

fn reject_mismatched_invocation(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
    invocation_ordinal: i64,
    invocation_path_json: &str,
    expected_kind: &str,
) -> Result<(), StorageError> {
    let row = sql_query(
        "SELECT invocation_ordinal, invocation_kind, node_id, invocation_path_json, payload_id FROM execution_invocation_ledger WHERE execution_id = ? AND (invocation_ordinal = ? OR invocation_path_json = ?) LIMIT 1",
    )
    .bind::<Text, _>(execution_id.to_string())
    .bind::<BigInt, _>(invocation_ordinal)
    .bind::<Text, _>(invocation_path_json)
    .get_result::<InvocationLedgerRow>(conn)
    .optional()
    .map_err(database_error)?;
    if row.is_some_and(|row| {
        row.invocation_ordinal != invocation_ordinal
            || row.invocation_path_json != invocation_path_json
            || row.invocation_kind != expected_kind
    }) {
        return Err(StorageError::ReplayUnavailable(
            "replay invocation 顺序或归属不匹配".to_string(),
        ));
    }
    Ok(())
}

fn control_trace_row(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
    invocation_ordinal: i64,
) -> Result<Option<ControlTraceRow>, StorageError> {
    sql_query(
        "SELECT execution_id, invocation_ordinal, invocation_path_json, trace_hash, trace_json FROM control_traces WHERE execution_id = ? AND invocation_ordinal = ?",
    )
    .bind::<Text, _>(execution_id.to_string())
    .bind::<BigInt, _>(invocation_ordinal)
    .get_result::<ControlTraceRow>(conn)
    .optional()
    .map_err(database_error)
}

fn validate_ledger_row(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
    row: &InvocationLedgerRow,
) -> Result<(), StorageError> {
    let path: InvocationPath = deserialize(row.invocation_path_json.as_bytes())?;
    if to_i64(path.ordinal())? != row.invocation_ordinal
        || path.node_id().to_string() != row.node_id
    {
        return Err(StorageError::ReplayUnavailable(
            "invocation ledger path 被篡改".to_string(),
        ));
    }
    let count = match row.invocation_kind.as_str() {
        "effect" => sql_query(
            "SELECT COUNT(*) AS value FROM effect_captures WHERE execution_id = ? AND invocation_ordinal = ? AND invocation_path_json = ? AND effect_id = ?",
        )
        .bind::<Text, _>(execution_id.to_string())
        .bind::<BigInt, _>(row.invocation_ordinal)
        .bind::<Text, _>(&row.invocation_path_json)
        .bind::<Text, _>(&row.payload_id)
        .get_result::<CountRow>(conn)
        .map_err(database_error)?
        .value,
        "control" => sql_query(
            "SELECT COUNT(*) AS value FROM control_traces WHERE execution_id = ? AND invocation_ordinal = ? AND invocation_path_json = ?",
        )
        .bind::<Text, _>(execution_id.to_string())
        .bind::<BigInt, _>(row.invocation_ordinal)
        .bind::<Text, _>(&row.invocation_path_json)
        .get_result::<CountRow>(conn)
        .map_err(database_error)?
        .value,
        _ => {
            return Err(StorageError::ReplayUnavailable(
                "invocation ledger kind 无效".to_string(),
            ));
        }
    };
    if count != 1 {
        return Err(StorageError::ReplayUnavailable(
            "invocation ledger payload 缺失或重复".to_string(),
        ));
    }
    Ok(())
}

fn current_invocation_payload_count(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
) -> Result<u64, StorageError> {
    let count = sql_query(
        "SELECT (SELECT COUNT(*) FROM effect_captures WHERE execution_id = ? AND invocation_path_json IS NOT NULL AND invocation_ordinal IS NOT NULL) + (SELECT COUNT(*) FROM control_traces WHERE execution_id = ?) AS value",
    )
    .bind::<Text, _>(execution_id.to_string())
    .bind::<Text, _>(execution_id.to_string())
    .get_result::<CountRow>(conn)
    .map_err(database_error)?
    .value;
    u64::try_from(count)
        .map_err(|_| StorageError::ReplayUnavailable("invocation payload 数量无效".to_string()))
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
    #[diesel(sql_type = Nullable<Text>)]
    invocation_path_json: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    invocation_ordinal: Option<i64>,
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
        "SELECT execution_id, effect_id, node_id, invocation_path_json, invocation_ordinal, effect_kind, fingerprint, output_hash, witness_hash, output_artifact_hash, witness_artifact_hash, response_headers_secret_id, request_body_secret_id FROM effect_captures WHERE execution_id = ? AND effect_id = ?",
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

    async fn persist_control_trace(
        &self,
        capture: ControlTraceCapture,
    ) -> Result<ControlTraceReceipt, EffectArchiveError> {
        EventProjectionStorage::persist_control_trace(self, capture)
            .await
            .map_err(|error| effect_archive_error(&error))
    }

    async fn load_control_trace(
        &self,
        lookup: ControlReplayLookup,
    ) -> Result<Option<ControlTraceCapture>, EffectArchiveError> {
        self.replay_control_trace(lookup)
            .await
            .map_err(|error| effect_archive_error(&error))
    }

    async fn validate_replay_complete(
        &self,
        lookup: ReplayCompletionLookup,
    ) -> Result<(), EffectArchiveError> {
        self.validate_replay_invocations(lookup)
            .await
            .map_err(|error| effect_archive_error(&error))
    }
}

fn effect_archive_error(error: &StorageError) -> EffectArchiveError {
    match error {
        StorageError::LegacyInvocationArchiveUnsupported => EffectArchiveError::with_code(
            EffectArchiveErrorCode::LegacyRuleContractUnsupported,
            "历史 effect invocation archive 不受支持",
        ),
        StorageError::ReplayUnavailable(_)
        | StorageError::IdempotencyMismatch
        | StorageError::InvalidInput(_)
        | StorageError::ArtifactCorrupt
        | StorageError::Serialization => EffectArchiveError::with_code(
            EffectArchiveErrorCode::Integrity,
            "execution archive 完整性校验失败",
        ),
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
        _ => EffectArchiveError::new("effect archive 持久化或读取失败"),
    }
}
