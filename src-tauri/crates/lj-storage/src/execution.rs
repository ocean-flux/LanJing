//! Execution archive 生命周期、revision pin 与事件 catch-up。
//!
//! live start 由单 writer transaction 同时追加 Started Event 和固定 `(source identity,
//! source revision)`；成功只返回 [`ExecutionStartReceipt`]，其中 installed snapshot 与 pin 来自
//! 同一 immutable `source_versions` 行。legacy execution 只有具备唯一历史证据时才可回填 pin，
//! 否则保持 typed unavailable，绝不读取 current source 猜测。

use std::str::FromStr;

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Integer, Nullable, Text};
use diesel::sqlite::SqliteConnection;
use lj_media::SourceProfile;
use lj_rule_model::{EventType, PolicyCapabilities};
use lj_runtime::ExecutionMode;
use uuid::Uuid;

use crate::artifact::ArtifactStore;
use crate::candidate_install::{
    canonical_plan_hash, get_source_row, grant_covers, read_execution_plan_artifact,
    read_rule_package_artifact, source_cookie_namespace, source_version_owner_id,
    validate_candidate_package_and_plan,
};
use crate::event_store::{
    ArtifactLink, EventDraft, append_event_transaction, database_error, deserialize,
    ensure_blake3_hash, from_i64, idempotent_event, read_body_by_hash, serialize,
    stored_event_from_row, to_i64,
};
use crate::projection_query::{apply_projection_delta, validate_delta_source};
use crate::secret_artifact::read_owned_secret;
use crate::types::{
    ArtifactKind, CommitReceipt, DeltaCommit, ExecutionFinish, ExecutionPin, ExecutionRecord,
    ExecutionReplayPin, ExecutionSourceCredentials, ExecutionStart, ExecutionStartReceipt,
    ExecutionStatus, GcState, InstalledSourceSnapshot, ReplayExecutionStart,
    ReplayUnavailableReason, SecretArtifactId, StorageError, StoredEvent,
};

struct LoadedSourceVersion {
    snapshot: InstalledSourceSnapshot,
    package_artifact_hash: String,
    plan_artifact_hash: String,
    definition_hash: String,
    plan_hash: String,
}

/// 在一个 writer transaction 中持久化 Started Event + source revision pin。
pub(crate) fn process_start_execution(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    request: ExecutionStart,
) -> Result<ExecutionStartReceipt, StorageError> {
    let existing = get_execution_sync(conn, request.execution_id)?;
    let source_revision = if let Some(record) = &existing {
        if record.source_identity != request.source_identity {
            return Err(StorageError::IdempotencyMismatch);
        }
        record.source_revision.ok_or_else(|| {
            StorageError::ReplayUnavailable(
                "legacy execution 缺少可证明的 source revision".to_string(),
            )
        })?
    } else {
        let source =
            get_source_row(conn, &request.source_identity)?.ok_or(StorageError::SourceMissing)?;
        from_i64(source.revision, "source revision")?
    };
    let loaded = load_source_version(conn, artifacts, &request.source_identity, source_revision)?;
    let event = live_started_event(&request, &loaded, source_revision);
    let links = vec![
        ArtifactLink::Existing {
            hash: loaded.package_artifact_hash.clone(),
            kind: ArtifactKind::Body,
        },
        ArtifactLink::Existing {
            hash: loaded.plan_artifact_hash.clone(),
            kind: ArtifactKind::Body,
        },
    ];
    if let Some(receipt) = idempotent_event(conn, &event)? {
        let mut record = existing.ok_or(StorageError::ExecutionMissing)?;
        record.revision = receipt.stream_version;
        return Ok(ExecutionStartReceipt {
            record,
            installed_snapshot: loaded.snapshot,
        });
    }
    if existing.is_some() {
        return Err(StorageError::IdempotencyMismatch);
    }
    let execution_id = request.execution_id;
    let source_identity = request.source_identity;
    let source_version = loaded.snapshot.version.clone();
    let plan_hash = loaded.plan_hash.clone();
    let plan_artifact_hash = loaded.plan_artifact_hash.clone();
    let started_at_ms = request.started_at_ms;
    append_event_transaction(conn, &event, &links, move |conn, global_seq, revision| {
        sql_query(
            "INSERT INTO execution_projection (execution_id, source_identity, source_version, source_revision, plan_hash, plan_artifact_hash, status, pinned, archive_available, replay_unavailable_reason, gc_state, started_at_ms, finished_at_ms, revision, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?, 'running', 0, 1, NULL, ?, ?, NULL, ?, ?)",
        )
        .bind::<Text, _>(execution_id.to_string())
        .bind::<Text, _>(&source_identity)
        .bind::<Text, _>(&source_version)
        .bind::<BigInt, _>(to_i64(source_revision)?)
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
    let record =
        get_execution_sync(conn, request.execution_id)?.ok_or(StorageError::ExecutionMissing)?;
    Ok(ExecutionStartReceipt {
        record,
        installed_snapshot: loaded.snapshot,
    })
}

fn live_started_event(
    request: &ExecutionStart,
    source: &LoadedSourceVersion,
    source_revision: u64,
) -> EventDraft {
    EventDraft {
        stream_id: execution_stream_id(request.execution_id),
        expected_version: 0,
        event_id: request.event_id,
        event_type: EventType::Execution,
        schema_version: 2,
        correlation_id: request.correlation_id,
        causation_id: None,
        trace_id: request.trace_id.clone(),
        occurred_at_ms: request.started_at_ms,
        payload: serde_json::json!({
            "kind": "started",
            "execution_id": request.execution_id,
            "source_identity": request.source_identity,
            "source_revision": source_revision,
            "source_version": source.snapshot.version,
            "definition_hash": source.definition_hash,
            "plan_hash": source.plan_hash,
            "plan_artifact_hash": source.plan_artifact_hash,
            "package_artifact_hash": source.package_artifact_hash,
        }),
        source_identity: Some(request.source_identity.clone()),
    }
}

/// 用已验证 historical revision pin 创建 replay execution；不会读取 current source。
pub(crate) fn process_start_replay_execution(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    request: &ReplayExecutionStart,
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
        schema_version: 2,
        correlation_id: request.correlation_id,
        causation_id: Some(canonical_pin.execution_id),
        trace_id: request.trace_id.clone(),
        occurred_at_ms: request.started_at_ms,
        payload: serde_json::json!({
            "kind": "replay_started",
            "execution_id": request.execution_id,
            "archived_execution_id": canonical_pin.execution_id,
            "source_identity": canonical_pin.source_identity,
            "source_revision": canonical_pin.source_revision,
            "source_version": canonical_pin.source_version,
            "plan_hash": canonical_pin.plan_hash,
            "plan_artifact_hash": canonical_pin.plan_artifact_hash,
            "package_artifact_hash": canonical_pin.package_artifact_hash,
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
    if get_execution_sync(conn, request.execution_id)?.is_some() {
        return Err(StorageError::IdempotencyMismatch);
    }
    let execution_id = request.execution_id;
    let source_identity = canonical_pin.source_identity;
    let source_version = canonical_pin.source_version;
    let source_revision = canonical_pin.source_revision;
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
            "INSERT INTO execution_projection (execution_id, source_identity, source_version, source_revision, plan_hash, plan_artifact_hash, status, pinned, archive_available, replay_unavailable_reason, gc_state, started_at_ms, finished_at_ms, revision, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?, 'running', 0, 1, NULL, ?, ?, NULL, ?, ?)",
        )
        .bind::<Text, _>(execution_id.to_string())
        .bind::<Text, _>(&source_identity)
        .bind::<Text, _>(&source_version)
        .bind::<BigInt, _>(to_i64(source_revision)?)
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

/// 将 execution Delta 与 O(delta) projection 更新原子提交。
pub(crate) fn process_delta(
    conn: &mut SqliteConnection,
    request: DeltaCommit,
) -> Result<CommitReceipt, StorageError> {
    let execution =
        get_execution_sync(conn, request.execution_id)?.ok_or(StorageError::ExecutionMissing)?;
    let source_revision = execution.source_revision.ok_or_else(|| {
        StorageError::ReplayUnavailable(
            "legacy execution 缺少可证明的 source revision，不能继续提交".to_string(),
        )
    })?;
    validate_delta_source(conn, &request.delta, &execution.source_identity)?;
    let payload = serialize(&request.delta)?;
    let event = EventDraft {
        stream_id: execution_stream_id(request.execution_id),
        expected_version: request.expected_version,
        event_id: request.event_id,
        event_type: EventType::Execution,
        schema_version: 2,
        correlation_id: None,
        causation_id: None,
        trace_id: request.trace_id,
        occurred_at_ms: request.occurred_at_ms,
        payload: serde_json::json!({
            "kind": "delta",
            "source_revision": source_revision,
            "delta": serde_json::from_str::<serde_json::Value>(&payload)
                .map_err(|_| StorageError::Serialization)?,
        }),
        source_identity: Some(execution.source_identity.clone()),
    };
    if let Some(receipt) = idempotent_event(conn, &event)? {
        return Ok(receipt);
    }
    if execution.status.is_terminal() {
        return Err(StorageError::InvalidInput(
            "终态 execution 不能提交 Delta".to_string(),
        ));
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
    let already_terminal = execution.status.is_terminal();
    let event = EventDraft {
        stream_id: execution_stream_id(request.execution_id),
        expected_version: request.expected_version,
        event_id: request.event_id,
        event_type: EventType::Execution,
        schema_version: 2,
        correlation_id: None,
        causation_id: None,
        trace_id: request.trace_id,
        occurred_at_ms: request.finished_at_ms,
        payload: serde_json::json!({
            "kind": "terminal",
            "status": request.status.as_db(),
            "source_revision": execution.source_revision,
        }),
        source_identity: Some(execution.source_identity),
    };
    if idempotent_event(conn, &event)?.is_some() {
        return get_execution_sync(conn, request.execution_id)?
            .ok_or(StorageError::ExecutionMissing);
    }
    if already_terminal {
        return Err(StorageError::InvalidInput(
            "execution 已处于终态".to_string(),
        ));
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

/// 修改 archive pin；retention 仅处理未 pin archive。
pub(crate) fn process_pin_execution(
    conn: &mut SqliteConnection,
    request: ExecutionPin,
) -> Result<ExecutionRecord, StorageError> {
    let execution =
        get_execution_sync(conn, request.execution_id)?.ok_or(StorageError::ExecutionMissing)?;
    let gc_state = execution.gc_state;
    let event = EventDraft {
        stream_id: execution_stream_id(request.execution_id),
        expected_version: request.expected_version,
        event_id: request.event_id,
        event_type: EventType::Execution,
        schema_version: 2,
        correlation_id: None,
        causation_id: None,
        trace_id: request.trace_id,
        occurred_at_ms: request.occurred_at_ms,
        payload: serde_json::json!({
            "kind": "pin",
            "pinned": request.pinned,
            "source_revision": execution.source_revision,
        }),
        source_identity: Some(execution.source_identity),
    };
    if idempotent_event(conn, &event)?.is_some() {
        return get_execution_sync(conn, request.execution_id)?
            .ok_or(StorageError::ExecutionMissing);
    }
    if gc_state != GcState::Active {
        return Err(StorageError::ReplayUnavailable(
            "GC 已开始的 archive 不能修改 pin".to_string(),
        ));
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

/// 更新 execution summary revision；只可从同一个 Event transaction closure 调用。
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

/// 读取 execution summary；未知状态或 typed reason 损坏时失败。
pub(crate) fn get_execution_sync(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
) -> Result<Option<ExecutionRecord>, StorageError> {
    let row = sql_query(
        "SELECT execution_id, source_identity, source_revision, plan_hash, status, pinned, archive_available, replay_unavailable_reason, gc_state, started_at_ms, finished_at_ms, revision FROM execution_projection WHERE execution_id = ?",
    )
    .bind::<Text, _>(execution_id.to_string())
    .get_result::<ExecutionRow>(conn)
    .optional()
    .map_err(database_error)?;
    row.map(execution_from_row).transpose()
}

/// 从 `SQLite` row 转换公开 execution summary。
pub(crate) fn execution_from_row(row: ExecutionRow) -> Result<ExecutionRecord, StorageError> {
    let replay_unavailable_reason = row
        .replay_unavailable_reason
        .as_deref()
        .map(ReplayUnavailableReason::from_db)
        .transpose()?;
    let source_revision = row
        .source_revision
        .map(|value| from_i64(value, "source revision"))
        .transpose()?;
    if source_revision.is_some() == replay_unavailable_reason.is_some() {
        return Err(StorageError::InvalidInput(
            "execution replay pin 状态不一致".to_string(),
        ));
    }
    Ok(ExecutionRecord {
        execution_id: Uuid::parse_str(&row.execution_id)
            .map_err(|_| StorageError::InvalidInput("损坏的 execution ID".to_string()))?,
        source_identity: row.source_identity,
        source_revision,
        plan_hash: row.plan_hash,
        status: ExecutionStatus::from_db(&row.status)?,
        pinned: row.pinned != 0,
        replayable: row.archive_available != 0 && replay_unavailable_reason.is_none(),
        replay_unavailable_reason,
        gc_state: GcState::from_db(&row.gc_state)?,
        started_at_ms: row.started_at_ms,
        finished_at_ms: row.finished_at_ms,
        revision: from_i64(row.revision, "execution revision")?,
    })
}

/// 加载并完整验证 historical revision pin；不能以 current source 替代。
pub(crate) fn load_execution_replay_pin_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    execution_id: Uuid,
) -> Result<ExecutionReplayPin, StorageError> {
    let row = sql_query(
        "SELECT execution_id, source_identity, source_version, source_revision, plan_hash, plan_artifact_hash, archive_available, replay_unavailable_reason, gc_state FROM execution_projection WHERE execution_id = ?",
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
    if let Some(reason) = row.replay_unavailable_reason.as_deref() {
        let reason = ReplayUnavailableReason::from_db(reason)?;
        return Err(StorageError::ReplayUnavailable(format!(
            "execution archive 无可证明 source pin: {}",
            reason.as_db()
        )));
    }
    let source_revision = row
        .source_revision
        .map(|value| from_i64(value, "source revision"))
        .transpose()?
        .ok_or_else(|| {
            StorageError::ReplayUnavailable("execution 缺少 source revision pin".to_string())
        })?;
    let loaded = load_source_version(conn, artifacts, &row.source_identity, source_revision)
        .map_err(map_replay_snapshot_error)?;
    if loaded.snapshot.version != row.source_version
        || loaded.plan_hash != row.plan_hash
        || loaded.plan_artifact_hash != row.plan_artifact_hash
    {
        return Err(StorageError::ReplayUnavailable(
            "execution pin 与 immutable source snapshot 不一致".to_string(),
        ));
    }
    let execution_id = Uuid::parse_str(&row.execution_id)
        .map_err(|_| StorageError::InvalidInput("损坏的 execution ID".to_string()))?;
    Ok(ExecutionReplayPin {
        execution_id,
        source_identity: row.source_identity,
        source_revision,
        source_version: loaded.snapshot.version,
        profile: loaded.snapshot.profile,
        grant: loaded.snapshot.grant,
        base_url: loaded.snapshot.base_url,
        package_artifact_hash: loaded.package_artifact_hash,
        plan: loaded.snapshot.plan,
        plan_hash: loaded.plan_hash,
        plan_artifact_hash: loaded.plan_artifact_hash,
        mode: ExecutionMode::Replay {
            archived_execution_id: execution_id,
        },
    })
}

fn map_replay_snapshot_error(error: StorageError) -> StorageError {
    match error {
        StorageError::KeyringLocked
        | StorageError::KeyringUnavailable
        | StorageError::KeyLost
        | StorageError::ContractSchemaUnsupported { .. }
        | StorageError::LegacyRuleContractUnsupported { .. } => error,
        _ => StorageError::ReplayUnavailable(
            "execution pin source snapshot 缺失、损坏或不一致".to_string(),
        ),
    }
}

fn load_source_version(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    source_identity: &str,
    source_revision: u64,
) -> Result<LoadedSourceVersion, StorageError> {
    let row = sql_query(
        "SELECT source_identity, source_revision, version, profile_json, grant_json, base_url, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, cookie_namespace, runtime_credential_secret_id FROM source_versions WHERE source_identity = ? AND source_revision = ?",
    )
    .bind::<Text, _>(source_identity)
    .bind::<BigInt, _>(to_i64(source_revision)?)
    .get_result::<SourceVersionRow>(conn)
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::SourceMissing)?;
    if row.source_identity != source_identity
        || from_i64(row.source_revision, "source revision")? != source_revision
        || row.cookie_namespace.is_empty()
    {
        return Err(StorageError::ArtifactCorrupt);
    }
    ensure_blake3_hash(&row.package_artifact_hash, "package artifact hash")?;
    ensure_blake3_hash(&row.plan_artifact_hash, "Plan artifact hash")?;
    ensure_blake3_hash(&row.definition_hash, "definition hash")?;
    ensure_blake3_hash(&row.plan_hash, "Plan hash")?;
    let package = read_rule_package_artifact(&read_body_by_hash(
        conn,
        artifacts,
        &row.package_artifact_hash,
    )?)?;
    let plan = read_execution_plan_artifact(&read_body_by_hash(
        conn,
        artifacts,
        &row.plan_artifact_hash,
    )?)?;
    let profile = deserialize::<SourceProfile>(row.profile_json.as_bytes())?;
    let grant = deserialize::<PolicyCapabilities>(row.grant_json.as_bytes())?;
    if package.source_identity().id != source_identity
        || package.version() != row.version
        || package.definition().base_url() != row.base_url
        || !grant_covers(&grant, &package.definition().capability_manifest().required)
        || profile.id.0 != source_identity
        || profile.version.as_deref() != Some(row.version.as_str())
        || plan.definition_hash() != row.definition_hash
        || plan.plan_hash() != row.plan_hash
        || canonical_plan_hash(&plan)? != row.plan_hash
    {
        return Err(StorageError::ArtifactCorrupt);
    }
    validate_candidate_package_and_plan(&package, &plan)
        .map_err(|_| StorageError::ArtifactCorrupt)?;
    let secret_bytes = row
        .runtime_credential_secret_id
        .as_deref()
        .map(SecretArtifactId::from_str)
        .transpose()?
        .map(|secret_id| {
            read_owned_secret(
                conn,
                artifacts,
                secret_id,
                "source_version_runtime",
                &source_version_owner_id(source_identity, source_revision),
            )
        })
        .transpose()?;
    Ok(LoadedSourceVersion {
        snapshot: InstalledSourceSnapshot {
            source_identity: source_identity.to_string(),
            source_revision,
            version: row.version,
            profile,
            grant,
            base_url: row.base_url,
            package,
            plan,
            runtime_credentials: ExecutionSourceCredentials::new(
                row.cookie_namespace,
                secret_bytes,
            ),
        },
        package_artifact_hash: row.package_artifact_hash,
        plan_artifact_hash: row.plan_artifact_hash,
        definition_hash: row.definition_hash,
        plan_hash: row.plan_hash,
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

/// 仅为 live execution 解密固定 source-revision credential；replay 显式失败。
pub(crate) fn load_execution_source_credentials_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    execution_id: Uuid,
) -> Result<ExecutionSourceCredentials, StorageError> {
    let execution = sql_query(
        "SELECT source_identity, source_revision FROM execution_projection WHERE execution_id = ?",
    )
    .bind::<Text, _>(execution_id.to_string())
    .get_result::<ExecutionSourceRevisionRow>(conn)
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::ExecutionMissing)?;
    if execution_is_replay_sync(conn, execution_id)? {
        return Err(StorageError::ReplayUnavailable(
            "replay execution 不传递 source credential".to_string(),
        ));
    }
    let source_revision = execution
        .source_revision
        .map(|value| from_i64(value, "source revision"))
        .transpose()?
        .ok_or(StorageError::SourceCredentialUnavailable)?;
    let row = sql_query(
        "SELECT cookie_namespace, runtime_credential_secret_id FROM source_versions WHERE source_identity = ? AND source_revision = ?",
    )
    .bind::<Text, _>(&execution.source_identity)
    .bind::<BigInt, _>(to_i64(source_revision)?)
    .get_result::<SourceVersionCredentialRow>(conn)
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::SourceCredentialUnavailable)?;
    let cookie_namespace = if row.cookie_namespace.is_empty() {
        source_cookie_namespace(&execution.source_identity)
    } else {
        row.cookie_namespace
    };
    let secret_bytes = row
        .runtime_credential_secret_id
        .as_deref()
        .map(SecretArtifactId::from_str)
        .transpose()?
        .map(|secret_id| {
            read_owned_secret(
                conn,
                artifacts,
                secret_id,
                "source_version_runtime",
                &source_version_owner_id(&execution.source_identity, source_revision),
            )
        })
        .transpose()?;
    Ok(ExecutionSourceCredentials::new(
        cookie_namespace,
        secret_bytes,
    ))
}

/// 按 stream revision 有序读取 durable events。
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

/// 按 source/global sequence 有序读取 events。
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
struct SourceVersionRow {
    #[diesel(sql_type = Text)]
    source_identity: String,
    #[diesel(sql_type = BigInt)]
    source_revision: i64,
    #[diesel(sql_type = Text)]
    version: String,
    #[diesel(sql_type = Text)]
    profile_json: String,
    #[diesel(sql_type = Text)]
    grant_json: String,
    #[diesel(sql_type = Text)]
    base_url: String,
    #[diesel(sql_type = Text)]
    package_artifact_hash: String,
    #[diesel(sql_type = Text)]
    plan_artifact_hash: String,
    #[diesel(sql_type = Text)]
    definition_hash: String,
    #[diesel(sql_type = Text)]
    plan_hash: String,
    #[diesel(sql_type = Text)]
    cookie_namespace: String,
    #[diesel(sql_type = Nullable<Text>)]
    runtime_credential_secret_id: Option<String>,
}

#[derive(QueryableByName)]
struct SourceVersionCredentialRow {
    #[diesel(sql_type = Text)]
    cookie_namespace: String,
    #[diesel(sql_type = Nullable<Text>)]
    runtime_credential_secret_id: Option<String>,
}

#[derive(QueryableByName)]
pub(crate) struct ExecutionRow {
    #[diesel(sql_type = Text)]
    pub(crate) execution_id: String,
    #[diesel(sql_type = Text)]
    pub(crate) source_identity: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub(crate) source_revision: Option<i64>,
    #[diesel(sql_type = Text)]
    pub(crate) plan_hash: String,
    #[diesel(sql_type = Text)]
    pub(crate) status: String,
    #[diesel(sql_type = Integer)]
    pub(crate) pinned: i32,
    #[diesel(sql_type = Integer)]
    pub(crate) archive_available: i32,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) replay_unavailable_reason: Option<String>,
    #[diesel(sql_type = Text)]
    pub(crate) gc_state: String,
    #[diesel(sql_type = BigInt)]
    pub(crate) started_at_ms: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub(crate) finished_at_ms: Option<i64>,
    #[diesel(sql_type = BigInt)]
    pub(crate) revision: i64,
}

#[derive(QueryableByName)]
struct ExecutionReplayPinRow {
    #[diesel(sql_type = Text)]
    execution_id: String,
    #[diesel(sql_type = Text)]
    source_identity: String,
    #[diesel(sql_type = Text)]
    source_version: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_revision: Option<i64>,
    #[diesel(sql_type = Text)]
    plan_hash: String,
    #[diesel(sql_type = Text)]
    plan_artifact_hash: String,
    #[diesel(sql_type = Integer)]
    archive_available: i32,
    #[diesel(sql_type = Nullable<Text>)]
    replay_unavailable_reason: Option<String>,
    #[diesel(sql_type = Text)]
    gc_state: String,
}

#[derive(QueryableByName)]
struct ExecutionSourceRevisionRow {
    #[diesel(sql_type = Text)]
    source_identity: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_revision: Option<i64>,
}

/// execution aggregate 稳定 Event stream ID。
pub(crate) fn execution_stream_id(execution_id: Uuid) -> String {
    format!("execution/{execution_id}")
}
