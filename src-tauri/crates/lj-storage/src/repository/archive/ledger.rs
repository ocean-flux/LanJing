async fn ensure_replayable_execution(
    conn: &mut DatabaseSession,
    execution_id: Uuid,
) -> Result<(), StorageError> {
    let execution =
        get_execution_sync(conn, execution_id).await?.ok_or(StorageError::ExecutionMissing)?;
    if !execution.replayable {
        return Err(StorageError::ReplayUnavailable(
            "archive 已被 GC".to_string(),
        ));
    }
    Ok(())
}

async fn claim_invocation(
    conn: &mut DatabaseSession,
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
    let maximum = statement(
        "SELECT MAX(invocation_ordinal) AS value FROM execution_invocation_ledger WHERE execution_id = ?",
    )
    .bind(execution_id.to_string())
    .get_result::<OptionalOrdinalRow>(conn).await
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
    statement(
        "INSERT INTO execution_invocation_ledger (execution_id, invocation_ordinal, invocation_kind, node_id, invocation_path_json, payload_id) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(execution_id.to_string())
    .bind(invocation_ordinal)
    .bind(invocation_kind)
    .bind(node_id.to_string())
    .bind(invocation_path_json)
    .bind(payload_id)
    .execute(conn).await
    .map_err(database_error)?;
    Ok(())
}

async fn validate_invocation_claim(
    conn: &mut DatabaseSession,
    execution_id: Uuid,
    path: &InvocationPath,
    invocation_kind: &str,
    payload_id: &str,
) -> Result<(), StorageError> {
    let invocation_ordinal = to_i64(path.ordinal())?;
    let invocation_path_json = canonical_json(path).map_err(|_| StorageError::Serialization)?;
    let row = statement(
        "SELECT invocation_ordinal, invocation_kind, node_id, invocation_path_json, payload_id FROM execution_invocation_ledger WHERE execution_id = ? AND invocation_ordinal = ?",
    )
    .bind(execution_id.to_string())
    .bind(invocation_ordinal)
    .get_result::<InvocationLedgerRow>(conn).await
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

async fn invocation_rows(
    conn: &mut DatabaseSession,
    execution_id: Uuid,
) -> Result<Vec<InvocationLedgerRow>, StorageError> {
    statement(
        "SELECT invocation_ordinal, invocation_kind, node_id, invocation_path_json, payload_id FROM execution_invocation_ledger WHERE execution_id = ? ORDER BY invocation_ordinal ASC",
    )
    .bind(execution_id.to_string())
    .load::<InvocationLedgerRow>(conn).await
    .map_err(database_error)
}

async fn reject_mismatched_invocation(
    conn: &mut DatabaseSession,
    execution_id: Uuid,
    invocation_ordinal: i64,
    invocation_path_json: &str,
    expected_kind: &str,
) -> Result<(), StorageError> {
    let row = statement(
        "SELECT invocation_ordinal, invocation_kind, node_id, invocation_path_json, payload_id FROM execution_invocation_ledger WHERE execution_id = ? AND (invocation_ordinal = ? OR invocation_path_json = ?) LIMIT 1",
    )
    .bind(execution_id.to_string())
    .bind(invocation_ordinal)
    .bind(invocation_path_json)
    .get_result::<InvocationLedgerRow>(conn).await
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

async fn control_trace_row(
    conn: &mut DatabaseSession,
    execution_id: Uuid,
    invocation_ordinal: i64,
) -> Result<Option<ControlTraceRow>, StorageError> {
    statement(
        "SELECT execution_id, invocation_ordinal, invocation_path_json, trace_hash, trace_json FROM control_traces WHERE execution_id = ? AND invocation_ordinal = ?",
    )
    .bind(execution_id.to_string())
    .bind(invocation_ordinal)
    .get_result::<ControlTraceRow>(conn).await
    .optional()
    .map_err(database_error)
}

async fn validate_ledger_row(
    conn: &mut DatabaseSession,
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
        "effect" => statement(
            "SELECT COUNT(*) AS value FROM effect_captures WHERE execution_id = ? AND invocation_ordinal = ? AND invocation_path_json = ? AND effect_id = ?",
        )
        .bind(execution_id.to_string())
        .bind(row.invocation_ordinal)
        .bind(&row.invocation_path_json)
        .bind(&row.payload_id)
        .get_result::<CountRow>(conn).await
        .map_err(database_error)?
        .value,
        "control" => statement(
            "SELECT COUNT(*) AS value FROM control_traces WHERE execution_id = ? AND invocation_ordinal = ? AND invocation_path_json = ?",
        )
        .bind(execution_id.to_string())
        .bind(row.invocation_ordinal)
        .bind(&row.invocation_path_json)
        .get_result::<CountRow>(conn).await
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

async fn current_invocation_payload_count(
    conn: &mut DatabaseSession,
    execution_id: Uuid,
) -> Result<u64, StorageError> {
    let count = statement(
        "SELECT (SELECT COUNT(*) FROM effect_captures WHERE execution_id = ? AND invocation_path_json IS NOT NULL AND invocation_ordinal IS NOT NULL) + (SELECT COUNT(*) FROM control_traces WHERE execution_id = ?) AS value",
    )
    .bind(execution_id.to_string())
    .bind(execution_id.to_string())
    .get_result::<CountRow>(conn).await
    .map_err(database_error)?
    .value;
    u64::try_from(count)
        .map_err(|_| StorageError::ReplayUnavailable("invocation payload 数量无效".to_string()))
}

#[derive(FromQueryResult)]
struct EffectCaptureRow {
    execution_id: String,
    effect_id: String,
    node_id: String,
    effect_kind: String,
    invocation_path_json: String,
    invocation_ordinal: i64,
    fingerprint: String,
    output_hash: String,
    witness_hash: Option<String>,
    output_artifact_hash: String,
    witness_artifact_hash: Option<String>,
    response_headers_secret_id: Option<String>,
    request_body_secret_id: Option<String>,
}

async fn effect_capture_row(
    conn: &mut DatabaseSession,
    execution_id: Uuid,
    effect_id: Uuid,
) -> Result<Option<EffectCaptureRow>, StorageError> {
    statement(
        "SELECT execution_id, effect_id, node_id, invocation_path_json, invocation_ordinal, effect_kind, fingerprint, output_hash, witness_hash, output_artifact_hash, witness_artifact_hash, response_headers_secret_id, request_body_secret_id FROM effect_captures WHERE execution_id = ? AND effect_id = ?",
    )
    .bind(execution_id.to_string())
    .bind(effect_id.to_string())
    .get_result::<EffectCaptureRow>(conn).await
    .optional()
    .map_err(database_error)
}
