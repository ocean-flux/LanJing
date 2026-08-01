/// 原子消费 candidate-v2 并追加权威 source revision。
pub(crate) async fn process_install_candidate(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    request: InstallCandidateRequest,
) -> Result<InstalledSource, StorageError> {
    let candidate =
        get_candidate_sync(conn, request.candidate_id).await?.ok_or(StorageError::CandidateMissing)?;
    if candidate.expires_at_ms <= request.occurred_at_ms {
        expire_candidate(conn, request.candidate_id).await?;
        return Err(StorageError::CandidateExpired);
    }
    validate_candidate_status_and_schema(&candidate, request.occurred_at_ms)?;
    validate_staged_candidate_event(conn, request.candidate_id, &candidate).await?;
    let current_source = get_source_row(conn, &candidate.target_source_identity).await?;
    let actual_installed_revision = current_source
        .as_ref()
        .map(|row| from_i64(row.revision, "source revision"))
        .transpose()?
        .unwrap_or(0);
    let expected_installed_revision = from_i64(
        candidate.expected_installed_revision,
        "candidate expected revision",
    )?;
    if actual_installed_revision != expected_installed_revision {
        return Err(StorageError::CandidateStale);
    }
    ensure_candidate_secrets(conn, artifacts, request.candidate_id, &candidate).await?;
    let (package, plan, profile, required_grant) =
        load_and_validate_candidate_artifacts(conn, artifacts, &candidate).await?;
    if !grant_covers(&request.grant, &required_grant) {
        return Err(StorageError::GrantInsufficient);
    }

    let source_identity = candidate.target_source_identity.clone();
    let event = EventDraft {
        stream_id: source_stream_id(&source_identity),
        expected_version: expected_installed_revision,
        event_id: request.event_id,
        event_type: EventType::Source,
        schema_version: 2,
        correlation_id: request.correlation_id,
        causation_id: Some(request.candidate_id),
        trace_id: request.trace_id,
        occurred_at_ms: request.occurred_at_ms,
        payload: serde_json::json!({
            "kind": if expected_installed_revision == 0 { "installed" } else { "updated" },
            "candidate_id": request.candidate_id,
            "source_identity": source_identity,
            "definition_version": package.version(),
            "definition_hash": plan.definition_hash(),
            "plan_hash": plan.plan_hash(),
            "expected_installed_revision": expected_installed_revision,
            "schema_version": INSTALL_CANDIDATE_SCHEMA_VERSION,
        }),
        source_identity: Some(source_identity.clone()),
    };
    if let Some(receipt) = idempotent_event(conn, &event).await? {
        return installed_source_from_revision(
            conn,
            artifacts,
            &source_identity,
            receipt.stream_version,
        ).await;
    }
    let package_hash = candidate.package_artifact_hash.clone();
    let plan_artifact_hash = candidate.plan_artifact_hash.clone();
    let definition_hash = candidate.definition_hash.clone();
    let plan_hash = candidate.plan_hash.clone();
    let version = package.version().to_string();
    let base_url = package.definition().base_url().to_string();
    let profile_json = serialize(&profile)?;
    let grant_json = serialize(&request.grant)?;
    let runtime_secret_id = candidate
        .runtime_credential_secret_id
        .as_deref()
        .map(SecretArtifactId::from_str)
        .transpose()?;
    let cookie_namespace = source_cookie_namespace(&source_identity);
    let links = vec![
        ArtifactLink::Existing {
            hash: package_hash.clone(),
            kind: ArtifactKind::Body,
        },
        ArtifactLink::Existing {
            hash: plan_artifact_hash.clone(),
            kind: ArtifactKind::Body,
        },
    ];
    let commit = CandidateInstallCommit {
        artifacts: artifacts.clone(),
        profile: profile.clone(),
        source_identity: source_identity.clone(),
        version: version.clone(),
        profile_json,
        grant_json,
        base_url,
        package_hash,
        plan_artifact_hash,
        definition_hash,
        plan_hash,
        cookie_namespace,
        runtime_secret_id,
        candidate_id: request.candidate_id,
        occurred_at_ms: event.occurred_at_ms,
    };
    let receipt =
        append_event_transaction(conn, &event, &links, move |conn, global_seq, source_revision| {
            Box::pin(async move {
                commit_candidate_install(conn, global_seq, source_revision, &commit).await
            })
        }).await?;
    Ok(InstalledSource {
        source_identity,
        version,
        package,
        plan,
        profile,
        grant: request.grant,
        source_revision: receipt.stream_version,
    })
}

struct CandidateInstallCommit {
    artifacts: ArtifactStore,
    profile: SourceProfile,
    source_identity: String,
    version: String,
    profile_json: String,
    grant_json: String,
    base_url: String,
    package_hash: String,
    plan_artifact_hash: String,
    definition_hash: String,
    plan_hash: String,
    cookie_namespace: String,
    runtime_secret_id: Option<SecretArtifactId>,
    candidate_id: Uuid,
    occurred_at_ms: i64,
}

async fn commit_candidate_install(
    conn: &mut DatabaseSession,
    global_seq: u64,
    source_revision: u64,
    commit: &CandidateInstallCommit,
) -> Result<(), StorageError> {
    upsert_projection_source(conn, &commit.profile, global_seq).await?;
    statement(
        "INSERT INTO source_projection (source_identity, version, profile_json, grant_json, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, cookie_namespace, runtime_credential_secret_id, revision, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(source_identity) DO UPDATE SET version = excluded.version, profile_json = excluded.profile_json, grant_json = excluded.grant_json, package_artifact_hash = excluded.package_artifact_hash, plan_artifact_hash = excluded.plan_artifact_hash, definition_hash = excluded.definition_hash, plan_hash = excluded.plan_hash, cookie_namespace = excluded.cookie_namespace, runtime_credential_secret_id = excluded.runtime_credential_secret_id, revision = excluded.revision, updated_global_seq = excluded.updated_global_seq",
    )
    .bind(&commit.source_identity)
    .bind(&commit.version)
    .bind(&commit.profile_json)
    .bind(&commit.grant_json)
    .bind(&commit.package_hash)
    .bind(&commit.plan_artifact_hash)
    .bind(&commit.definition_hash)
    .bind(&commit.plan_hash)
    .bind(&commit.cookie_namespace)
    .bind(
        commit
            .runtime_secret_id
            .map(|value| value.to_string())
            .as_deref(),
    )
    .bind(to_i64(source_revision)?)
    .bind(to_i64(global_seq)?)
    .execute(conn).await
    .map_err(database_error)?;
    statement(
        "INSERT INTO source_versions (source_identity, source_revision, version, profile_json, grant_json, base_url, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, cookie_namespace, runtime_credential_secret_id, schema_version, installed_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&commit.source_identity)
    .bind(to_i64(source_revision)?)
    .bind(&commit.version)
    .bind(&commit.profile_json)
    .bind(&commit.grant_json)
    .bind(&commit.base_url)
    .bind(&commit.package_hash)
    .bind(&commit.plan_artifact_hash)
    .bind(&commit.definition_hash)
    .bind(&commit.plan_hash)
    .bind(&commit.cookie_namespace)
    .bind(
        commit
            .runtime_secret_id
            .map(|value| value.to_string())
            .as_deref(),
    )
    .bind(i32::try_from(INSTALL_CANDIDATE_SCHEMA_VERSION).map_err(|_| {
        StorageError::InvalidInput("source schema version 无效".to_string())
    })?)
    .bind(commit.occurred_at_ms)
    .execute(conn).await
    .map_err(database_error)?;
    release_secret_owner(conn, "source_projection_runtime", &commit.source_identity).await?;
    if let Some(runtime_secret_id) = commit.runtime_secret_id {
        retain_existing_secret(
            conn,
            &commit.artifacts,
            runtime_secret_id,
            "source_projection_runtime",
            &commit.source_identity,
            commit.occurred_at_ms,
        ).await?;
        retain_existing_secret(
            conn,
            &commit.artifacts,
            runtime_secret_id,
            "source_version_runtime",
            &source_version_owner_id(&commit.source_identity, source_revision),
            commit.occurred_at_ms,
        ).await?;
    }
    release_candidate_secret_owners(conn, commit.candidate_id).await?;
    remove_candidate_event_refs(conn, commit.candidate_id).await?;
    statement("DELETE FROM candidate_projection WHERE candidate_id = ?")
        .bind(commit.candidate_id.to_string())
        .execute(conn).await
        .map_err(database_error)?;
    Ok(())
}

async fn ensure_candidate_secrets(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    candidate_id: Uuid,
    candidate: &CandidateRow,
) -> Result<(), StorageError> {
    let owner_id = candidate_id.to_string();
    if let Some(runtime) = candidate.runtime_credential_secret_id.as_deref() {
        read_owned_secret(
            conn,
            artifacts,
            SecretArtifactId::from_str(runtime)?,
            "candidate_runtime",
            &owner_id,
        )
        .await
        .map_err(map_candidate_secret_error)?;
    }
    Ok(())
}

fn map_candidate_secret_error(error: StorageError) -> StorageError {
    match error {
        StorageError::KeyringLocked | StorageError::KeyringUnavailable | StorageError::KeyLost => {
            error
        }
        _ => StorageError::CandidateTampered,
    }
}

async fn load_and_validate_candidate_artifacts(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    candidate: &CandidateRow,
) -> Result<
    (
        RulePackage,
        ExecutionPlan,
        SourceProfile,
        PolicyCapabilities,
    ),
    StorageError,
> {
    let package_bytes = read_body_by_hash(conn, artifacts, &candidate.package_artifact_hash)
        .await
        .map_err(|_| StorageError::CandidateTampered)?;
    let plan_bytes = read_body_by_hash(conn, artifacts, &candidate.plan_artifact_hash)
        .await
        .map_err(|_| StorageError::CandidateTampered)?;
    let package =
        read_rule_package_artifact(&package_bytes).map_err(candidate_contract_artifact_error)?;
    let plan =
        read_execution_plan_artifact(&plan_bytes).map_err(candidate_contract_artifact_error)?;
    validate_candidate_package_and_plan(&package, &plan)
        .map_err(candidate_contract_artifact_error)?;
    if package.source_identity().id != candidate.target_source_identity
        || plan.plan_hash() != candidate.plan_hash
        || plan.definition_hash() != candidate.definition_hash
    {
        return Err(StorageError::CandidateTampered);
    }
    let profile = deserialize::<SourceProfile>(candidate.profile_json.as_bytes())
        .map_err(|_| StorageError::CandidateTampered)?;
    if profile.id.0 != candidate.target_source_identity
        || profile.version.as_deref() != Some(package.version())
    {
        return Err(StorageError::CandidateTampered);
    }
    let required_grant =
        deserialize::<PolicyCapabilities>(candidate.required_grant_json.as_bytes())
            .map_err(|_| StorageError::CandidateTampered)?;
    if required_grant != package.definition().capability_manifest().required {
        return Err(StorageError::CandidateTampered);
    }
    Ok((package, plan, profile, required_grant))
}

fn validate_candidate_status_and_schema(
    candidate: &CandidateRow,
    now_ms: i64,
) -> Result<(), StorageError> {
    if candidate.candidate_schema_version
        != i32::try_from(INSTALL_CANDIDATE_SCHEMA_VERSION)
            .map_err(|_| StorageError::CandidateSchemaMismatch)?
    {
        return Err(StorageError::CandidateSchemaMismatch);
    }
    match candidate.status.as_str() {
        "staged" => {}
        "expired" => return Err(StorageError::CandidateExpired),
        _ => return Err(StorageError::CandidateUnavailable),
    }
    if candidate.expires_at_ms <= now_ms {
        return Err(StorageError::CandidateExpired);
    }
    Ok(())
}

async fn validate_staged_candidate_event(
    conn: &mut DatabaseSession,
    candidate_id: Uuid,
    candidate: &CandidateRow,
) -> Result<(), StorageError> {
    let event = statement(
        "SELECT stream_version, event_id, source_identity, event_type, schema_version, payload_json, artifact_refs_json, secret_refs_json FROM events WHERE stream_id = ? AND stream_version = 1",
    )
    .bind(candidate_stream_id(candidate_id))
    .get_result::<CandidateStagedEventRow>(conn).await
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::CandidateTampered)?;
    let payload = serde_json::from_str::<serde_json::Value>(&event.payload_json)
        .map_err(|_| StorageError::CandidateTampered)?;
    let artifact_refs = serde_json::from_str::<Vec<ArtifactRef>>(&event.artifact_refs_json)
        .map_err(|_| StorageError::CandidateTampered)?;
    let secret_refs = serde_json::from_str::<Vec<SecretRef>>(&event.secret_refs_json)
        .map_err(|_| StorageError::CandidateTampered)?;
    let mut actual_artifacts = artifact_refs
        .iter()
        .map(|reference| reference.hash.as_str())
        .collect::<Vec<_>>();
    actual_artifacts.sort_unstable();
    let mut expected_artifacts = vec![
        candidate.package_artifact_hash.as_str(),
        candidate.plan_artifact_hash.as_str(),
    ];
    expected_artifacts.sort_unstable();
    let contract_hash = candidate_contract_hash(&CandidateContractHashInput {
        candidate_id,
        source_identity: &candidate.target_source_identity,
        runtime_secret_id: candidate
            .runtime_credential_secret_id
            .as_deref()
            .map(SecretArtifactId::from_str)
            .transpose()?,
        expected_installed_revision: from_i64(
            candidate.expected_installed_revision,
            "candidate expected revision",
        )?,
        package_artifact_hash: &candidate.package_artifact_hash,
        plan_artifact_hash: &candidate.plan_artifact_hash,
        definition_hash: &candidate.definition_hash,
        plan_hash: &candidate.plan_hash,
        profile_json: &candidate.profile_json,
        grant_json: &candidate.required_grant_json,
        diagnostics_json: &candidate.diagnostics_json,
        expires_at_ms: candidate.expires_at_ms,
    })?;
    if event.stream_version != 1
        || event.event_id != candidate_id.to_string()
        || event.source_identity.as_deref() != Some(candidate.target_source_identity.as_str())
        || event.event_type != serialize(&EventType::Candidate)?
        || event.schema_version
            != i32::try_from(INSTALL_CANDIDATE_SCHEMA_VERSION)
                .map_err(|_| StorageError::CandidateSchemaMismatch)?
        || payload.get("kind").and_then(serde_json::Value::as_str) != Some("staged")
        || payload
            .get("contract_hash")
            .and_then(serde_json::Value::as_str)
            != Some(contract_hash.as_str())
        || payload
            .get("candidate_schema_version")
            .and_then(serde_json::Value::as_u64)
            != Some(u64::from(INSTALL_CANDIDATE_SCHEMA_VERSION))
        || payload
            .get("expires_at_ms")
            .and_then(serde_json::Value::as_i64)
            != Some(candidate.expires_at_ms)
        || actual_artifacts != expected_artifacts
        || artifact_refs
            .iter()
            .any(|reference| reference.codec != "zstd")
        || !secret_refs.is_empty()
    {
        return Err(StorageError::CandidateTampered);
    }
    Ok(())
}

struct CandidateContractHashInput<'a> {
    candidate_id: Uuid,
    source_identity: &'a str,
    runtime_secret_id: Option<SecretArtifactId>,
    expected_installed_revision: u64,
    package_artifact_hash: &'a str,
    plan_artifact_hash: &'a str,
    definition_hash: &'a str,
    plan_hash: &'a str,
    profile_json: &'a str,
    grant_json: &'a str,
    diagnostics_json: &'a str,
    expires_at_ms: i64,
}

fn candidate_contract_hash(input: &CandidateContractHashInput<'_>) -> Result<String, StorageError> {
    let value = serde_json::json!({
        "candidate_id": input.candidate_id,
        "source_identity": input.source_identity,
        "runtime_secret_id": input.runtime_secret_id,
        "expected_installed_revision": input.expected_installed_revision,
        "package_artifact_hash": input.package_artifact_hash,
        "plan_artifact_hash": input.plan_artifact_hash,
        "definition_hash": input.definition_hash,
        "plan_hash": input.plan_hash,
        "profile_hash": blake3::hash(input.profile_json.as_bytes()).to_hex().to_string(),
        "grant_hash": blake3::hash(input.grant_json.as_bytes()).to_hex().to_string(),
        "diagnostics_hash": blake3::hash(input.diagnostics_json.as_bytes()).to_hex().to_string(),
        "expires_at_ms": input.expires_at_ms,
        "candidate_schema_version": INSTALL_CANDIDATE_SCHEMA_VERSION,
    });
    let bytes = serde_json::to_vec(&value).map_err(|_| StorageError::Serialization)?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}
