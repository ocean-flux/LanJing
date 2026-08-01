//! Maintenance aggregate repository：checkpoint、可恢复 GC 与启动期 artifact recovery。
//! GC 按 `active → marked → external_refs_removed → finalized` 单向推进；checkpoint、引用删除与
//! 文件清理由持久状态支持幂等恢复。

use std::collections::HashSet;

use crate::database::DatabaseSession;
use crate::database::OptionalResultExt;
use crate::database::statement;
use sea_orm::FromQueryResult;
use uuid::Uuid;

use crate::artifact::{ArtifactStore, PendingArtifact};
use crate::repository::candidate_source::{expire_candidate, get_source_row, source_stream_id};
use crate::repository::event::{
    ArtifactReferenceRow, BODY_KIND, I64Value, TextValue, current_global_seq, database_error,
    decrement_artifact_ref, deserialize, from_i64, read_body_by_hash, retain_pending_artifact,
    serialize, to_i64,
};
use crate::repository::execution::{
    ExecutionRow, events_after_source_sync, execution_from_row, execution_stream_id,
    get_execution_sync,
};
use crate::repository::projection::{list_library_entries_sync, source_projection_sync};
use crate::repository::secret::{purge_zero_ref_secrets, release_secret_owner};
use crate::types::{
    ArtifactKind, CheckpointReceipt, ExecutionRecord, GcReport, GcState, LibraryProjectionSnapshot,
    ProjectionDelta, RetentionPolicy, SourceProjectionSnapshot, StorageError,
};

mod recovery;
pub(crate) use recovery::{
    mark_interrupted_executions, normalize_artifact_relative_paths, purge_zero_ref_artifacts,
    recover_orphans_sync,
};

/// 为单个 source aggregate 写入并回读验证 checkpoint。
pub(crate) async fn process_checkpoint_source(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    source_identity: &str,
    created_at_ms: i64,
) -> Result<CheckpointReceipt, StorageError> {
    let source = get_source_row(conn, source_identity)
        .await?
        .ok_or(StorageError::SourceMissing)?;
    let view = source_projection_sync(conn, source_identity).await?;
    let global_seq = current_global_seq(conn).await?;
    let snapshot = SourceProjectionSnapshot {
        source_identity: source_identity.to_string(),
        global_seq,
        source_revision: from_i64(source.revision, "source revision")?,
        delta: view.delta,
    };
    let pending = artifacts.write(ArtifactKind::Body, serialize(&snapshot)?.as_bytes())?;
    let verified = deserialize::<SourceProjectionSnapshot>(
        &artifacts.read_body(&pending.hash, &pending.relative_path)?,
    )?;
    if verified != snapshot {
        return Err(StorageError::InvalidInput(
            "来源 checkpoint 验证不一致".to_string(),
        ));
    }
    replace_source_checkpoint(conn, &pending, &snapshot, created_at_ms).await?;
    Ok(CheckpointReceipt {
        aggregate_id: source_stream_id(source_identity),
        global_seq,
        artifact_hash: pending.hash,
    })
}

/// 为 library aggregate 写入小型 checkpoint；不触发全库 backup。
pub(crate) async fn process_checkpoint_library(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    created_at_ms: i64,
) -> Result<CheckpointReceipt, StorageError> {
    let snapshot = LibraryProjectionSnapshot {
        global_seq: current_global_seq(conn).await?,
        entries: list_library_entries_sync(conn).await?,
    };
    let pending = artifacts.write(ArtifactKind::Body, serialize(&snapshot)?.as_bytes())?;
    let verified = deserialize::<LibraryProjectionSnapshot>(
        &artifacts.read_body(&pending.hash, &pending.relative_path)?,
    )?;
    if verified != snapshot {
        return Err(StorageError::InvalidInput(
            "资料库 checkpoint 验证不一致".to_string(),
        ));
    }
    replace_library_checkpoint(conn, &pending, &snapshot, created_at_ms).await?;
    Ok(CheckpointReceipt {
        aggregate_id: "library".to_string(),
        global_seq: snapshot.global_seq,
        artifact_hash: pending.hash,
    })
}

async fn replace_source_checkpoint(
    conn: &mut DatabaseSession,
    pending: &PendingArtifact,
    snapshot: &SourceProjectionSnapshot,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    let old = statement(
        "SELECT artifact_hash AS value FROM source_checkpoints WHERE source_identity = ?",
    )
    .bind(&snapshot.source_identity)
    .get_result::<TextValue>(conn)
    .await
    .optional()
    .map_err(database_error)?
    .map(|row| row.value);
    let pending = pending.clone();
    let snapshot = snapshot.clone();
    conn.immediate_transaction(|conn| Box::pin(async move {
        let changed = old.as_deref() != Some(pending.hash.as_str());
        if changed {
            retain_pending_artifact(conn, &pending, created_at_ms).await?;
        }
        statement(
            "INSERT INTO source_checkpoints (source_identity, source_revision, global_seq, artifact_hash, created_at_ms) VALUES (?, ?, ?, ?, ?) ON CONFLICT(source_identity) DO UPDATE SET source_revision = excluded.source_revision, global_seq = excluded.global_seq, artifact_hash = excluded.artifact_hash, created_at_ms = excluded.created_at_ms",
        )
        .bind(&snapshot.source_identity)
        .bind(to_i64(snapshot.source_revision)?)
        .bind(to_i64(snapshot.global_seq)?)
        .bind(&pending.hash)
        .bind(created_at_ms)
        .execute(conn).await
        .map_err(database_error)?;
        if changed
            && let Some(old) = old.as_deref() {
                decrement_artifact_ref(conn, old, BODY_KIND).await?;
            }
        Ok(())
    })).await
}

async fn replace_library_checkpoint(
    conn: &mut DatabaseSession,
    pending: &PendingArtifact,
    snapshot: &LibraryProjectionSnapshot,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    let old = statement("SELECT artifact_hash AS value FROM library_checkpoints WHERE id = 1")
        .get_result::<TextValue>(conn)
        .await
        .optional()
        .map_err(database_error)?
        .map(|row| row.value);
    let pending = pending.clone();
    let snapshot = snapshot.clone();
    conn.immediate_transaction(|conn| Box::pin(async move {
        let changed = old.as_deref() != Some(pending.hash.as_str());
        if changed {
            retain_pending_artifact(conn, &pending, created_at_ms).await?;
        }
        statement(
            "INSERT INTO library_checkpoints (id, global_seq, artifact_hash, created_at_ms) VALUES (1, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET global_seq = excluded.global_seq, artifact_hash = excluded.artifact_hash, created_at_ms = excluded.created_at_ms",
        )
        .bind(to_i64(snapshot.global_seq)?)
        .bind(&pending.hash)
        .bind(created_at_ms)
        .execute(conn).await
        .map_err(database_error)?;
        if changed
            && let Some(old) = old.as_deref() {
                decrement_artifact_ref(conn, old, BODY_KIND).await?;
            }
        Ok(())
    })).await
}

pub(crate) async fn load_source_checkpoint_sync(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    source_identity: &str,
) -> Result<Option<SourceProjectionSnapshot>, StorageError> {
    let row = statement(
        "SELECT artifact_hash AS value FROM source_checkpoints WHERE source_identity = ?",
    )
    .bind(source_identity)
    .get_result::<TextValue>(conn)
    .await
    .optional()
    .map_err(database_error)?;
    match row {
        Some(row) => Ok(Some(deserialize(
            &read_body_by_hash(conn, artifacts, &row.value).await?,
        )?)),
        None => Ok(None),
    }
}

pub(crate) async fn load_library_checkpoint_sync(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
) -> Result<Option<LibraryProjectionSnapshot>, StorageError> {
    let row = statement("SELECT artifact_hash AS value FROM library_checkpoints WHERE id = 1")
        .get_result::<TextValue>(conn)
        .await
        .optional()
        .map_err(database_error)?;
    match row {
        Some(row) => Ok(Some(deserialize(
            &read_body_by_hash(conn, artifacts, &row.value).await?,
        )?)),
        None => Ok(None),
    }
}

/// 从 source checkpoint 后的 ordered Events 重建内存视图，不写回 projection。
pub(crate) async fn recover_source_snapshot_sync(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    source_identity: &str,
) -> Result<SourceProjectionSnapshot, StorageError> {
    let mut snapshot = load_source_checkpoint_sync(conn, artifacts, source_identity)
        .await?
        .ok_or_else(|| StorageError::ReplayUnavailable("来源没有 checkpoint".to_string()))?;
    let events = events_after_source_sync(conn, source_identity, snapshot.global_seq).await?;
    for event in events {
        if event
            .envelope
            .payload
            .get("kind")
            .and_then(serde_json::Value::as_str)
            == Some("delta")
        {
            let delta_value = event
                .envelope
                .payload
                .get("delta")
                .cloned()
                .ok_or(StorageError::Serialization)?;
            let delta = serde_json::from_value::<ProjectionDelta>(delta_value)
                .map_err(|_| StorageError::Serialization)?;
            crate::mapper::projection::apply_delta_in_memory(&mut snapshot.delta, delta);
        }
        snapshot.global_seq = event.envelope.global_seq;
    }
    Ok(snapshot)
}

/// 执行 retention policy；pinned archive 永不参与自动清理。
pub(crate) async fn process_gc(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    policy: RetentionPolicy,
    now_ms: i64,
) -> Result<GcReport, StorageError> {
    let mut report = GcReport::default();
    for candidate_id in expired_candidate_ids(conn, now_ms).await? {
        expire_candidate(conn, candidate_id).await?;
        report.expired_candidates += 1;
    }
    purge_unpinned_source_versions(conn).await?;
    purge_zero_ref_artifacts(conn, artifacts).await?;
    purge_zero_ref_secrets(conn, artifacts).await?;
    let rows = gc_execution_rows(conn).await?;
    let soft_quota = policy.quota_bytes.saturating_mul(90) / 100;
    let mut checkpointed_sources = HashSet::new();
    let mut library_checkpointed = false;
    for row in rows {
        let execution = execution_from_row(row)?;
        let mut state = execution.gc_state;
        if state == GcState::Active {
            let expired = policy.archive_ttl_ms.is_some_and(|ttl| {
                execution
                    .finished_at_ms
                    .is_some_and(|finished| finished <= now_ms.saturating_sub(ttl))
            });
            let over_quota = !expired && artifact_usage(conn).await? >= soft_quota;
            if !expired && !over_quota {
                continue;
            }
            if checkpointed_sources.insert(execution.source_identity.clone()) {
                process_checkpoint_source(conn, artifacts, &execution.source_identity, now_ms)
                    .await?;
            }
            if !library_checkpointed {
                process_checkpoint_library(conn, artifacts, now_ms).await?;
                library_checkpointed = true;
            }
            mark_execution_after_checkpoint(conn, execution.execution_id).await?;
            report.marked += 1;
            state = GcState::Marked;
        }
        if state == GcState::Marked {
            remove_execution_external_refs(conn, execution.execution_id).await?;
            report.external_refs_removed += 1;
            state = GcState::ExternalRefsRemoved;
        }
        if state == GcState::ExternalRefsRemoved {
            finalize_execution_gc(conn, artifacts, execution.execution_id).await?;
            report.finalized += 1;
        }
    }
    purge_unpinned_source_versions(conn).await?;
    purge_zero_ref_secrets(conn, artifacts).await?;
    Ok(report)
}

/// 立即清理一个 execution archive；pin archive 需要调用方显式确认。
pub(crate) async fn process_clear_execution_archive(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    execution_id: Uuid,
    confirm_pinned: bool,
    now_ms: i64,
) -> Result<GcReport, StorageError> {
    let execution = get_execution_sync(conn, execution_id)
        .await?
        .ok_or(StorageError::ExecutionMissing)?;
    if !execution.status.is_terminal() {
        return Err(StorageError::InvalidInput(
            "running execution 不能手动清理".to_string(),
        ));
    }
    if execution.pinned && !confirm_pinned {
        return Err(StorageError::InvalidInput(
            "pin archive 需要显式确认后清理".to_string(),
        ));
    }
    let mut report = GcReport::default();
    let mut state = execution.gc_state;
    if state == GcState::Active {
        mark_execution_for_gc(conn, artifacts, &execution, now_ms).await?;
        report.marked = 1;
        state = GcState::Marked;
    }
    if state == GcState::Marked {
        remove_execution_external_refs(conn, execution_id).await?;
        report.external_refs_removed = 1;
        state = GcState::ExternalRefsRemoved;
    }
    if state == GcState::ExternalRefsRemoved {
        finalize_execution_gc(conn, artifacts, execution_id).await?;
        report.finalized = 1;
    }
    purge_unpinned_source_versions(conn).await?;
    purge_zero_ref_secrets(conn, artifacts).await?;
    Ok(report)
}

async fn expired_candidate_ids(
    conn: &mut DatabaseSession,
    now_ms: i64,
) -> Result<Vec<Uuid>, StorageError> {
    let rows = statement("SELECT candidate_id AS value FROM candidate_projection WHERE status = 'staged' AND expires_at_ms <= ? ORDER BY expires_at_ms ASC")
        .bind(now_ms)
        .load::<TextValue>(conn).await
        .map_err(database_error)?;
    rows.into_iter()
        .map(|row| {
            Uuid::parse_str(&row.value)
                .map_err(|_| StorageError::InvalidInput("损坏的 candidate ID".to_string()))
        })
        .collect()
}

async fn gc_execution_rows(conn: &mut DatabaseSession) -> Result<Vec<ExecutionRow>, StorageError> {
    statement(
        "SELECT execution_id, source_identity, source_revision, plan_hash, status, pinned, archive_available, gc_state, started_at_ms, finished_at_ms, revision FROM execution_projection WHERE pinned = 0 AND status != 'running' AND gc_state != 'finalized' ORDER BY COALESCE(finished_at_ms, started_at_ms) ASC",
    )
    .load::<ExecutionRow>(conn).await
    .map_err(database_error)
}

async fn purge_unpinned_source_versions(conn: &mut DatabaseSession) -> Result<(), StorageError> {
    let rows = statement(
        "SELECT versions.source_identity, versions.source_revision FROM source_versions AS versions WHERE NOT EXISTS (SELECT 1 FROM source_projection AS current_source WHERE current_source.source_identity = versions.source_identity AND current_source.revision = versions.source_revision) AND NOT EXISTS (SELECT 1 FROM execution_projection AS execution WHERE execution.source_identity = versions.source_identity AND execution.source_revision = versions.source_revision AND execution.archive_available = 1) ORDER BY versions.source_identity, versions.source_revision",
    )
    .load::<SourceVersionKeyRow>(conn).await
    .map_err(database_error)?;
    conn.immediate_transaction(|conn| {
        Box::pin(async move {
            for row in &rows {
                let source_revision = from_i64(row.source_revision, "source revision")?;
                let owner_id = format!("{}:{source_revision}", row.source_identity);
                release_secret_owner(conn, "source_version_runtime", &owner_id).await?;
                statement(
                    "DELETE FROM source_versions WHERE source_identity = ? AND source_revision = ?",
                )
                .bind(&row.source_identity)
                .bind(row.source_revision)
                .execute(conn)
                .await
                .map_err(database_error)?;
            }
            Ok(())
        })
    })
    .await
}

async fn artifact_usage(conn: &mut DatabaseSession) -> Result<u64, StorageError> {
    let value = statement("SELECT (SELECT COALESCE(SUM(stored_bytes), 0) FROM artifact_metadata) + (SELECT COALESCE(SUM(stored_bytes), 0) FROM secret_artifact_projection) AS value")
        .get_result::<I64Value>(conn).await
        .map_err(database_error)?;
    from_i64(value.value, "artifact usage")
}

async fn mark_execution_for_gc(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    execution: &ExecutionRecord,
    now_ms: i64,
) -> Result<(), StorageError> {
    process_checkpoint_source(conn, artifacts, &execution.source_identity, now_ms).await?;
    process_checkpoint_library(conn, artifacts, now_ms).await?;
    mark_execution_after_checkpoint(conn, execution.execution_id).await
}

async fn mark_execution_after_checkpoint(
    conn: &mut DatabaseSession,
    execution_id: Uuid,
) -> Result<(), StorageError> {
    statement("UPDATE execution_projection SET gc_state = 'marked' WHERE execution_id = ? AND gc_state = 'active'")
        .bind(execution_id.to_string())
        .execute(conn).await
        .map_err(database_error)?;
    Ok(())
}

async fn remove_execution_external_refs(
    conn: &mut DatabaseSession,
    execution_id: Uuid,
) -> Result<(), StorageError> {
    let stream_id = execution_stream_id(execution_id);
    conn.immediate_transaction(|conn| Box::pin(async move {
        let refs = statement(
            "SELECT event_artifact_refs.hash, event_artifact_refs.artifact_kind FROM event_artifact_refs INNER JOIN events ON events.global_seq = event_artifact_refs.global_seq WHERE events.stream_id = ?",
        )
        .bind(&stream_id)
        .load::<ArtifactReferenceRow>(conn).await
        .map_err(database_error)?;
        for reference in refs {
            decrement_artifact_ref(conn, &reference.hash, &reference.artifact_kind).await?;
        }
        statement("DELETE FROM event_artifact_refs WHERE global_seq IN (SELECT global_seq FROM events WHERE stream_id = ?)")
            .bind(&stream_id)
            .execute(conn).await
            .map_err(database_error)?;
        statement("UPDATE execution_projection SET gc_state = 'external_refs_removed', archive_available = 0 WHERE execution_id = ? AND gc_state = 'marked'")
            .bind(execution_id.to_string())
            .execute(conn).await
            .map_err(database_error)?;
        Ok(())
    })).await
}

async fn finalize_execution_gc(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    execution_id: Uuid,
) -> Result<(), StorageError> {
    purge_zero_ref_artifacts(conn, artifacts).await?;
    let stream_id = execution_stream_id(execution_id);
    conn.immediate_transaction(|conn| Box::pin(async move {
        let effect_ids = statement(
            "SELECT effect_id AS value FROM effect_captures WHERE execution_id = ?",
        )
        .bind(execution_id.to_string())
        .load::<TextValue>(conn).await
        .map_err(database_error)?;
        for effect_id in effect_ids {
            let owner_id = format!("{execution_id}:{}", effect_id.value);
            release_secret_owner(conn, "effect_response_headers", &owner_id).await?;
            release_secret_owner(conn, "effect_request_body", &owner_id).await?;
        }
        statement("DELETE FROM effect_captures WHERE execution_id = ?")
            .bind(execution_id.to_string())
            .execute(conn).await
            .map_err(database_error)?;
        statement("DELETE FROM control_traces WHERE execution_id = ?")
            .bind(execution_id.to_string())
            .execute(conn).await
            .map_err(database_error)?;
        statement("DELETE FROM execution_invocation_ledger WHERE execution_id = ?")
            .bind(execution_id.to_string())
            .execute(conn).await
            .map_err(database_error)?;
        statement("DELETE FROM events WHERE stream_id = ?")
            .bind(&stream_id)
            .execute(conn).await
            .map_err(database_error)?;
        statement("UPDATE execution_projection SET gc_state = 'finalized', archive_available = 0 WHERE execution_id = ? AND gc_state = 'external_refs_removed'")
            .bind(execution_id.to_string())
            .execute(conn).await
            .map_err(database_error)?;
        Ok(())
    })).await
}

#[derive(FromQueryResult)]
struct SourceVersionKeyRow {
    source_identity: String,
    source_revision: i64,
}
