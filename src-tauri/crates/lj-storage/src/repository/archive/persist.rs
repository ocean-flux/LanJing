/// 在 writer blocking lane 内完成 effect archive 的文件与 Event/SQLite 原子认领。
///
/// 返回 receipt 时，output body、safe witness、可选 response header secret、可选 request body
/// secret 都已有 durable 文件并已被同一 Event transaction 建立引用。
pub(crate) async fn persist_effect_capture(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    capture: EffectCapture,
) -> Result<DurableCaptureReceipt, StorageError> {
    let prepared = prepare_effect_capture_persistence(capture)?;
    if let Some(receipt) = existing_effect_capture_receipt(conn, artifacts, &prepared).await? {
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
        get_execution_sync(conn, capture.execution_id).await?.ok_or(StorageError::ExecutionMissing)?;
    if execution.status.is_terminal() {
        return Err(StorageError::InvalidInput(
            "终态 execution 不能写入 effect capture".to_string(),
        ));
    }

    let occurred_at_ms = now_millis();
    let request_body_secret =
        write_request_body_secret(conn, artifacts, &capture, occurred_at_ms).await?;
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
    let response_headers_secret = match secret_headers.as_ref() {
        Some(headers) => {
            let json = serialize(headers)?;
            Some(write_secret(conn, artifacts, json.as_bytes(), occurred_at_ms).await?)
        }
        None => None,
    };

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
    let expected_version = stream_version(conn, &execution_stream_id(execution_id)).await?;
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

    append_event_transaction(conn, &event, &links, move |conn, global_seq, revision| Box::pin(async move {
        let owner_id = format!("{execution_id}:{effect_id}");
        if let Some(secret) = &response_headers_secret {
            retain_pending_secret(
                conn,
                secret,
                "effect_response_headers",
                &owner_id,
                occurred_at_ms,
            ).await?;
        }
        if let Some(secret) = &request_body_secret {
            retain_pending_secret(
                conn,
                secret,
                "effect_request_body",
                &owner_id,
                occurred_at_ms,
            ).await?;
        }
        claim_invocation(
            conn,
            execution_id,
            &invocation_path_json,
            invocation_ordinal,
            node_id,
            "effect",
            &effect_id.to_string(),
        ).await?;
        statement(
            "INSERT INTO effect_captures (execution_id, effect_id, node_id, invocation_path_json, invocation_ordinal, effect_kind, fingerprint, output_hash, witness_hash, output_artifact_hash, witness_artifact_hash, response_headers_secret_id, request_body_secret_id, global_seq) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(execution_id.to_string())
        .bind(effect_id.to_string())
        .bind(node_id.to_string())
        .bind(&invocation_path_json)
        .bind(invocation_ordinal)
        .bind(&effect_kind)
        .bind(&fingerprint)
        .bind(&output_hash)
        .bind(&witness_hash)
        .bind(&output_artifact_hash)
        .bind(&witness_artifact_hash)
        .bind(response_headers_secret_id.as_deref())
        .bind(request_body_secret_id.as_deref())
        .bind(to_i64(global_seq)?)
        .execute(conn).await
        .map_err(database_error)?;
        update_execution_revision(conn, execution_id, revision, global_seq).await?;
        Ok(())
    }))
    .await?;
    Ok(receipt)
}
