struct LoadedSourceVersion {
    snapshot: InstalledSourceSnapshot,
    package_artifact_hash: String,
    plan_artifact_hash: String,
    definition_hash: String,
    plan_hash: String,
}

/// 在一个 writer transaction 中持久化 Started Event + source revision pin。
pub(crate) async fn process_start_execution(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    request: ExecutionStart,
) -> Result<ExecutionStartReceipt, StorageError> {
    let existing = get_execution_sync(conn, request.execution_id).await?;
    let source_revision = if let Some(record) = &existing {
        if record.source_identity != request.source_identity {
            return Err(StorageError::IdempotencyMismatch);
        }
        record.source_revision
    } else {
        let source =
            get_source_row(conn, &request.source_identity).await?.ok_or(StorageError::SourceMissing)?;
        from_i64(source.revision, "source revision")?
    };
    let loaded = load_source_version(conn, artifacts, &request.source_identity, source_revision).await?;
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
    if let Some(receipt) = idempotent_event(conn, &event).await? {
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
    append_event_transaction(conn, &event, &links, move |conn, global_seq, revision| Box::pin(async move {
        statement(
            "INSERT INTO execution_projection (execution_id, source_identity, source_version, source_revision, plan_hash, plan_artifact_hash, status, pinned, archive_available, gc_state, started_at_ms, finished_at_ms, revision, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?, 'running', 0, 1, ?, ?, NULL, ?, ?)",
        )
        .bind(execution_id.to_string())
        .bind(&source_identity)
        .bind(&source_version)
        .bind(to_i64(source_revision)?)
        .bind(&plan_hash)
        .bind(&plan_artifact_hash)
        .bind(GcState::Active.as_db())
        .bind(started_at_ms)
        .bind(to_i64(revision)?)
        .bind(to_i64(global_seq)?)
        .execute(conn).await
        .map_err(database_error)?;
        Ok(())
    })).await?;
    let record =
        get_execution_sync(conn, request.execution_id).await?.ok_or(StorageError::ExecutionMissing)?;
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
pub(crate) async fn process_start_replay_execution(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    request: &ReplayExecutionStart,
) -> Result<ExecutionRecord, StorageError> {
    let canonical_pin = load_execution_replay_pin_sync(conn, artifacts, request.pin.execution_id).await?;
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
    if let Some(receipt) = idempotent_event(conn, &event).await? {
        return get_execution_sync(conn, request.execution_id).await?
            .ok_or(StorageError::ExecutionMissing)
            .map(|mut value| {
                value.revision = receipt.stream_version;
                value
            });
    }
    if get_execution_sync(conn, request.execution_id).await?.is_some() {
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
    append_event_transaction(conn, &event, &links, move |conn, global_seq, revision| Box::pin(async move {
        statement(
            "INSERT INTO execution_projection (execution_id, source_identity, source_version, source_revision, plan_hash, plan_artifact_hash, status, pinned, archive_available, gc_state, started_at_ms, finished_at_ms, revision, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?, 'running', 0, 1, ?, ?, NULL, ?, ?)",
        )
        .bind(execution_id.to_string())
        .bind(&source_identity)
        .bind(&source_version)
        .bind(to_i64(source_revision)?)
        .bind(&plan_hash)
        .bind(&plan_artifact_hash)
        .bind(GcState::Active.as_db())
        .bind(started_at_ms)
        .bind(to_i64(revision)?)
        .bind(to_i64(global_seq)?)
        .execute(conn).await
        .map_err(database_error)?;
        Ok(())
    })).await?;
    get_execution_sync(conn, request.execution_id).await?.ok_or(StorageError::ExecutionMissing)
}
