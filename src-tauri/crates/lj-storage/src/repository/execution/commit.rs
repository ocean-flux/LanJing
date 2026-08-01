/// 将 execution Delta 与 O(delta) projection 更新原子提交。
pub(crate) async fn process_delta(
    conn: &mut DatabaseSession,
    request: DeltaCommit,
) -> Result<CommitReceipt, StorageError> {
    let execution =
        get_execution_sync(conn, request.execution_id).await?.ok_or(StorageError::ExecutionMissing)?;
    let source_revision = execution.source_revision;
    validate_delta_source(conn, &request.delta, &execution.source_identity).await?;
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
    if let Some(receipt) = idempotent_event(conn, &event).await? {
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
    append_event_transaction(conn, &event, &[], move |conn, global_seq, revision| Box::pin(async move {
        apply_projection_delta(conn, &source_identity, &delta, global_seq).await?;
        update_execution_revision(conn, execution_id, revision, global_seq).await?;
        Ok(())
    })).await
}

/// 写入 execution 唯一终态；不同终态的重复写入被拒绝。
pub(crate) async fn process_finish_execution(
    conn: &mut DatabaseSession,
    request: ExecutionFinish,
) -> Result<ExecutionRecord, StorageError> {
    if !request.status.is_terminal() {
        return Err(StorageError::InvalidInput(
            "execution 终态不能为 running".to_string(),
        ));
    }
    let execution =
        get_execution_sync(conn, request.execution_id).await?.ok_or(StorageError::ExecutionMissing)?;
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
    if idempotent_event(conn, &event).await?.is_some() {
        return get_execution_sync(conn, request.execution_id).await?
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
    append_event_transaction(conn, &event, &[], move |conn, global_seq, revision| Box::pin(async move {
        statement("UPDATE execution_projection SET status = ?, finished_at_ms = ?, revision = ?, updated_global_seq = ? WHERE execution_id = ?")
            .bind(status)
            .bind(finished_at_ms)
            .bind(to_i64(revision)?)
            .bind(to_i64(global_seq)?)
            .bind(execution_id.to_string())
            .execute(conn).await
            .map_err(database_error)?;
        Ok(())
    })).await?;
    get_execution_sync(conn, request.execution_id).await?.ok_or(StorageError::ExecutionMissing)
}

/// 修改 archive pin；retention 仅处理未 pin archive。
pub(crate) async fn process_pin_execution(
    conn: &mut DatabaseSession,
    request: ExecutionPin,
) -> Result<ExecutionRecord, StorageError> {
    let execution =
        get_execution_sync(conn, request.execution_id).await?.ok_or(StorageError::ExecutionMissing)?;
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
    if idempotent_event(conn, &event).await?.is_some() {
        return get_execution_sync(conn, request.execution_id).await?
            .ok_or(StorageError::ExecutionMissing);
    }
    if gc_state != GcState::Active {
        return Err(StorageError::ReplayUnavailable(
            "GC 已开始的 archive 不能修改 pin".to_string(),
        ));
    }
    let execution_id = request.execution_id;
    let pinned = i32::from(request.pinned);
    append_event_transaction(conn, &event, &[], move |conn, global_seq, revision| Box::pin(async move {
        statement("UPDATE execution_projection SET pinned = ?, revision = ?, updated_global_seq = ? WHERE execution_id = ?")
            .bind(pinned)
            .bind(to_i64(revision)?)
            .bind(to_i64(global_seq)?)
            .bind(execution_id.to_string())
            .execute(conn).await
            .map_err(database_error)?;
        Ok(())
    })).await?;
    get_execution_sync(conn, request.execution_id).await?.ok_or(StorageError::ExecutionMissing)
}
