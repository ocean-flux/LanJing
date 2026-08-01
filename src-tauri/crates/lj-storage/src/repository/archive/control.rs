/// 在 writer transaction 中 durable 保存一个 control trace。
pub(crate) async fn persist_control_trace(
    conn: &mut DatabaseSession,
    capture: &ControlTraceCapture,
) -> Result<ControlTraceReceipt, StorageError> {
    let execution =
        get_execution_sync(conn, capture.execution_id).await?.ok_or(StorageError::ExecutionMissing)?;
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

    let execution_id = capture.execution_id;
    let invocation_path = capture.invocation_path.clone();
    let trace_hash = capture.trace_hash.clone();
    if let Some(existing) = control_trace_row(conn, execution_id, invocation_ordinal).await? {
            validate_invocation_claim(
                conn,
                execution_id,
                &invocation_path,
                "control",
                &payload_id,
            ).await?;
            if existing.invocation_path_json == invocation_path_json
                && existing.trace_hash == trace_hash
                && existing.trace_json == trace_json
            {
                return Ok(receipt);
            }
            return Err(StorageError::IdempotencyMismatch);
    }
    claim_invocation(
        conn,
        execution_id,
        &invocation_path_json,
        invocation_ordinal,
        invocation_path.node_id(),
        "control",
        &payload_id,
    )
    .await?;
    statement(
        "INSERT INTO control_traces (execution_id, invocation_ordinal, invocation_path_json, trace_hash, trace_json) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(execution_id.to_string())
    .bind(invocation_ordinal)
    .bind(&invocation_path_json)
    .bind(&trace_hash)
    .bind(&trace_json)
    .execute(conn)
    .await
    .map_err(database_error)?;
    Ok(receipt)
}

/// exact 读取并验证 control trace。
pub(crate) async fn load_control_trace(
    conn: &mut DatabaseSession,
    lookup: &ControlReplayLookup,
) -> Result<Option<ControlTraceCapture>, StorageError> {
    ensure_replayable_execution(conn, lookup.archived_execution_id).await?;
    let invocation_path_json =
        canonical_json(&lookup.invocation_path).map_err(|_| StorageError::Serialization)?;
    let invocation_ordinal = to_i64(lookup.invocation_path.ordinal())?;
    let row = statement(
        "SELECT trace.execution_id, trace.invocation_ordinal, trace.invocation_path_json, trace.trace_hash, trace.trace_json FROM execution_invocation_ledger AS ledger INNER JOIN control_traces AS trace ON trace.execution_id = ledger.execution_id AND trace.invocation_ordinal = ledger.invocation_ordinal WHERE ledger.execution_id = ? AND ledger.invocation_ordinal = ? AND ledger.invocation_kind = 'control' AND ledger.invocation_path_json = ? AND trace.invocation_path_json = ledger.invocation_path_json",
    )
    .bind(lookup.archived_execution_id.to_string())
    .bind(invocation_ordinal)
    .bind(&invocation_path_json)
    .get_result::<ControlTraceRow>(conn).await
    .optional()
    .map_err(database_error)?;
    let Some(row) = row else {
        reject_mismatched_invocation(
            conn,
            lookup.archived_execution_id,
            invocation_ordinal,
            &invocation_path_json,
            "control",
        ).await?;
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
    ).await?;
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
pub(crate) async fn validate_replay_complete(
    conn: &mut DatabaseSession,
    lookup: ReplayCompletionLookup,
) -> Result<(), StorageError> {
    ensure_replayable_execution(conn, lookup.archived_execution_id).await?;
    let rows = invocation_rows(conn, lookup.archived_execution_id).await?;
    let observed = u64::try_from(rows.len()).ok();
    let payload_count = current_invocation_payload_count(conn, lookup.archived_execution_id).await?;
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
        validate_ledger_row(conn, lookup.archived_execution_id, row).await?;
    }
    Ok(())
}

/// 验证 request body secret artifact 仍可认证，且与 witness 的逻辑 hash/长度一致。
async fn verify_request_body(
    conn: &mut DatabaseSession,
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
            ).await?;
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
