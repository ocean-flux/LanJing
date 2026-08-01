use crate::types::{ArtifactKind, SecretArtifactId, StorageError};

/// 将原始 request body 与 HTTP witness 的 BLAKE3/长度绑定。
///
/// 该校验发生在写任何 artifact 前，防止调用方以同一个 witness 偷换 live material。
fn request_body_hash(capture: &EffectCapture) -> Result<Option<&str>, StorageError> {
    let witness_body = match &capture.witness {
        EffectWitness::Http(witness) => witness.request.body.as_ref(),
        EffectWitness::QuickJs(_) | EffectWitness::Extract(_) => None,
    };
    match (capture.request_body(), witness_body) {
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => Err(StorageError::InvalidInput(
            "HTTP request body material 与 witness 不一致".to_string(),
        )),
        (Some(bytes), Some(witness)) => {
            let byte_len = u64::try_from(bytes.len()).map_err(|_| {
                StorageError::InvalidInput("HTTP request body 长度超出 u64".to_string())
            })?;
            let hash = blake3::hash(bytes).to_hex().to_string();
            if witness.hash != hash || witness.byte_len != byte_len {
                return Err(StorageError::InvalidInput(
                    "HTTP request body material 与 witness hash 不一致".to_string(),
                ));
            }
            Ok(Some(witness.hash.as_str()))
        }
    }
}

/// 以强制 secret 敏感性写入 HTTP request body。
///
/// 任何未来出现的非 secret material variant 都会被拒绝，而不是猜测请求体是否安全。
async fn write_request_body_secret(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    capture: &EffectCapture,
    created_at_ms: i64,
) -> Result<Option<PendingSecretArtifact>, StorageError> {
    if request_body_hash(capture)?.is_none() {
        return Ok(None);
    }
    if capture.request_body_sensitivity() != Some(EffectCaptureMaterialSensitivity::Secret) {
        return Err(StorageError::InvalidInput(
            "HTTP request body 必须作为 Secret Artifact 持久化".to_string(),
        ));
    }
    let bytes = capture
        .request_body()
        .ok_or_else(|| StorageError::InvalidInput("HTTP request body material 缺失".to_string()))?;
    write_secret(conn, artifacts, bytes, created_at_ms)
        .await
        .map(Some)
}

struct PreparedEffectCapture {
    capture: EffectCapture,
    invocation_path_json: String,
    invocation_ordinal: i64,
    witness_payload: String,
    expected_witness_artifact_hash: String,
    has_request_body: bool,
}

fn prepare_effect_capture_persistence(
    capture: EffectCapture,
) -> Result<PreparedEffectCapture, StorageError> {
    capture.validate_replay_integrity().map_err(|_| {
        StorageError::InvalidInput("effect capture witness 不满足完整性合同".to_string())
    })?;
    ensure_blake3_hash(&capture.output_hash, "effect output hash")?;
    ensure_blake3_hash(&capture.witness_hash, "effect witness hash")?;
    let invocation_path_json =
        canonical_json(&capture.invocation_path).map_err(|_| StorageError::Serialization)?;
    let invocation_ordinal = to_i64(capture.invocation_path.ordinal())?;
    let witness_payload = canonical_json(&StoredEffectWitness {
        witness: capture.witness.clone(),
    })
    .map_err(|_| StorageError::Serialization)?;
    let expected_witness_artifact_hash = blake3::hash(witness_payload.as_bytes())
        .to_hex()
        .to_string();
    let has_request_body = request_body_hash(&capture)?.is_some();

    Ok(PreparedEffectCapture {
        capture,
        invocation_path_json,
        invocation_ordinal,
        witness_payload,
        expected_witness_artifact_hash,
        has_request_body,
    })
}

async fn existing_effect_capture_receipt(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    prepared: &PreparedEffectCapture,
) -> Result<Option<DurableCaptureReceipt>, StorageError> {
    let capture = &prepared.capture;
    let Some(existing) = effect_capture_row(conn, capture.execution_id, capture.effect_id).await? else {
        return Ok(None);
    };
    let existing_path = existing.invocation_path_json.as_str();
    let existing_ordinal = existing.invocation_ordinal;
    if existing_path != prepared.invocation_path_json
        || existing_ordinal != prepared.invocation_ordinal
    {
        return Err(StorageError::IdempotencyMismatch);
    }
    validate_invocation_claim(
        conn,
        capture.execution_id,
        &capture.invocation_path,
        "effect",
        &capture.effect_id.to_string(),
    ).await?;
    if existing.fingerprint == capture.fingerprint
        && existing.output_hash == capture.output_hash
        && existing.witness_hash.as_deref() == Some(capture.witness_hash.as_str())
        && existing.witness_artifact_hash.as_deref()
            == Some(prepared.expected_witness_artifact_hash.as_str())
        && existing.request_body_secret_id.is_some() == prepared.has_request_body
    {
        ensure_existing_capture_durable(conn, artifacts, &existing, capture).await?;
        return Ok(Some(DurableCaptureReceipt {
            effect_id: capture.effect_id,
            invocation_path: capture.invocation_path.clone(),
            fingerprint: capture.fingerprint.clone(),
            output_hash: capture.output_hash.clone(),
            witness_hash: capture.witness_hash.clone(),
        }));
    }
    Err(StorageError::IdempotencyMismatch)
}
