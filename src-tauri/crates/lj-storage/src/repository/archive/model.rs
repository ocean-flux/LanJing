#[derive(Debug, Serialize, Deserialize)]
struct StoredEffectOutput {
    output: EffectOutput,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredEffectWitness {
    witness: EffectWitness,
}

#[derive(Serialize, Deserialize)]
struct SecretHeaderSnapshot {
    headers: HashMap<String, String>,
}

/// 防止重复 receipt 在磁盘/SQLite 已损坏时伪装为 durable 成功。
async fn ensure_existing_capture_durable(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    row: &EffectCaptureRow,
    capture: &EffectCapture,
) -> Result<(), StorageError> {
    let output_payload = deserialize::<StoredEffectOutput>(&read_body_by_hash(
        conn,
        artifacts,
        &row.output_artifact_hash,
    ).await?)?;
    let witness_artifact_hash = row
        .witness_artifact_hash
        .as_deref()
        .ok_or_else(|| StorageError::ArtifactUnavailable("effect witness artifact".to_string()))?;
    let stored_witness = deserialize::<StoredEffectWitness>(&read_body_by_hash(
        conn,
        artifacts,
        witness_artifact_hash,
    ).await?)?;
    if stored_witness.witness != capture.witness {
        return Err(StorageError::IdempotencyMismatch);
    }
    let secret_owner_id = format!("{}:{}", row.execution_id, row.effect_id);
    let output = if let Some(secret_id) = row.response_headers_secret_id.as_deref() {
        let secret = deserialize::<SecretHeaderSnapshot>(&read_owned_secret(
            conn,
            artifacts,
            SecretArtifactId::from_str(secret_id)?,
            "effect_response_headers",
            &secret_owner_id,
        ).await?)?;
        restore_sensitive_headers(output_payload.output, secret.headers)
    } else {
        output_payload.output
    };
    if output != *capture.output {
        return Err(StorageError::IdempotencyMismatch);
    }
    verify_request_body(
        conn,
        artifacts,
        &capture.witness,
        row.request_body_secret_id.as_deref(),
        &secret_owner_id,
    )
    .await
}

/// 将 response 中仅可加密保存的 header 从可 replay output 剥离。
fn split_sensitive_output(output: &EffectOutput) -> (EffectOutput, Option<SecretHeaderSnapshot>) {
    match output {
        EffectOutput::Http(response) => {
            let mut safe_response = response.clone();
            let mut secret_headers = HashMap::new();
            safe_response.headers.retain(|key, value| {
                if is_sensitive_header(key) {
                    secret_headers.insert(key.clone(), value.clone());
                    false
                } else {
                    true
                }
            });
            let secret = (!secret_headers.is_empty()).then_some(SecretHeaderSnapshot {
                headers: secret_headers,
            });
            (EffectOutput::Http(safe_response), secret)
        }
        _ => (output.clone(), None),
    }
}

fn restore_sensitive_headers(
    output: EffectOutput,
    secret_headers: HashMap<String, String>,
) -> EffectOutput {
    match output {
        EffectOutput::Http(mut response) => {
            response.headers.extend(secret_headers);
            EffectOutput::Http(response)
        }
        other => other,
    }
}

fn is_sensitive_header(header: &str) -> bool {
    SensitiveNamePolicy::is_sensitive_response_header(header)
}

#[derive(FromQueryResult)]
struct InvocationLedgerRow {
    invocation_ordinal: i64,
    invocation_kind: String,
    node_id: String,
    invocation_path_json: String,
    payload_id: String,
}

#[derive(FromQueryResult)]
struct ControlTraceRow {
    execution_id: String,
    invocation_ordinal: i64,
    invocation_path_json: String,
    trace_hash: String,
    trace_json: String,
}

#[derive(FromQueryResult)]
struct OptionalOrdinalRow {
    value: Option<i64>,
}

#[derive(FromQueryResult)]
struct CountRow {
    value: i64,
}
