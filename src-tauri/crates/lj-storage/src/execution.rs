//! Execution archive 生命周期、replay pin 与事件 catch-up 的 storage 实现。
//!
//! execution 启动时固定 source version 和 immutable Plan ref；replay 永远验证该历史 pin，
//! 绝不回退到当前 source。每次 Delta 与规范化 projection 在同一 Event transaction 中提交，
//! 所以成功 receipt 的 stream revision 与 global sequence 是 delivery/catch-up 的唯一顺序边界。
//! effect output 的 durable body/secret/witness 细节留在 sibling `execution_archive`，该模块只
//! 负责 execution 状态机和历史读取。

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Integer, Nullable, Text};
use diesel::sqlite::SqliteConnection;
use lj_media::SourceProfile;
use lj_rule_model::{EventType, ExecutionPlan, PolicyCapabilities};
use lj_runtime::ExecutionMode;
use uuid::Uuid;

use crate::artifact::ArtifactStore;
use crate::candidate_install::{
    canonical_plan_hash, get_source_row, source_cookie_namespace,
    validate_candidate_package_and_plan,
};
use crate::event_store::{
    ArtifactLink, EventDraft, SECRET_ALGORITHM, append_event_transaction, artifact_row,
    database_error, deserialize, ensure_blake3_hash, from_i64, idempotent_event, read_body_by_hash,
    serialize, stored_event_from_row, to_i64,
};
use crate::projection_query::{apply_projection_delta, validate_delta_source};
use crate::types::{
    ArtifactKind, CommitReceipt, DeltaCommit, ExecutionFinish, ExecutionPin, ExecutionRecord,
    ExecutionReplayPin, ExecutionSourceCredentials, ExecutionStart, ExecutionStatus, GcState,
    ReplayExecutionStart, StorageError, StoredEvent,
};

/// 创建 live execution 并在第一条 Event 中固定当前 source version 与 Plan ref。
pub(crate) fn process_start_execution(
    conn: &mut SqliteConnection,
    request: ExecutionStart,
) -> Result<ExecutionRecord, StorageError> {
    let source =
        get_source_row(conn, &request.source_identity)?.ok_or(StorageError::SourceMissing)?;
    let event = EventDraft {
        stream_id: execution_stream_id(request.execution_id),
        expected_version: 0,
        event_id: request.event_id,
        event_type: EventType::Execution,
        schema_version: 1,
        correlation_id: request.correlation_id,
        causation_id: None,
        trace_id: request.trace_id,
        occurred_at_ms: request.started_at_ms,
        payload: serde_json::json!({
            "kind": "started",
            "execution_id": request.execution_id,
            "source_identity": request.source_identity,
            "source_version": source.version,
            "plan_hash": source.plan_hash,
        }),
        source_identity: Some(request.source_identity.clone()),
    };
    if let Some(receipt) = idempotent_event(conn, &event)? {
        return get_execution_sync(conn, request.execution_id)?
            .ok_or(StorageError::ExecutionMissing)
            .map(|mut value| {
                value.revision = receipt.stream_version;
                value
            });
    }
    let execution_id = request.execution_id;
    let source_identity = request.source_identity;
    let source_version = source.version;
    let plan_hash = source.plan_hash;
    let plan_artifact_hash = source.plan_artifact_hash;
    let started_at_ms = request.started_at_ms;
    let links = vec![ArtifactLink::Existing {
        hash: plan_artifact_hash.clone(),
        kind: ArtifactKind::Body,
    }];
    append_event_transaction(conn, &event, &links, move |conn, global_seq, revision| {
        sql_query(
            "INSERT INTO execution_projection (execution_id, source_identity, source_version, plan_hash, plan_artifact_hash, status, pinned, archive_available, gc_state, started_at_ms, finished_at_ms, revision, updated_global_seq) VALUES (?, ?, ?, ?, ?, 'running', 0, 1, ?, ?, NULL, ?, ?)",
        )
        .bind::<Text, _>(execution_id.to_string())
        .bind::<Text, _>(&source_identity)
        .bind::<Text, _>(&source_version)
        .bind::<Text, _>(&plan_hash)
        .bind::<Text, _>(&plan_artifact_hash)
        .bind::<Text, _>(GcState::Active.as_db())
        .bind::<BigInt, _>(started_at_ms)
        .bind::<BigInt, _>(to_i64(revision)?)
        .bind::<BigInt, _>(to_i64(global_seq)?)
        .execute(conn)
        .map_err(database_error)?;
        Ok(())
    })?;
    get_execution_sync(conn, request.execution_id)?.ok_or(StorageError::ExecutionMissing)
}

/// 用已验证的历史 replay pin 创建新的 execution；不会读取当前 source。
pub(crate) fn process_start_replay_execution(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    request: ReplayExecutionStart,
) -> Result<ExecutionRecord, StorageError> {
    let canonical_pin = load_execution_replay_pin_sync(conn, artifacts, request.pin.execution_id)?;
    if !matches!(
        request.pin.mode,
        ExecutionMode::Replay {
            archived_execution_id
        } if archived_execution_id == request.pin.execution_id
    ) || request.pin != canonical_pin
    {
        return Err(StorageError::ReplayUnavailable(
            "replay start 输入 pin 与历史 archive 不一致".to_string(),
        ));
    }
    let event = EventDraft {
        stream_id: execution_stream_id(request.execution_id),
        expected_version: 0,
        event_id: request.event_id,
        event_type: EventType::Execution,
        schema_version: 1,
        correlation_id: request.correlation_id,
        causation_id: Some(canonical_pin.execution_id),
        trace_id: request.trace_id,
        occurred_at_ms: request.started_at_ms,
        payload: serde_json::json!({
            "kind": "replay_started",
            "execution_id": request.execution_id,
            "archived_execution_id": canonical_pin.execution_id,
            "source_identity": &canonical_pin.source_identity,
            "source_version": &canonical_pin.source_version,
            "plan_hash": &canonical_pin.plan_hash,
            "plan_artifact_hash": &canonical_pin.plan_artifact_hash,
            "package_artifact_hash": &canonical_pin.package_artifact_hash,
        }),
        source_identity: Some(canonical_pin.source_identity.clone()),
    };
    if let Some(receipt) = idempotent_event(conn, &event)? {
        return get_execution_sync(conn, request.execution_id)?
            .ok_or(StorageError::ExecutionMissing)
            .map(|mut value| {
                value.revision = receipt.stream_version;
                value
            });
    }
    let execution_id = request.execution_id;
    let source_identity = canonical_pin.source_identity;
    let source_version = canonical_pin.source_version;
    let plan_hash = canonical_pin.plan_hash;
    let plan_artifact_hash = canonical_pin.plan_artifact_hash;
    let package_artifact_hash = canonical_pin.package_artifact_hash;
    let started_at_ms = request.started_at_ms;
    let links = vec![
        ArtifactLink::Existing {
            hash: plan_artifact_hash.clone(),
            kind: ArtifactKind::Body,
        },
        ArtifactLink::Existing {
            hash: package_artifact_hash,
            kind: ArtifactKind::Body,
        },
    ];
    append_event_transaction(conn, &event, &links, move |conn, global_seq, revision| {
        sql_query(
            "INSERT INTO execution_projection (execution_id, source_identity, source_version, plan_hash, plan_artifact_hash, status, pinned, archive_available, gc_state, started_at_ms, finished_at_ms, revision, updated_global_seq) VALUES (?, ?, ?, ?, ?, 'running', 0, 1, ?, ?, NULL, ?, ?)",
        )
        .bind::<Text, _>(execution_id.to_string())
        .bind::<Text, _>(&source_identity)
        .bind::<Text, _>(&source_version)
        .bind::<Text, _>(&plan_hash)
        .bind::<Text, _>(&plan_artifact_hash)
        .bind::<Text, _>(GcState::Active.as_db())
        .bind::<BigInt, _>(started_at_ms)
        .bind::<BigInt, _>(to_i64(revision)?)
        .bind::<BigInt, _>(to_i64(global_seq)?)
        .execute(conn)
        .map_err(database_error)?;
        Ok(())
    })?;
    get_execution_sync(conn, request.execution_id)?.ok_or(StorageError::ExecutionMissing)
}

/// 将 execution Delta 和 O(delta) projection 更新原子提交。
pub(crate) fn process_delta(
    conn: &mut SqliteConnection,
    request: DeltaCommit,
) -> Result<CommitReceipt, StorageError> {
    let execution =
        get_execution_sync(conn, request.execution_id)?.ok_or(StorageError::ExecutionMissing)?;
    if execution.status.is_terminal() {
        return Err(StorageError::InvalidInput(
            "终态 execution 不能提交 Delta".to_string(),
        ));
    }
    validate_delta_source(&request.delta, &execution.source_identity)?;
    let payload = serialize(&request.delta)?;
    let event = EventDraft {
        stream_id: execution_stream_id(request.execution_id),
        expected_version: request.expected_version,
        event_id: request.event_id,
        event_type: EventType::Execution,
        schema_version: 1,
        correlation_id: None,
        causation_id: None,
        trace_id: request.trace_id,
        occurred_at_ms: request.occurred_at_ms,
        payload: serde_json::json!({"kind": "delta", "delta": serde_json::from_str::<serde_json::Value>(&payload).map_err(|_| StorageError::Serialization)?}),
        source_identity: Some(execution.source_identity.clone()),
    };
    if let Some(receipt) = idempotent_event(conn, &event)? {
        return Ok(receipt);
    }
    let source_identity = execution.source_identity;
    let delta = request.delta;
    let execution_id = request.execution_id;
    append_event_transaction(conn, &event, &[], move |conn, global_seq, revision| {
        apply_projection_delta(conn, &source_identity, &delta, global_seq)?;
        update_execution_revision(conn, execution_id, revision, global_seq)?;
        Ok(())
    })
}

/// 写入 execution 唯一终态；不同终态的重复写入被拒绝。
pub(crate) fn process_finish_execution(
    conn: &mut SqliteConnection,
    request: ExecutionFinish,
) -> Result<ExecutionRecord, StorageError> {
    if !request.status.is_terminal() {
        return Err(StorageError::InvalidInput(
            "execution 终态不能为 running".to_string(),
        ));
    }
    let execution =
        get_execution_sync(conn, request.execution_id)?.ok_or(StorageError::ExecutionMissing)?;
    if execution.status.is_terminal() && execution.status != request.status {
        return Err(StorageError::InvalidInput(
            "execution 已处于不同终态".to_string(),
        ));
    }
    let event = EventDraft {
        stream_id: execution_stream_id(request.execution_id),
        expected_version: request.expected_version,
        event_id: request.event_id,
        event_type: EventType::Execution,
        schema_version: 1,
        correlation_id: None,
        causation_id: None,
        trace_id: request.trace_id,
        occurred_at_ms: request.finished_at_ms,
        payload: serde_json::json!({"kind": "terminal", "status": request.status.as_db()}),
        source_identity: Some(execution.source_identity),
    };
    if idempotent_event(conn, &event)?.is_some() {
        return get_execution_sync(conn, request.execution_id)?
            .ok_or(StorageError::ExecutionMissing);
    }
    let execution_id = request.execution_id;
    let status = request.status.as_db();
    let finished_at_ms = request.finished_at_ms;
    append_event_transaction(conn, &event, &[], move |conn, global_seq, revision| {
        sql_query("UPDATE execution_projection SET status = ?, finished_at_ms = ?, revision = ?, updated_global_seq = ? WHERE execution_id = ?")
            .bind::<Text, _>(status)
            .bind::<BigInt, _>(finished_at_ms)
            .bind::<BigInt, _>(to_i64(revision)?)
            .bind::<BigInt, _>(to_i64(global_seq)?)
            .bind::<Text, _>(execution_id.to_string())
            .execute(conn)
            .map_err(database_error)?;
        Ok(())
    })?;
    get_execution_sync(conn, request.execution_id)?.ok_or(StorageError::ExecutionMissing)
}

/// 修改 pin，但不直接触发删除；retention 状态机只处理未 pin archive。
pub(crate) fn process_pin_execution(
    conn: &mut SqliteConnection,
    request: ExecutionPin,
) -> Result<ExecutionRecord, StorageError> {
    let execution =
        get_execution_sync(conn, request.execution_id)?.ok_or(StorageError::ExecutionMissing)?;
    let event = EventDraft {
        stream_id: execution_stream_id(request.execution_id),
        expected_version: request.expected_version,
        event_id: request.event_id,
        event_type: EventType::Execution,
        schema_version: 1,
        correlation_id: None,
        causation_id: None,
        trace_id: request.trace_id,
        occurred_at_ms: request.occurred_at_ms,
        payload: serde_json::json!({"kind": "pin", "pinned": request.pinned}),
        source_identity: Some(execution.source_identity),
    };
    if idempotent_event(conn, &event)?.is_some() {
        return get_execution_sync(conn, request.execution_id)?
            .ok_or(StorageError::ExecutionMissing);
    }
    let execution_id = request.execution_id;
    let pinned = i32::from(request.pinned);
    append_event_transaction(conn, &event, &[], move |conn, global_seq, revision| {
        sql_query("UPDATE execution_projection SET pinned = ?, revision = ?, updated_global_seq = ? WHERE execution_id = ?")
            .bind::<Integer, _>(pinned)
            .bind::<BigInt, _>(to_i64(revision)?)
            .bind::<BigInt, _>(to_i64(global_seq)?)
            .bind::<Text, _>(execution_id.to_string())
            .execute(conn)
            .map_err(database_error)?;
        Ok(())
    })?;
    get_execution_sync(conn, request.execution_id)?.ok_or(StorageError::ExecutionMissing)
}

/// 更新 execution summary 的 revision；只可在同一个 Event transaction closure 内调用。
pub(crate) fn update_execution_revision(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
    revision: u64,
    global_seq: u64,
) -> Result<(), StorageError> {
    let changed = sql_query(
        "UPDATE execution_projection SET revision = ?, updated_global_seq = ? WHERE execution_id = ?",
    )
    .bind::<BigInt, _>(to_i64(revision)?)
    .bind::<BigInt, _>(to_i64(global_seq)?)
    .bind::<Text, _>(execution_id.to_string())
    .execute(conn)
    .map_err(database_error)?;
    if changed == 1 {
        Ok(())
    } else {
        Err(StorageError::ExecutionMissing)
    }
}

/// 读取 execution summary；状态文本损坏时显式失败。
pub(crate) fn get_execution_sync(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
) -> Result<Option<ExecutionRecord>, StorageError> {
    let row = sql_query(
        "SELECT execution_id, source_identity, plan_hash, status, pinned, archive_available, gc_state, started_at_ms, finished_at_ms, revision FROM execution_projection WHERE execution_id = ?",
    )
    .bind::<Text, _>(execution_id.to_string())
    .get_result::<ExecutionRow>(conn)
    .optional()
    .map_err(database_error)?;
    row.map(execution_from_row).transpose()
}

/// 从 `SQLite` 行转换为公开 execution summary。
pub(crate) fn execution_from_row(row: ExecutionRow) -> Result<ExecutionRecord, StorageError> {
    Ok(ExecutionRecord {
        execution_id: Uuid::parse_str(&row.execution_id)
            .map_err(|_| StorageError::InvalidInput("损坏的 execution ID".to_string()))?,
        source_identity: row.source_identity,
        plan_hash: row.plan_hash,
        status: ExecutionStatus::from_db(&row.status)?,
        pinned: row.pinned != 0,
        replayable: row.archive_available != 0,
        gc_state: GcState::from_db(&row.gc_state)?,
        started_at_ms: row.started_at_ms,
        finished_at_ms: row.finished_at_ms,
        revision: from_i64(row.revision, "execution revision")?,
    })
}

fn ensure_replay_source_secret_available(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    secret_artifact_hash: Option<&str>,
) -> Result<(), StorageError> {
    let Some(secret_artifact_hash) = secret_artifact_hash else {
        return Ok(());
    };
    let available = (|| {
        ensure_blake3_hash(secret_artifact_hash, "replay source credential secret hash")?;
        let artifact = artifact_row(conn, secret_artifact_hash, ArtifactKind::Secret)?
            .ok_or_else(|| StorageError::ArtifactUnavailable(secret_artifact_hash.to_string()))?;
        if artifact.encryption.as_deref() != Some(SECRET_ALGORITHM) {
            return Err(StorageError::SecretUnavailable);
        }
        artifacts.ensure_secret_artifact_exists(secret_artifact_hash, &artifact.relative_path)?;
        artifacts.ensure_secret_key_available()
    })();
    available.map_err(|_| {
        StorageError::ReplayUnavailable("execution pin source credential 不可用".to_string())
    })
}

/// 加载且完整性验证 historical pin；不能以当前 source 替代历史定义。
pub(crate) fn load_execution_replay_pin_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    execution_id: Uuid,
) -> Result<ExecutionReplayPin, StorageError> {
    let row = sql_query(
        "SELECT execution_id, source_identity, source_version, plan_hash, plan_artifact_hash, archive_available, gc_state FROM execution_projection WHERE execution_id = ?",
    )
    .bind::<Text, _>(execution_id.to_string())
    .get_result::<ExecutionReplayPinRow>(conn)
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::ExecutionMissing)?;
    if row.archive_available == 0 || row.gc_state != GcState::Active.as_db() {
        return Err(StorageError::ReplayUnavailable(
            "execution archive 已被 GC 或不可 replay".to_string(),
        ));
    }
    let source = sql_query(
        "SELECT source_identity, version, profile_json, grant_json, base_url, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, secret_artifact_hash FROM source_versions WHERE source_identity = ? AND version = ?",
    )
    .bind::<Text, _>(&row.source_identity)
    .bind::<Text, _>(&row.source_version)
    .get_result::<SourceVersionReplayRow>(conn)
    .optional()
    .map_err(database_error)?
    .ok_or_else(|| {
        StorageError::ReplayUnavailable("execution pin 缺少 source version snapshot".to_string())
    })?;
    let profile_json = source
        .profile_json
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            StorageError::ReplayUnavailable("execution pin 缺少 profile snapshot".to_string())
        })?;
    let grant_json = source
        .grant_json
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            StorageError::ReplayUnavailable("execution pin 缺少 grant snapshot".to_string())
        })?;
    let base_url = source
        .base_url
        .clone()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            StorageError::ReplayUnavailable("execution pin 缺少 base URL snapshot".to_string())
        })?;
    let profile = deserialize::<SourceProfile>(profile_json.as_bytes()).map_err(|_| {
        StorageError::ReplayUnavailable("execution pin profile snapshot 损坏".to_string())
    })?;
    let grant = deserialize::<PolicyCapabilities>(grant_json.as_bytes()).map_err(|_| {
        StorageError::ReplayUnavailable("execution pin grant snapshot 损坏".to_string())
    })?;
    if source.source_identity != row.source_identity
        || source.version != row.source_version
        || source.plan_hash != row.plan_hash
        || source.plan_artifact_hash != row.plan_artifact_hash
    {
        return Err(StorageError::ReplayUnavailable(
            "execution pin source version 与 Plan ref 不一致".to_string(),
        ));
    }
    ensure_replay_source_secret_available(conn, artifacts, source.secret_artifact_hash.as_deref())?;
    ensure_blake3_hash(&row.plan_hash, "execution pin Plan hash")?;
    ensure_blake3_hash(&row.plan_artifact_hash, "execution pin Plan artifact hash")?;
    ensure_blake3_hash(
        &source.package_artifact_hash,
        "execution pin package artifact hash",
    )?;
    let package = deserialize::<lj_rule_model::RulePackage>(&read_body_by_hash(
        conn,
        artifacts,
        &source.package_artifact_hash,
    )?)
    .map_err(|_| {
        StorageError::ReplayUnavailable("execution pin package artifact 损坏".to_string())
    })?;
    let plan = deserialize::<ExecutionPlan>(&read_body_by_hash(
        conn,
        artifacts,
        &row.plan_artifact_hash,
    )?)
    .map_err(|_| StorageError::ReplayUnavailable("execution pin Plan artifact 损坏".to_string()))?;
    if profile.id.0 != row.source_identity
        || package.source_identity.id != row.source_identity
        || package.version != row.source_version
        || package.definition.base_url != base_url
        || plan.plan_hash != row.plan_hash
        || plan.definition_hash != source.definition_hash
        || canonical_plan_hash(&plan)? != row.plan_hash
    {
        return Err(StorageError::ReplayUnavailable(
            "execution pin source snapshot 或 Plan 不一致".to_string(),
        ));
    }
    validate_candidate_package_and_plan(&package, &plan).map_err(|_| {
        StorageError::ReplayUnavailable("execution pin package 与 Plan 不一致".to_string())
    })?;
    let execution_id = Uuid::parse_str(&row.execution_id)
        .map_err(|_| StorageError::InvalidInput("损坏的 execution ID".to_string()))?;
    Ok(ExecutionReplayPin {
        execution_id,
        source_identity: row.source_identity,
        source_version: row.source_version,
        profile,
        grant,
        base_url,
        package_artifact_hash: source.package_artifact_hash,
        plan,
        plan_hash: row.plan_hash,
        plan_artifact_hash: row.plan_artifact_hash,
        mode: ExecutionMode::Replay {
            archived_execution_id: execution_id,
        },
    })
}

fn execution_is_replay_sync(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
) -> Result<bool, StorageError> {
    let payload =
        sql_query("SELECT payload_json FROM events WHERE stream_id = ? AND stream_version = 1")
            .bind::<Text, _>(execution_stream_id(execution_id))
            .get_result::<JsonRow>(conn)
            .optional()
            .map_err(database_error)?;
    let Some(payload) = payload else {
        return Err(StorageError::ExecutionMissing);
    };
    let payload: serde_json::Value = deserialize(payload.payload_json.as_bytes())?;
    Ok(payload["kind"].as_str() == Some("replay_started"))
}

/// 仅为 live execution 解密固定 source-version credential；replay 必须显式失败。
pub(crate) fn load_execution_source_credentials_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    execution_id: Uuid,
) -> Result<ExecutionSourceCredentials, StorageError> {
    let execution = sql_query(
        "SELECT source_identity, source_version FROM execution_projection WHERE execution_id = ?",
    )
    .bind::<Text, _>(execution_id.to_string())
    .get_result::<ExecutionSourceVersionRow>(conn)
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::ExecutionMissing)?;
    if execution_is_replay_sync(conn, execution_id)? {
        return Err(StorageError::ReplayUnavailable(
            "replay execution 不传递 source credential".to_string(),
        ));
    }
    let source = sql_query(
        "SELECT cookie_namespace, secret_artifact_hash FROM source_versions WHERE source_identity = ? AND version = ?",
    )
    .bind::<Text, _>(&execution.source_identity)
    .bind::<Text, _>(&execution.source_version)
    .get_result::<SourceVersionCredentialRow>(conn)
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::SourceCredentialUnavailable)?;
    let cookie_namespace = if source.cookie_namespace.is_empty() {
        source_cookie_namespace(&execution.source_identity)
    } else {
        source.cookie_namespace
    };
    let Some(secret_artifact_hash) = source.secret_artifact_hash else {
        return Ok(ExecutionSourceCredentials::new(cookie_namespace, None));
    };
    ensure_blake3_hash(&secret_artifact_hash, "source credential secret hash")?;
    let artifact = artifact_row(conn, &secret_artifact_hash, ArtifactKind::Secret)?
        .ok_or_else(|| StorageError::ArtifactUnavailable(secret_artifact_hash.clone()))?;
    if artifact.encryption.as_deref() != Some(SECRET_ALGORITHM) {
        return Err(StorageError::SecretUnavailable);
    }
    let secret_bytes = artifacts.read_secret(&secret_artifact_hash, &artifact.relative_path)?;
    Ok(ExecutionSourceCredentials::new(
        cookie_namespace,
        Some(secret_bytes),
    ))
}

/// 按 stream revision 有序读取持久事件，供 delivery catch-up 使用。
pub(crate) fn events_after_stream_sync(
    conn: &mut SqliteConnection,
    stream_id: &str,
    after_version: u64,
) -> Result<Vec<StoredEvent>, StorageError> {
    let rows = sql_query(
        "SELECT global_seq, stream_id, stream_version, event_id, source_identity, event_type, schema_version, correlation_id, causation_id, trace_id, occurred_at_ms, payload_json, artifact_refs_json, secret_refs_json FROM events WHERE stream_id = ? AND stream_version > ? ORDER BY stream_version ASC",
    )
    .bind::<Text, _>(stream_id)
    .bind::<BigInt, _>(to_i64(after_version)?)
    .load::<crate::event_store::EventRow>(conn)
    .map_err(database_error)?;
    rows.into_iter().map(stored_event_from_row).collect()
}

/// 按 source/global sequence 有序读取事件，供 checkpoint 后恢复使用。
pub(crate) fn events_after_source_sync(
    conn: &mut SqliteConnection,
    source_identity: &str,
    after_global_seq: u64,
) -> Result<Vec<StoredEvent>, StorageError> {
    let rows = sql_query(
        "SELECT global_seq, stream_id, stream_version, event_id, source_identity, event_type, schema_version, correlation_id, causation_id, trace_id, occurred_at_ms, payload_json, artifact_refs_json, secret_refs_json FROM events WHERE source_identity = ? AND global_seq > ? ORDER BY global_seq ASC",
    )
    .bind::<Text, _>(source_identity)
    .bind::<BigInt, _>(to_i64(after_global_seq)?)
    .load::<crate::event_store::EventRow>(conn)
    .map_err(database_error)?;
    rows.into_iter().map(stored_event_from_row).collect()
}

#[derive(QueryableByName)]
struct JsonRow {
    #[diesel(sql_type = Text)]
    payload_json: String,
}

#[derive(QueryableByName)]
struct SourceVersionReplayRow {
    #[diesel(sql_type = Text)]
    source_identity: String,
    #[diesel(sql_type = Text)]
    version: String,
    #[diesel(sql_type = Nullable<Text>)]
    profile_json: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    grant_json: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    base_url: Option<String>,
    #[diesel(sql_type = Text)]
    package_artifact_hash: String,
    #[diesel(sql_type = Text)]
    plan_artifact_hash: String,
    #[diesel(sql_type = Text)]
    definition_hash: String,
    #[diesel(sql_type = Text)]
    plan_hash: String,
    #[diesel(sql_type = Nullable<Text>)]
    secret_artifact_hash: Option<String>,
}

#[derive(QueryableByName)]
struct SourceVersionCredentialRow {
    #[diesel(sql_type = Text)]
    cookie_namespace: String,
    #[diesel(sql_type = Nullable<Text>)]
    secret_artifact_hash: Option<String>,
}

#[derive(QueryableByName)]
pub(crate) struct ExecutionRow {
    #[diesel(sql_type = Text)]
    execution_id: String,
    #[diesel(sql_type = Text)]
    source_identity: String,
    #[diesel(sql_type = Text)]
    plan_hash: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Integer)]
    pinned: i32,
    #[diesel(sql_type = Integer)]
    archive_available: i32,
    #[diesel(sql_type = Text)]
    gc_state: String,
    #[diesel(sql_type = BigInt)]
    started_at_ms: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    finished_at_ms: Option<i64>,
    #[diesel(sql_type = BigInt)]
    revision: i64,
}

#[derive(QueryableByName)]
struct ExecutionReplayPinRow {
    #[diesel(sql_type = Text)]
    execution_id: String,
    #[diesel(sql_type = Text)]
    source_identity: String,
    #[diesel(sql_type = Text)]
    source_version: String,
    #[diesel(sql_type = Text)]
    plan_hash: String,
    #[diesel(sql_type = Text)]
    plan_artifact_hash: String,
    #[diesel(sql_type = Integer)]
    archive_available: i32,
    #[diesel(sql_type = Text)]
    gc_state: String,
}

#[derive(QueryableByName)]
struct ExecutionSourceVersionRow {
    #[diesel(sql_type = Text)]
    source_identity: String,
    #[diesel(sql_type = Text)]
    source_version: String,
}

/// 构造 execution aggregate 的稳定 Event stream ID。
pub(crate) fn execution_stream_id(execution_id: Uuid) -> String {
    format!("execution/{execution_id}")
}
