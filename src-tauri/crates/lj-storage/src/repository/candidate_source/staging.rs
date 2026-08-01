/// 单 writer composite publish candidate。
pub(crate) async fn process_stage_candidate(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    draft: CandidateDraft,
) -> Result<CandidateSummary, StorageError> {
    if get_candidate_sync(conn, draft.candidate_id).await?.is_some() {
        return Err(StorageError::CandidateUnavailable);
    }
    let expires_at_ms = validate_candidate_draft(&draft)?;
    let profile_json = serialize(&draft.profile)?;
    let grant_json = serialize(&draft.required_grant)?;
    let diagnostics_json = serialize(&draft.diagnostics)?;
    let package_bytes =
        serde_json::to_vec(&draft.package).map_err(|_| StorageError::Serialization)?;
    let plan_bytes = serde_json::to_vec(&draft.plan).map_err(|_| StorageError::Serialization)?;

    let runtime_secret = match draft.runtime_credentials.as_ref() {
        Some(material) => Some(
            write_secret(
                conn,
                artifacts,
                material.expose_bytes(),
                draft.created_at_ms,
            )
            .await?,
        ),
        None => None,
    };
    let package_artifact = artifacts.write(ArtifactKind::Body, &package_bytes)?;
    let plan_artifact = artifacts.write(ArtifactKind::Body, &plan_bytes)?;
    let source_identity = draft.package.source_identity().id.clone();
    let contract_hash = candidate_contract_hash(&CandidateContractHashInput {
        candidate_id: draft.candidate_id,
        source_identity: &source_identity,
        runtime_secret_id: runtime_secret.as_ref().map(|value| value.secret_id),
        expected_installed_revision: draft.expected_installed_revision,
        package_artifact_hash: &package_artifact.hash,
        plan_artifact_hash: &plan_artifact.hash,
        definition_hash: draft.plan.definition_hash(),
        plan_hash: draft.plan.plan_hash(),
        profile_json: &profile_json,
        grant_json: &grant_json,
        diagnostics_json: &diagnostics_json,
        expires_at_ms,
    })?;
    let event = EventDraft {
        stream_id: candidate_stream_id(draft.candidate_id),
        expected_version: 0,
        event_id: draft.candidate_id,
        event_type: EventType::Candidate,
        schema_version: INSTALL_CANDIDATE_SCHEMA_VERSION,
        correlation_id: draft.correlation_id,
        causation_id: None,
        trace_id: draft.trace_id,
        occurred_at_ms: draft.created_at_ms,
        payload: serde_json::json!({
            "kind": "staged",
            "candidate_id": draft.candidate_id,
            "target_source_identity": source_identity,
            "candidate_schema_version": INSTALL_CANDIDATE_SCHEMA_VERSION,
            "contract_hash": contract_hash,
            "expires_at_ms": expires_at_ms,
        }),
        source_identity: Some(source_identity.clone()),
    };
    let links = vec![
        ArtifactLink::New(package_artifact),
        ArtifactLink::New(plan_artifact),
    ];
    let package_hash = match &links[0] {
        ArtifactLink::New(value) => value.hash.clone(),
        ArtifactLink::Existing { .. } => unreachable!(),
    };
    let plan_artifact_hash = match &links[1] {
        ArtifactLink::New(value) => value.hash.clone(),
        ArtifactLink::Existing { .. } => unreachable!(),
    };
    let candidate_id = draft.candidate_id;
    let created_at_ms = draft.created_at_ms;
    let expected_installed_revision = draft.expected_installed_revision;
    let definition_hash = draft.plan.definition_hash().to_string();
    let plan_hash = draft.plan.plan_hash().to_string();
    let summary = CandidateSummary {
        candidate_id,
        source_identity: source_identity.clone(),
        expected_installed_revision,
        profile: draft.profile,
        required_grant: draft.required_grant,
        diagnostics: draft.diagnostics,
        definition_hash: definition_hash.clone(),
        plan_hash: plan_hash.clone(),
        expires_at_ms,
    };
    append_event_transaction(conn, &event, &links, move |conn, _, stream_revision| Box::pin(async move {
        if let Some(runtime_secret) = runtime_secret.as_ref() {
            retain_pending_secret(
                conn,
                runtime_secret,
                "candidate_runtime",
                &candidate_id.to_string(),
                created_at_ms,
            ).await?;
        }
        statement(
            "INSERT INTO candidate_projection (candidate_id, candidate_schema_version, runtime_credential_secret_id, target_source_identity, expected_installed_revision, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, profile_json, required_grant_json, diagnostics_json, expires_at_ms, status, stream_version, created_at_ms, consumed_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'staged', ?, ?, NULL)",
        )
        .bind(candidate_id.to_string())
        .bind(i32::try_from(INSTALL_CANDIDATE_SCHEMA_VERSION).map_err(|_| StorageError::CandidateSchemaMismatch)?)
        .bind(runtime_secret.as_ref().map(|value| value.secret_id.to_string()).as_deref())
        .bind(&source_identity)
        .bind(to_i64(expected_installed_revision)?)
        .bind(&package_hash)
        .bind(&plan_artifact_hash)
        .bind(&definition_hash)
        .bind(&plan_hash)
        .bind(&profile_json)
        .bind(&grant_json)
        .bind(&diagnostics_json)
        .bind(expires_at_ms)
        .bind(to_i64(stream_revision)?)
        .bind(created_at_ms)
        .execute(conn).await
        .map_err(database_error)?;
        Ok(())
    })).await?;
    Ok(summary)
}

fn validate_candidate_draft(draft: &CandidateDraft) -> Result<i64, StorageError> {
    validate_candidate_package_and_plan(&draft.package, &draft.plan)?;
    validate_candidate_identity(draft)?;
    let expires_at_ms = draft
        .expires_at_ms
        .unwrap_or(draft.created_at_ms.saturating_add(DEFAULT_CANDIDATE_TTL_MS));
    if expires_at_ms <= draft.created_at_ms {
        return Err(StorageError::InvalidInput(
            "candidate 到期时间必须晚于创建时间".to_string(),
        ));
    }
    if draft
        .runtime_credentials
        .as_ref()
        .is_some_and(|material| material.expose_bytes().is_empty())
    {
        return Err(StorageError::InvalidInput(
            "runtime credential 不能为空".to_string(),
        ));
    }
    Ok(expires_at_ms)
}

fn validate_candidate_identity(draft: &CandidateDraft) -> Result<(), StorageError> {
    let source = &draft.package.source_identity().id;
    if source.is_empty()
        || draft.package.definition().source_identity().id != *source
        || draft.profile.id.0 != *source
        || draft.profile.version.as_deref() != Some(draft.package.version())
        || draft.required_grant != draft.package.definition().capability_manifest().required
    {
        return Err(StorageError::InvalidInput(
            "candidate 来源身份或 grant/profile 不一致".to_string(),
        ));
    }
    Ok(())
}
