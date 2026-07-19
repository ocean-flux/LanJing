//! 分区 checkpoint、可恢复 GC 与启动期 artifact recovery。
//!
//! checkpoint 只覆盖 source 或 library aggregate，绝不复制整个 `SQLite` 文件。GC 对每个未 pin
//! execution archive 固定执行 `active → marked → external_refs_removed → finalized`：先写并验证
//! checkpoint，再删 `SQLite` refs，最后删文件/metadata；任一步崩溃都可在下次调用幂等继续。artifact
//! 文件的 temp → fsync → rename 已由 artifact store 完成，metadata 失败留下的 orphan 在启动和
//! 显式 recovery 中被扫除。running execution 重启时被标为 `incomplete`，不能静默 replay 成功。

use std::collections::HashSet;

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Nullable, Text};
use diesel::sqlite::SqliteConnection;
use uuid::Uuid;

use crate::artifact::{ArtifactStore, PendingArtifact};
use crate::candidate_install::{expire_candidate, get_source_row, source_stream_id};
use crate::event_store::{
    ArtifactPathRow, ArtifactReferenceRow, BODY_KIND, I64Value, TextValue, current_global_seq,
    database_error, decrement_artifact_ref, deserialize, from_i64, now_millis, read_body_by_hash,
    retain_pending_artifact, serialize, to_i64,
};
use crate::execution::{
    ExecutionRow, events_after_source_sync, execution_from_row, execution_stream_id,
    get_execution_sync,
};
use crate::projection_query::{list_library_entries_sync, source_projection_sync};
use crate::types::{
    ArtifactKind, CheckpointReceipt, ExecutionRecord, GcReport, GcState, LibraryProjectionSnapshot,
    OrphanRecovery, ProjectionDelta, RetentionPolicy, SourceProjectionSnapshot, StorageError,
};

/// 为单个 source aggregate 写入并回读验证 checkpoint。
pub(crate) fn process_checkpoint_source(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    source_identity: &str,
    created_at_ms: i64,
) -> Result<CheckpointReceipt, StorageError> {
    let source = get_source_row(conn, source_identity)?.ok_or(StorageError::SourceMissing)?;
    let view = source_projection_sync(conn, source_identity)?;
    let global_seq = current_global_seq(conn)?;
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
    replace_source_checkpoint(conn, &pending, &snapshot, created_at_ms)?;
    Ok(CheckpointReceipt {
        aggregate_id: source_stream_id(source_identity),
        global_seq,
        artifact_hash: pending.hash,
    })
}

/// 为 library aggregate 写入小型 checkpoint；不触发全库 backup。
pub(crate) fn process_checkpoint_library(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    created_at_ms: i64,
) -> Result<CheckpointReceipt, StorageError> {
    let snapshot = LibraryProjectionSnapshot {
        global_seq: current_global_seq(conn)?,
        entries: list_library_entries_sync(conn)?,
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
    replace_library_checkpoint(conn, &pending, &snapshot, created_at_ms)?;
    Ok(CheckpointReceipt {
        aggregate_id: "library".to_string(),
        global_seq: snapshot.global_seq,
        artifact_hash: pending.hash,
    })
}

fn replace_source_checkpoint(
    conn: &mut SqliteConnection,
    pending: &PendingArtifact,
    snapshot: &SourceProjectionSnapshot,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    let old = sql_query(
        "SELECT artifact_hash AS value FROM source_checkpoints WHERE source_identity = ?",
    )
    .bind::<Text, _>(&snapshot.source_identity)
    .get_result::<TextValue>(conn)
    .optional()
    .map_err(database_error)?
    .map(|row| row.value);
    conn.immediate_transaction::<_, StorageError, _>(|conn| {
        let changed = old.as_deref() != Some(pending.hash.as_str());
        if changed {
            retain_pending_artifact(conn, pending, created_at_ms)?;
        }
        sql_query(
            "INSERT INTO source_checkpoints (source_identity, source_revision, global_seq, artifact_hash, created_at_ms) VALUES (?, ?, ?, ?, ?) ON CONFLICT(source_identity) DO UPDATE SET source_revision = excluded.source_revision, global_seq = excluded.global_seq, artifact_hash = excluded.artifact_hash, created_at_ms = excluded.created_at_ms",
        )
        .bind::<Text, _>(&snapshot.source_identity)
        .bind::<BigInt, _>(to_i64(snapshot.source_revision)?)
        .bind::<BigInt, _>(to_i64(snapshot.global_seq)?)
        .bind::<Text, _>(&pending.hash)
        .bind::<BigInt, _>(created_at_ms)
        .execute(conn)
        .map_err(database_error)?;
        if changed
            && let Some(old) = old.as_deref() {
                decrement_artifact_ref(conn, old, BODY_KIND)?;
            }
        Ok(())
    })
}

fn replace_library_checkpoint(
    conn: &mut SqliteConnection,
    pending: &PendingArtifact,
    snapshot: &LibraryProjectionSnapshot,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    let old = sql_query("SELECT artifact_hash AS value FROM library_checkpoints WHERE id = 1")
        .get_result::<TextValue>(conn)
        .optional()
        .map_err(database_error)?
        .map(|row| row.value);
    conn.immediate_transaction::<_, StorageError, _>(|conn| {
        let changed = old.as_deref() != Some(pending.hash.as_str());
        if changed {
            retain_pending_artifact(conn, pending, created_at_ms)?;
        }
        sql_query(
            "INSERT INTO library_checkpoints (id, global_seq, artifact_hash, created_at_ms) VALUES (1, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET global_seq = excluded.global_seq, artifact_hash = excluded.artifact_hash, created_at_ms = excluded.created_at_ms",
        )
        .bind::<BigInt, _>(to_i64(snapshot.global_seq)?)
        .bind::<Text, _>(&pending.hash)
        .bind::<BigInt, _>(created_at_ms)
        .execute(conn)
        .map_err(database_error)?;
        if changed
            && let Some(old) = old.as_deref() {
                decrement_artifact_ref(conn, old, BODY_KIND)?;
            }
        Ok(())
    })
}

pub(crate) fn load_source_checkpoint_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    source_identity: &str,
) -> Result<Option<SourceProjectionSnapshot>, StorageError> {
    let row = sql_query(
        "SELECT artifact_hash AS value FROM source_checkpoints WHERE source_identity = ?",
    )
    .bind::<Text, _>(source_identity)
    .get_result::<TextValue>(conn)
    .optional()
    .map_err(database_error)?;
    row.map(|row| deserialize(&read_body_by_hash(conn, artifacts, &row.value)?))
        .transpose()
}

pub(crate) fn load_library_checkpoint_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
) -> Result<Option<LibraryProjectionSnapshot>, StorageError> {
    let row = sql_query("SELECT artifact_hash AS value FROM library_checkpoints WHERE id = 1")
        .get_result::<TextValue>(conn)
        .optional()
        .map_err(database_error)?;
    row.map(|row| deserialize(&read_body_by_hash(conn, artifacts, &row.value)?))
        .transpose()
}

/// 从 source checkpoint 后的 ordered Events 重建内存视图，不写回 projection。
pub(crate) fn recover_source_snapshot_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    source_identity: &str,
) -> Result<SourceProjectionSnapshot, StorageError> {
    let mut snapshot = load_source_checkpoint_sync(conn, artifacts, source_identity)?
        .ok_or_else(|| StorageError::ReplayUnavailable("来源没有 checkpoint".to_string()))?;
    let events = events_after_source_sync(conn, source_identity, snapshot.global_seq)?;
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
            apply_delta_in_memory(&mut snapshot.delta, delta);
        }
        snapshot.global_seq = event.envelope.global_seq;
    }
    Ok(snapshot)
}

fn apply_delta_in_memory(graph: &mut lj_media::MediaGraphDelta, delta: ProjectionDelta) {
    *graph = graph.clone().merge(delta.upserts);
    graph
        .sources
        .retain(|value| !delta.tombstones.sources.contains(&value.id));
    graph
        .items
        .retain(|value| !delta.tombstones.items.contains(&value.id));
    graph
        .collections
        .retain(|value| !delta.tombstones.collections.contains(&value.id));
    graph
        .units
        .retain(|value| !delta.tombstones.units.contains(&value.id));
    graph
        .assets
        .retain(|value| !delta.tombstones.assets.contains(&value.id));
    graph
        .actions
        .retain(|value| !delta.tombstones.actions.contains(&value.id));
    graph
        .hints
        .retain(|value| !delta.tombstones.hints.contains(&value.resource_id));
    graph.relations.retain(|value| {
        !delta.tombstones.relations.iter().any(|tombstone| {
            tombstone.source_id == value.source_id
                && tombstone.from_id == value.from_id
                && tombstone.to_id == value.to_id
                && tombstone.relation_kind == serialize(&value.relation_kind).unwrap_or_default()
        })
    });
}

/// 执行 retention policy；pinned archive 永不参与自动清理。
pub(crate) fn process_gc(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    policy: RetentionPolicy,
    now_ms: i64,
) -> Result<GcReport, StorageError> {
    let mut report = GcReport::default();
    for candidate_id in expired_candidate_ids(conn, now_ms)? {
        expire_candidate(conn, candidate_id)?;
        report.expired_candidates += 1;
    }
    purge_zero_ref_artifacts(conn, artifacts)?;
    let rows = gc_execution_rows(conn)?;
    for row in rows {
        let execution = execution_from_row(row)?;
        let mut state = execution.gc_state;
        if state == GcState::Active {
            let expired = policy.archive_ttl_ms.is_some_and(|ttl| {
                execution
                    .finished_at_ms
                    .is_some_and(|finished| finished <= now_ms.saturating_sub(ttl))
            });
            let soft_quota = policy.quota_bytes.saturating_mul(90) / 100;
            let over_quota = artifact_usage(conn)? >= soft_quota;
            if !expired && !over_quota {
                continue;
            }
            mark_execution_for_gc(conn, artifacts, &execution, now_ms)?;
            report.marked += 1;
            state = GcState::Marked;
        }
        if state == GcState::Marked {
            remove_execution_external_refs(conn, execution.execution_id)?;
            report.external_refs_removed += 1;
            state = GcState::ExternalRefsRemoved;
        }
        if state == GcState::ExternalRefsRemoved {
            finalize_execution_gc(conn, artifacts, execution.execution_id)?;
            report.finalized += 1;
        }
    }
    Ok(report)
}

/// 立即清理一个 execution archive；pin archive 需要调用方显式确认。
pub(crate) fn process_clear_execution_archive(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    execution_id: Uuid,
    confirm_pinned: bool,
    now_ms: i64,
) -> Result<GcReport, StorageError> {
    let execution =
        get_execution_sync(conn, execution_id)?.ok_or(StorageError::ExecutionMissing)?;
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
        mark_execution_for_gc(conn, artifacts, &execution, now_ms)?;
        report.marked = 1;
        state = GcState::Marked;
    }
    if state == GcState::Marked {
        remove_execution_external_refs(conn, execution_id)?;
        report.external_refs_removed = 1;
        state = GcState::ExternalRefsRemoved;
    }
    if state == GcState::ExternalRefsRemoved {
        finalize_execution_gc(conn, artifacts, execution_id)?;
        report.finalized = 1;
    }
    Ok(report)
}

fn expired_candidate_ids(
    conn: &mut SqliteConnection,
    now_ms: i64,
) -> Result<Vec<Uuid>, StorageError> {
    let rows = sql_query("SELECT candidate_id AS value FROM candidates WHERE status = 'staged' AND expires_at_ms <= ? ORDER BY expires_at_ms ASC")
        .bind::<BigInt, _>(now_ms)
        .load::<TextValue>(conn)
        .map_err(database_error)?;
    rows.into_iter()
        .map(|row| {
            Uuid::parse_str(&row.value)
                .map_err(|_| StorageError::InvalidInput("损坏的 candidate ID".to_string()))
        })
        .collect()
}

fn gc_execution_rows(conn: &mut SqliteConnection) -> Result<Vec<ExecutionRow>, StorageError> {
    sql_query(
        "SELECT execution_id, source_identity, plan_hash, status, pinned, archive_available, gc_state, started_at_ms, finished_at_ms, revision FROM execution_projection WHERE pinned = 0 AND status != 'running' AND gc_state != 'finalized' ORDER BY COALESCE(finished_at_ms, started_at_ms) ASC",
    )
    .load::<ExecutionRow>(conn)
    .map_err(database_error)
}

fn artifact_usage(conn: &mut SqliteConnection) -> Result<u64, StorageError> {
    let value = sql_query("SELECT COALESCE(SUM(stored_bytes), 0) AS value FROM artifact_metadata")
        .get_result::<I64Value>(conn)
        .map_err(database_error)?;
    from_i64(value.value, "artifact usage")
}

fn mark_execution_for_gc(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    execution: &ExecutionRecord,
    now_ms: i64,
) -> Result<(), StorageError> {
    process_checkpoint_source(conn, artifacts, &execution.source_identity, now_ms)?;
    process_checkpoint_library(conn, artifacts, now_ms)?;
    sql_query("UPDATE execution_projection SET gc_state = 'marked' WHERE execution_id = ? AND gc_state = 'active'")
        .bind::<Text, _>(execution.execution_id.to_string())
        .execute(conn)
        .map_err(database_error)?;
    Ok(())
}

fn remove_execution_external_refs(
    conn: &mut SqliteConnection,
    execution_id: Uuid,
) -> Result<(), StorageError> {
    let stream_id = execution_stream_id(execution_id);
    conn.immediate_transaction::<_, StorageError, _>(|conn| {
        let refs = sql_query(
            "SELECT event_artifact_refs.hash, event_artifact_refs.artifact_kind FROM event_artifact_refs INNER JOIN events ON events.global_seq = event_artifact_refs.global_seq WHERE events.stream_id = ?",
        )
        .bind::<Text, _>(&stream_id)
        .load::<ArtifactReferenceRow>(conn)
        .map_err(database_error)?;
        for reference in refs {
            decrement_artifact_ref(conn, &reference.hash, &reference.artifact_kind)?;
        }
        sql_query("DELETE FROM event_artifact_refs WHERE global_seq IN (SELECT global_seq FROM events WHERE stream_id = ?)")
            .bind::<Text, _>(&stream_id)
            .execute(conn)
            .map_err(database_error)?;
        sql_query("UPDATE execution_projection SET gc_state = 'external_refs_removed', archive_available = 0 WHERE execution_id = ? AND gc_state = 'marked'")
            .bind::<Text, _>(execution_id.to_string())
            .execute(conn)
            .map_err(database_error)?;
        Ok(())
    })
}

fn finalize_execution_gc(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    execution_id: Uuid,
) -> Result<(), StorageError> {
    purge_zero_ref_artifacts(conn, artifacts)?;
    let stream_id = execution_stream_id(execution_id);
    conn.immediate_transaction::<_, StorageError, _>(|conn| {
        sql_query("DELETE FROM effect_captures WHERE execution_id = ?")
            .bind::<Text, _>(execution_id.to_string())
            .execute(conn)
            .map_err(database_error)?;
        sql_query("DELETE FROM events WHERE stream_id = ?")
            .bind::<Text, _>(&stream_id)
            .execute(conn)
            .map_err(database_error)?;
        sql_query("UPDATE execution_projection SET gc_state = 'finalized', archive_available = 0 WHERE execution_id = ? AND gc_state = 'external_refs_removed'")
            .bind::<Text, _>(execution_id.to_string())
            .execute(conn)
            .map_err(database_error)?;
        Ok(())
    })
}

fn purge_zero_ref_artifacts(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
) -> Result<(), StorageError> {
    let zero_ref_artifacts =
        sql_query("SELECT relative_path FROM artifact_metadata WHERE ref_count = 0")
            .load::<ArtifactPathRow>(conn)
            .map_err(database_error)?;
    for artifact in &zero_ref_artifacts {
        artifacts.remove_file(&artifact.relative_path)?;
    }
    conn.immediate_transaction::<_, StorageError, _>(|conn| {
        sql_query("DELETE FROM artifact_metadata WHERE ref_count = 0")
            .execute(conn)
            .map_err(database_error)?;
        Ok(())
    })
}

/// 将旧路径分隔符归一化，避免 Windows metadata 重启后无法定位 artifact。
pub(crate) fn normalize_artifact_relative_paths(
    conn: &mut SqliteConnection,
) -> Result<(), StorageError> {
    sql_query(
        "UPDATE artifact_metadata SET relative_path = REPLACE(relative_path, ?, ?) WHERE INSTR(relative_path, ?) > 0",
    )
    .bind::<Text, _>("\\")
    .bind::<Text, _>("/")
    .bind::<Text, _>("\\")
    .execute(conn)
    .map_err(database_error)?;
    Ok(())
}

/// 只保留 `SQLite` metadata 已认领的 artifact 路径；temp/孤儿文件可安全删除。
pub(crate) fn recover_orphans_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
) -> Result<OrphanRecovery, StorageError> {
    let rows = sql_query("SELECT relative_path AS value FROM artifact_metadata")
        .load::<TextValue>(conn)
        .map_err(database_error)?;
    let paths = rows
        .into_iter()
        .map(|row| row.value)
        .collect::<HashSet<_>>();
    artifacts.recover_orphans(&paths)
}

/// 为早期安装记录补回 source-version replay snapshot；无法验证的旧 artifact 保持显式不可 replay。
pub(crate) fn backfill_source_version_snapshots(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
) -> Result<(), StorageError> {
    let rows = sql_query(
        "SELECT source_identity, version, package_artifact_hash, profile_json, grant_json, base_url FROM source_versions WHERE profile_json IS NULL OR profile_json = '' OR grant_json IS NULL OR grant_json = '' OR base_url IS NULL OR base_url = ''",
    )
    .load::<SourceVersionBackfillRow>(conn)
    .map_err(database_error)?;
    for row in rows {
        if row.profile_json.as_deref().is_none_or(str::is_empty)
            || row.grant_json.as_deref().is_none_or(str::is_empty)
        {
            let current = sql_query(
                "SELECT profile_json, grant_json FROM source_projection WHERE source_identity = ? AND version = ?",
            )
            .bind::<Text, _>(&row.source_identity)
            .bind::<Text, _>(&row.version)
            .get_result::<SourceVersionCurrentSnapshotRow>(conn)
            .optional()
            .map_err(database_error)?;
            if let Some(current) = current {
                sql_query(
                    "UPDATE source_versions SET profile_json = CASE WHEN profile_json IS NULL OR profile_json = '' THEN ? ELSE profile_json END, grant_json = CASE WHEN grant_json IS NULL OR grant_json = '' THEN ? ELSE grant_json END WHERE source_identity = ? AND version = ?",
                )
                .bind::<Text, _>(&current.profile_json)
                .bind::<Text, _>(&current.grant_json)
                .bind::<Text, _>(&row.source_identity)
                .bind::<Text, _>(&row.version)
                .execute(conn)
                .map_err(database_error)?;
            }
        }
        if row
            .base_url
            .as_deref()
            .is_some_and(|value| !value.is_empty())
        {
            continue;
        }
        let Ok(package_bytes) = read_body_by_hash(conn, artifacts, &row.package_artifact_hash)
        else {
            continue;
        };
        let Ok(package) = deserialize::<lj_rule_model::RulePackage>(&package_bytes) else {
            continue;
        };
        if package.source_identity.id != row.source_identity
            || package.version != row.version
            || package.definition.base_url.is_empty()
        {
            continue;
        }
        sql_query(
            "UPDATE source_versions SET base_url = ? WHERE source_identity = ? AND version = ? AND (base_url IS NULL OR base_url = '')",
        )
        .bind::<Text, _>(&package.definition.base_url)
        .bind::<Text, _>(&row.source_identity)
        .bind::<Text, _>(&row.version)
        .execute(conn)
        .map_err(database_error)?;
    }
    Ok(())
}

/// 进程启动时把没有终态的 execution 标为 incomplete；不会自动触发 live/replay。
pub(crate) fn mark_interrupted_executions(conn: &mut SqliteConnection) -> Result<(), StorageError> {
    sql_query("UPDATE execution_projection SET status = 'incomplete', finished_at_ms = COALESCE(finished_at_ms, ?) WHERE status = 'running'")
        .bind::<BigInt, _>(now_millis())
        .execute(conn)
        .map_err(database_error)?;
    Ok(())
}

#[derive(QueryableByName)]
struct SourceVersionBackfillRow {
    #[diesel(sql_type = Text)]
    source_identity: String,
    #[diesel(sql_type = Text)]
    version: String,
    #[diesel(sql_type = Text)]
    package_artifact_hash: String,
    #[diesel(sql_type = Nullable<Text>)]
    profile_json: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    grant_json: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    base_url: Option<String>,
}

#[derive(QueryableByName)]
struct SourceVersionCurrentSnapshotRow {
    #[diesel(sql_type = Text)]
    profile_json: String,
    #[diesel(sql_type = Text)]
    grant_json: String,
}
