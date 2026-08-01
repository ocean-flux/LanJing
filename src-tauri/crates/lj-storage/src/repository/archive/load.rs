/// 读取 archive 并在向 runtime 返回前完成 artifact/witness/secret 完整性验证。
pub(crate) async fn load_effect_capture(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    lookup: &EffectReplayLookup,
) -> Result<Option<EffectCapture>, StorageError> {
    ensure_replayable_execution(conn, lookup.archived_execution_id).await?;
    let invocation_path_json =
        canonical_json(&lookup.invocation_path).map_err(|_| StorageError::Serialization)?;
    let invocation_ordinal = to_i64(lookup.invocation_path.ordinal())?;
    let row = statement(
        "SELECT capture.execution_id, capture.effect_id, capture.node_id, capture.effect_kind, capture.invocation_path_json, capture.invocation_ordinal, capture.fingerprint, capture.output_hash, capture.witness_hash, capture.output_artifact_hash, capture.witness_artifact_hash, capture.response_headers_secret_id, capture.request_body_secret_id FROM execution_invocation_ledger AS ledger INNER JOIN effect_captures AS capture ON capture.execution_id = ledger.execution_id AND capture.invocation_ordinal = ledger.invocation_ordinal WHERE ledger.execution_id = ? AND ledger.invocation_ordinal = ? AND ledger.invocation_kind = 'effect' AND ledger.invocation_path_json = ? AND capture.invocation_path_json = ledger.invocation_path_json",
    )
    .bind(lookup.archived_execution_id.to_string())
    .bind(invocation_ordinal)
    .bind(&invocation_path_json)
    .get_result::<EffectCaptureRow>(conn).await
    .optional()
    .map_err(database_error)?;
    let Some(row) = row else {
        reject_mismatched_invocation(
            conn,
            lookup.archived_execution_id,
            invocation_ordinal,
            &invocation_path_json,
            "effect",
        ).await?;
        return Ok(None);
    };
    let stored_path_json = row.invocation_path_json.as_str();
    let stored_ordinal = row.invocation_ordinal;
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
    ).await?;
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
    ).await?)?;
    let witness = read_body_by_hash(conn, artifacts, &witness_artifact_hash)
        .await
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
    ).await?;
    let output = if let Some(secret_id) = row.response_headers_secret_id.as_deref() {
        let secret = deserialize::<SecretHeaderSnapshot>(&read_owned_secret(
            conn,
            artifacts,
            SecretArtifactId::from_str(secret_id)?,
            "effect_response_headers",
            &secret_owner_id,
        ).await?)?;
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
