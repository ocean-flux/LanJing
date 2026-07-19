//! Candidate staging、source install 与 credential ref 的 writer-side 实现。
//!
//! candidate 是 24 小时独立 staging stream：作者 package、immutable Plan 与安全 preview 先以
//! BLAKE3 body artifact durable 落盘，再由 Event transaction 认领。install 必须重新读取并验证
//! canonical package/Plan、expected source version 与 grant；credential plaintext 只在 blocking
//! writer 中存在，落盘前已经 AES-256-GCM 加密，事件与 projection 永远只保存 Secret ref。

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Integer, Nullable, Text};
use diesel::sqlite::SqliteConnection;
use lj_media::SourceProfile;
use lj_rule_model::{
    ArtifactRef, EventType, ExecutionPlan, PolicyCapabilities, SecretRef, canonical_json,
    definition_hash,
};
use uuid::Uuid;

use crate::artifact::ArtifactStore;
use crate::event_store::{
    ArtifactLink, EventDraft, SECRET_ALGORITHM, append_event_transaction, artifact_row,
    database_error, decrement_artifact_ref, deserialize, ensure_blake3_hash, from_i64,
    idempotent_event, read_body_by_hash, remove_candidate_event_refs, retain_pending_artifact,
    serialize, to_i64,
};
use crate::projection_query::upsert_projection_source;
use crate::types::{
    ArtifactKind, CandidateDraft, CandidateSummary, DEFAULT_CANDIDATE_TTL_MS,
    InstallCandidateRequest, InstalledSource, InstalledSourceRecord, SourceCredentialInput,
    SourceCredentialSnapshot, StorageError,
};

/// 在同一 Event transaction 内持久化 candidate 的 package/Plan staging。
pub(crate) fn process_stage_candidate(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    draft: CandidateDraft,
) -> Result<CandidateSummary, StorageError> {
    validate_candidate(&draft)?;
    if let Some(existing) = get_candidate_summary(conn, draft.candidate_id)? {
        return Ok(existing);
    }
    let package_bytes =
        serde_json::to_vec(&draft.package).map_err(|_| StorageError::Serialization)?;
    let plan_bytes = serde_json::to_vec(&draft.plan).map_err(|_| StorageError::Serialization)?;
    let package = artifacts.write(ArtifactKind::Body, &package_bytes)?;
    let plan = artifacts.write(ArtifactKind::Body, &plan_bytes)?;
    let profile_json = serialize(&draft.profile)?;
    let grant_json = serialize(&draft.required_grant)?;
    let diagnostics_json = serialize(&draft.diagnostics)?;
    let profile_hash = blake3::hash(profile_json.as_bytes()).to_hex().to_string();
    let grant_hash = blake3::hash(grant_json.as_bytes()).to_hex().to_string();
    let diagnostics_hash = blake3::hash(diagnostics_json.as_bytes())
        .to_hex()
        .to_string();
    let expires_at_ms = draft
        .expires_at_ms
        .unwrap_or(draft.created_at_ms.saturating_add(DEFAULT_CANDIDATE_TTL_MS));
    let source_identity = draft.package.source_identity.id.clone();
    let event = EventDraft {
        stream_id: candidate_stream_id(draft.candidate_id),
        expected_version: 0,
        event_id: draft.candidate_id,
        event_type: EventType::Candidate,
        schema_version: 2,
        correlation_id: draft.correlation_id,
        causation_id: None,
        trace_id: draft.trace_id,
        occurred_at_ms: draft.created_at_ms,
        payload: serde_json::json!({
            "kind": "staged",
            "candidate_id": draft.candidate_id,
            "source_identity": source_identity,
            "definition_hash": draft.plan.definition_hash,
            "plan_hash": draft.plan.plan_hash,
            "profile_hash": profile_hash,
            "required_grant_hash": grant_hash,
            "diagnostics_hash": diagnostics_hash,
            "artifact_hash_algorithm": "blake3",
            "expires_at_ms": expires_at_ms,
        }),
        source_identity: Some(source_identity.clone()),
    };
    let candidate_id = draft.candidate_id;
    let package_hash = package.hash.clone();
    let plan_hash = plan.hash.clone();
    let definition_hash = draft.plan.definition_hash.clone();
    let execution_plan_hash = draft.plan.plan_hash.clone();
    let links = vec![ArtifactLink::New(package), ArtifactLink::New(plan)];
    append_event_transaction(conn, &event, &links, |conn, _global_seq, version| {
        sql_query(
            "INSERT INTO candidates (candidate_id, source_identity, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, profile_json, required_grant_json, diagnostics_json, expires_at_ms, status, stream_version, created_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'staged', ?, ?)",
        )
        .bind::<Text, _>(candidate_id.to_string())
        .bind::<Text, _>(&source_identity)
        .bind::<Text, _>(&package_hash)
        .bind::<Text, _>(&plan_hash)
        .bind::<Text, _>(&definition_hash)
        .bind::<Text, _>(&execution_plan_hash)
        .bind::<Text, _>(&profile_json)
        .bind::<Text, _>(&grant_json)
        .bind::<Text, _>(&diagnostics_json)
        .bind::<BigInt, _>(expires_at_ms)
        .bind::<BigInt, _>(to_i64(version)?)
        .bind::<BigInt, _>(event.occurred_at_ms)
        .execute(conn)
        .map_err(database_error)?;
        Ok(())
    })?;
    Ok(CandidateSummary {
        candidate_id,
        source_identity,
        profile: draft.profile,
        required_grant: draft.required_grant,
        diagnostics: draft.diagnostics,
        definition_hash,
        plan_hash: execution_plan_hash,
        expires_at_ms,
    })
}

/// 将单次 source credential snapshot 认领为 AES-GCM Secret Artifact。
pub(crate) fn process_stage_source_credentials(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    input: &SourceCredentialInput,
) -> Result<SourceCredentialSnapshot, StorageError> {
    if input.source_identity.is_empty() || input.secret_bytes.is_empty() {
        return Err(StorageError::InvalidInput(
            "source credential 输入不能为空".to_string(),
        ));
    }
    let candidate =
        get_candidate_sync(conn, input.candidate_id)?.ok_or(StorageError::CandidateMissing)?;
    match candidate.status.as_str() {
        "staged" => {}
        "expired" => return Err(StorageError::CandidateExpired),
        _ => return Err(StorageError::CandidateUnavailable),
    }
    if candidate.source_identity != input.source_identity {
        return Err(StorageError::SourceCredentialUnavailable);
    }
    if candidate.expires_at_ms <= input.created_at_ms {
        return Err(StorageError::CandidateExpired);
    }
    let cookie_namespace = source_cookie_namespace(&input.source_identity);
    let secret_hash = blake3::hash(&input.secret_bytes).to_hex().to_string();
    let expires_at_ms = input
        .created_at_ms
        .saturating_add(DEFAULT_CANDIDATE_TTL_MS)
        .min(candidate.expires_at_ms);
    if let Some(existing) = get_staged_source_credential(conn, input.candidate_id)? {
        if existing.expires_at_ms > input.created_at_ms {
            if existing.source_identity != input.source_identity
                || existing.cookie_namespace != cookie_namespace
                || existing.secret_artifact_hash != secret_hash
            {
                return Err(StorageError::InvalidInput(
                    "candidate 已绑定不同的 source credential".to_string(),
                ));
            }
            let artifact = artifact_row(conn, &secret_hash, ArtifactKind::Secret)?
                .ok_or_else(|| StorageError::ArtifactUnavailable(secret_hash.clone()))?;
            artifacts.ensure_secret_artifact_exists(&secret_hash, &artifact.relative_path)?;
            conn.immediate_transaction::<_, StorageError, _>(|conn| {
                mark_candidate_source_credentials_required(conn, input.candidate_id)
            })?;
            return Ok(SourceCredentialSnapshot {
                cookie_namespace,
                secret_ref: SecretRef {
                    hash: secret_hash,
                    algorithm: SECRET_ALGORITHM.to_string(),
                },
            });
        }
        conn.immediate_transaction::<_, StorageError, _>(|conn| {
            remove_staged_source_credential(conn, input.candidate_id)
        })?;
    }
    let pending = artifacts.write(ArtifactKind::Secret, &input.secret_bytes)?;
    conn.immediate_transaction::<_, StorageError, _>(|conn| {
        retain_pending_artifact(conn, &pending, input.created_at_ms)?;
        sql_query(
            "INSERT INTO source_credential_staging (candidate_id, source_identity, cookie_namespace, secret_artifact_hash, expires_at_ms, created_at_ms) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(input.candidate_id.to_string())
        .bind::<Text, _>(&input.source_identity)
        .bind::<Text, _>(&cookie_namespace)
        .bind::<Text, _>(&pending.hash)
        .bind::<BigInt, _>(expires_at_ms)
        .bind::<BigInt, _>(input.created_at_ms)
        .execute(conn)
        .map_err(database_error)?;
        mark_candidate_source_credentials_required(conn, input.candidate_id)?;
        Ok(())
    })?;
    Ok(SourceCredentialSnapshot {
        cookie_namespace,
        secret_ref: SecretRef {
            hash: pending.hash,
            algorithm: SECRET_ALGORITHM.to_string(),
        },
    })
}

struct ResolvedSourceCredentials {
    cookie_namespace: String,
    secret_artifact_hash: Option<String>,
}

fn resolve_install_source_credentials(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    candidate_id: Uuid,
    source_credentials_required: bool,
    snapshot: Option<&SourceCredentialSnapshot>,
    source_identity: &str,
    occurred_at_ms: i64,
) -> Result<ResolvedSourceCredentials, StorageError> {
    let cookie_namespace = source_cookie_namespace(source_identity);
    let (snapshot, staged) = match (snapshot, get_staged_source_credential(conn, candidate_id)?) {
        (None, None) if !source_credentials_required => {
            return Ok(ResolvedSourceCredentials {
                cookie_namespace,
                secret_artifact_hash: None,
            });
        }
        (Some(snapshot), Some(staged)) => (snapshot, staged),
        _ => return Err(StorageError::SourceCredentialUnavailable),
    };
    if staged.source_identity != source_identity
        || snapshot.cookie_namespace != cookie_namespace
        || snapshot.cookie_namespace != staged.cookie_namespace
        || snapshot.secret_ref.algorithm != SECRET_ALGORITHM
        || snapshot.secret_ref.hash != staged.secret_artifact_hash
        || staged.expires_at_ms <= occurred_at_ms
    {
        return Err(StorageError::SourceCredentialUnavailable);
    }
    ensure_blake3_hash(&snapshot.secret_ref.hash, "source credential secret hash")?;
    let artifact = artifact_row(conn, &snapshot.secret_ref.hash, ArtifactKind::Secret)?
        .ok_or_else(|| StorageError::ArtifactUnavailable(snapshot.secret_ref.hash.clone()))?;
    if artifact.encryption.as_deref() != Some(SECRET_ALGORITHM) {
        return Err(StorageError::SourceCredentialUnavailable);
    }
    artifacts.ensure_secret_artifact_exists(&snapshot.secret_ref.hash, &artifact.relative_path)?;
    Ok(ResolvedSourceCredentials {
        cookie_namespace,
        secret_artifact_hash: Some(snapshot.secret_ref.hash.clone()),
    })
}

/// 原子消费 staged candidate；调用方只能在得到返回值后使用 installation 的 Plan。
pub(crate) fn process_install_candidate(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    request: InstallCandidateRequest,
) -> Result<InstalledSource, StorageError> {
    let candidate =
        get_candidate_sync(conn, request.candidate_id)?.ok_or(StorageError::CandidateMissing)?;
    match candidate.status.as_str() {
        "staged" => {}
        "expired" => return Err(StorageError::CandidateExpired),
        _ => return Err(StorageError::CandidateUnavailable),
    }
    validate_staged_candidate_event(conn, request.candidate_id, &candidate)?;
    if candidate.expires_at_ms <= request.occurred_at_ms {
        expire_candidate(conn, request.candidate_id)?;
        return Err(StorageError::CandidateExpired);
    }
    let (package, plan, profile, required_grant) =
        load_and_validate_candidate_artifacts(conn, artifacts, &candidate)?;
    if !grant_covers(&request.grant, &required_grant) {
        return Err(StorageError::GrantInsufficient);
    }
    let source_identity = candidate.source_identity.clone();
    let source_credentials = resolve_install_source_credentials(
        conn,
        artifacts,
        request.candidate_id,
        candidate.source_credentials_required != 0,
        request.source_credentials.as_ref(),
        &source_identity,
        request.occurred_at_ms,
    )?;
    let event = EventDraft {
        stream_id: source_stream_id(&source_identity),
        expected_version: request.expected_source_version,
        event_id: request.event_id,
        event_type: EventType::Source,
        schema_version: 1,
        correlation_id: request.correlation_id,
        causation_id: None,
        trace_id: request.trace_id,
        occurred_at_ms: request.occurred_at_ms,
        payload: serde_json::json!({
            "kind": "installed",
            "candidate_id": request.candidate_id,
            "source_identity": source_identity,
            "version": package.version,
            "definition_hash": plan.definition_hash,
            "plan_hash": plan.plan_hash,
            "cookie_namespace": &source_credentials.cookie_namespace,
        }),
        source_identity: Some(candidate.source_identity.clone()),
    };
    if let Some(receipt) = idempotent_event(conn, &event)? {
        return installed_source_from_event(
            conn,
            artifacts,
            &candidate.source_identity,
            receipt.stream_version,
        );
    }
    let package_hash = candidate.package_artifact_hash.clone();
    let plan_artifact_hash = candidate.plan_artifact_hash.clone();
    let definition_hash = candidate.definition_hash.clone();
    let plan_hash = candidate.plan_hash.clone();
    let version = package.version.clone();
    let base_url = package.definition.base_url.clone();
    let profile_json = serialize(&profile)?;
    let grant_json = serialize(&request.grant)?;
    let candidate_id = request.candidate_id;
    let cookie_namespace = source_credentials.cookie_namespace;
    let secret_artifact_hash = source_credentials.secret_artifact_hash;
    let links = source_install_links(
        &package_hash,
        &plan_artifact_hash,
        secret_artifact_hash.as_deref(),
    );
    let receipt = append_event_transaction(
        conn,
        &event,
        &links,
        |conn, global_seq, source_revision| {
            upsert_projection_source(conn, &profile, global_seq)?;
            sql_query(
                "INSERT INTO source_projection (source_identity, version, profile_json, grant_json, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, cookie_namespace, secret_artifact_hash, revision, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(source_identity) DO UPDATE SET version = excluded.version, profile_json = excluded.profile_json, grant_json = excluded.grant_json, package_artifact_hash = excluded.package_artifact_hash, plan_artifact_hash = excluded.plan_artifact_hash, definition_hash = excluded.definition_hash, plan_hash = excluded.plan_hash, cookie_namespace = excluded.cookie_namespace, secret_artifact_hash = excluded.secret_artifact_hash, revision = excluded.revision, updated_global_seq = excluded.updated_global_seq",
            )
            .bind::<Text, _>(&source_identity)
            .bind::<Text, _>(&version)
            .bind::<Text, _>(&profile_json)
            .bind::<Text, _>(&grant_json)
            .bind::<Text, _>(&package_hash)
            .bind::<Text, _>(&plan_artifact_hash)
            .bind::<Text, _>(&definition_hash)
            .bind::<Text, _>(&plan_hash)
            .bind::<Text, _>(&cookie_namespace)
            .bind::<Nullable<Text>, _>(secret_artifact_hash.as_deref())
            .bind::<BigInt, _>(to_i64(source_revision)?)
            .bind::<BigInt, _>(to_i64(global_seq)?)
            .execute(conn)
            .map_err(database_error)?;
            sql_query(
                "INSERT INTO source_versions (source_identity, version, profile_json, grant_json, base_url, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, cookie_namespace, secret_artifact_hash, source_revision, installed_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(source_identity, version) DO NOTHING",
            )
            .bind::<Text, _>(&source_identity)
            .bind::<Text, _>(&version)
            .bind::<Text, _>(&profile_json)
            .bind::<Text, _>(&grant_json)
            .bind::<Text, _>(&base_url)
            .bind::<Text, _>(&package_hash)
            .bind::<Text, _>(&plan_artifact_hash)
            .bind::<Text, _>(&definition_hash)
            .bind::<Text, _>(&plan_hash)
            .bind::<Text, _>(&cookie_namespace)
            .bind::<Nullable<Text>, _>(secret_artifact_hash.as_deref())
            .bind::<BigInt, _>(to_i64(source_revision)?)
            .bind::<BigInt, _>(event.occurred_at_ms)
            .execute(conn)
            .map_err(database_error)?;
            remove_staged_source_credential(conn, candidate_id)?;
            remove_candidate_event_refs(conn, candidate_id)?;
            sql_query("UPDATE candidates SET status = 'installed' WHERE candidate_id = ?")
                .bind::<Text, _>(candidate_id.to_string())
                .execute(conn)
                .map_err(database_error)?;
            Ok(())
        },
    )?;
    Ok(InstalledSource {
        source_identity: candidate.source_identity,
        version,
        package,
        plan,
        profile,
        grant: request.grant,
        revision: receipt.stream_version,
    })
}

fn load_and_validate_candidate_artifacts(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    candidate: &CandidateRow,
) -> Result<
    (
        lj_rule_model::RulePackage,
        lj_rule_model::ExecutionPlan,
        SourceProfile,
        PolicyCapabilities,
    ),
    StorageError,
> {
    let package_bytes = read_body_by_hash(conn, artifacts, &candidate.package_artifact_hash)
        .map_err(|_| StorageError::CandidateTampered)?;
    let plan_bytes = read_body_by_hash(conn, artifacts, &candidate.plan_artifact_hash)
        .map_err(|_| StorageError::CandidateTampered)?;
    let package = deserialize::<lj_rule_model::RulePackage>(&package_bytes)
        .map_err(|_| StorageError::CandidateTampered)?;
    let plan = deserialize::<lj_rule_model::ExecutionPlan>(&plan_bytes)
        .map_err(|_| StorageError::CandidateTampered)?;
    validate_candidate_package_and_plan(&package, &plan)
        .map_err(|_| StorageError::CandidateTampered)?;
    if package.source_identity.id != candidate.source_identity
        || package.definition.source_identity != package.source_identity
        || plan.plan_hash != candidate.plan_hash
        || plan.definition_hash != candidate.definition_hash
    {
        return Err(StorageError::CandidateTampered);
    }
    let profile = deserialize::<SourceProfile>(candidate.profile_json.as_bytes())
        .map_err(|_| StorageError::CandidateTampered)?;
    if profile.id.0 != candidate.source_identity
        || profile.version.as_deref() != Some(package.version.as_str())
    {
        return Err(StorageError::CandidateTampered);
    }
    let required_grant =
        deserialize::<PolicyCapabilities>(candidate.required_grant_json.as_bytes())
            .map_err(|_| StorageError::CandidateTampered)?;
    if required_grant != package.definition.capability_manifest.required {
        return Err(StorageError::CandidateTampered);
    }
    Ok((package, plan, profile, required_grant))
}

fn validate_staged_candidate_event(
    conn: &mut SqliteConnection,
    candidate_id: Uuid,
    candidate: &CandidateRow,
) -> Result<(), StorageError> {
    let candidate_id_text = candidate_id.to_string();
    let event = sql_query(
        "SELECT stream_version, event_id, source_identity, event_type, schema_version, payload_json, artifact_refs_json, secret_refs_json FROM events WHERE stream_id = ? AND stream_version = 1",
    )
    .bind::<Text, _>(candidate_stream_id(candidate_id))
    .get_result::<CandidateStagedEventRow>(conn)
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::CandidateTampered)?;
    let payload = serde_json::from_str::<serde_json::Value>(&event.payload_json)
        .map_err(|_| StorageError::CandidateTampered)?;
    let event_type = serde_json::from_str::<EventType>(&event.event_type)
        .map_err(|_| StorageError::CandidateTampered)?;
    let artifact_refs = serde_json::from_str::<Vec<ArtifactRef>>(&event.artifact_refs_json)
        .map_err(|_| StorageError::CandidateTampered)?;
    let artifact_refs_use_zstd = artifact_refs
        .iter()
        .all(|artifact| artifact.codec == "zstd");
    let secret_refs = serde_json::from_str::<Vec<SecretRef>>(&event.secret_refs_json)
        .map_err(|_| StorageError::CandidateTampered)?;
    let mut actual_artifact_hashes = artifact_refs
        .into_iter()
        .map(|artifact| artifact.hash)
        .collect::<Vec<_>>();
    actual_artifact_hashes.sort_unstable();
    let mut expected_artifact_hashes = vec![
        candidate.package_artifact_hash.clone(),
        candidate.plan_artifact_hash.clone(),
    ];
    expected_artifact_hashes.sort_unstable();
    let profile_hash = blake3::hash(candidate.profile_json.as_bytes())
        .to_hex()
        .to_string();
    let grant_hash = blake3::hash(candidate.required_grant_json.as_bytes())
        .to_hex()
        .to_string();
    let diagnostics_hash = blake3::hash(candidate.diagnostics_json.as_bytes())
        .to_hex()
        .to_string();
    if event.stream_version != 1
        || event.event_id != candidate_id_text
        || event.source_identity.as_deref() != Some(candidate.source_identity.as_str())
        || event_type != EventType::Candidate
        || event.schema_version != 2
        || payload.get("kind").and_then(serde_json::Value::as_str) != Some("staged")
        || payload
            .get("candidate_id")
            .and_then(serde_json::Value::as_str)
            != Some(candidate_id_text.as_str())
        || payload
            .get("source_identity")
            .and_then(serde_json::Value::as_str)
            != Some(candidate.source_identity.as_str())
        || payload
            .get("definition_hash")
            .and_then(serde_json::Value::as_str)
            != Some(candidate.definition_hash.as_str())
        || payload.get("plan_hash").and_then(serde_json::Value::as_str)
            != Some(candidate.plan_hash.as_str())
        || payload
            .get("profile_hash")
            .and_then(serde_json::Value::as_str)
            != Some(profile_hash.as_str())
        || payload
            .get("required_grant_hash")
            .and_then(serde_json::Value::as_str)
            != Some(grant_hash.as_str())
        || payload
            .get("diagnostics_hash")
            .and_then(serde_json::Value::as_str)
            != Some(diagnostics_hash.as_str())
        || payload
            .get("artifact_hash_algorithm")
            .and_then(serde_json::Value::as_str)
            != Some("blake3")
        || payload
            .get("expires_at_ms")
            .and_then(serde_json::Value::as_i64)
            != Some(candidate.expires_at_ms)
        || actual_artifact_hashes != expected_artifact_hashes
        || !artifact_refs_use_zstd
        || !secret_refs.is_empty()
    {
        return Err(StorageError::CandidateTampered);
    }
    Ok(())
}

/// 读取 opaque candidate 的安全 preview；没有 candidate 时返回 `Ok(None)`。
pub(crate) fn get_candidate_summary(
    conn: &mut SqliteConnection,
    candidate_id: Uuid,
) -> Result<Option<CandidateSummary>, StorageError> {
    let row = get_candidate_sync(conn, candidate_id)?;
    row.map(|value| {
        Ok(CandidateSummary {
            candidate_id,
            source_identity: value.source_identity,
            profile: deserialize(value.profile_json.as_bytes())?,
            required_grant: deserialize(value.required_grant_json.as_bytes())?,
            diagnostics: deserialize(value.diagnostics_json.as_bytes())?,
            definition_hash: value.definition_hash,
            plan_hash: value.plan_hash,
            expires_at_ms: value.expires_at_ms,
        })
    })
    .transpose()
}

fn get_candidate_sync(
    conn: &mut SqliteConnection,
    candidate_id: Uuid,
) -> Result<Option<CandidateRow>, StorageError> {
    sql_query(
        "SELECT source_identity, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, profile_json, required_grant_json, diagnostics_json, expires_at_ms, status, source_credentials_required FROM candidates WHERE candidate_id = ?",
    )
    .bind::<Text, _>(candidate_id.to_string())
    .get_result::<CandidateRow>(conn)
    .optional()
    .map_err(database_error)
}

/// 读取 staged credential ref，不解密或返回 credential plaintext。
pub(crate) fn get_candidate_source_credentials_ref_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    candidate_id: Uuid,
) -> Result<Option<SourceCredentialSnapshot>, StorageError> {
    let Some(candidate) = get_candidate_sync(conn, candidate_id)? else {
        return Ok(None);
    };
    let staged = get_staged_source_credential(conn, candidate_id)?;
    if candidate.status != "staged" {
        return Ok(None);
    }
    let Some(staged) = staged else {
        return if candidate.source_credentials_required != 0 {
            Err(StorageError::SourceCredentialUnavailable)
        } else {
            Ok(None)
        };
    };
    if staged.source_identity != candidate.source_identity {
        return Err(StorageError::SourceCredentialUnavailable);
    }
    ensure_blake3_hash(
        &staged.secret_artifact_hash,
        "source credential secret hash",
    )?;
    let artifact = artifact_row(conn, &staged.secret_artifact_hash, ArtifactKind::Secret)?
        .ok_or_else(|| StorageError::ArtifactUnavailable(staged.secret_artifact_hash.clone()))?;
    if artifact.encryption.as_deref() != Some(SECRET_ALGORITHM) {
        return Err(StorageError::SourceCredentialUnavailable);
    }
    artifacts
        .ensure_secret_artifact_exists(&staged.secret_artifact_hash, &artifact.relative_path)?;
    Ok(Some(SourceCredentialSnapshot {
        cookie_namespace: staged.cookie_namespace,
        secret_ref: SecretRef {
            hash: staged.secret_artifact_hash,
            algorithm: SECRET_ALGORITHM.to_string(),
        },
    }))
}

fn get_staged_source_credential(
    conn: &mut SqliteConnection,
    candidate_id: Uuid,
) -> Result<Option<SourceCredentialStagingRow>, StorageError> {
    sql_query(
        "SELECT source_identity, cookie_namespace, secret_artifact_hash, expires_at_ms FROM source_credential_staging WHERE candidate_id = ?",
    )
    .bind::<Text, _>(candidate_id.to_string())
    .get_result::<SourceCredentialStagingRow>(conn)
    .optional()
    .map_err(database_error)
}

pub(crate) fn remove_staged_source_credential(
    conn: &mut SqliteConnection,
    candidate_id: Uuid,
) -> Result<(), StorageError> {
    let Some(staged) = get_staged_source_credential(conn, candidate_id)? else {
        return Ok(());
    };
    let deleted = sql_query("DELETE FROM source_credential_staging WHERE candidate_id = ?")
        .bind::<Text, _>(candidate_id.to_string())
        .execute(conn)
        .map_err(database_error)?;
    if deleted != 1 {
        return Err(StorageError::Database(
            "source credential staging 删除不一致".to_string(),
        ));
    }
    decrement_artifact_ref(conn, &staged.secret_artifact_hash, "secret")
}

fn mark_candidate_source_credentials_required(
    conn: &mut SqliteConnection,
    candidate_id: Uuid,
) -> Result<(), StorageError> {
    let changed = sql_query(
        "UPDATE candidates SET source_credentials_required = 1 WHERE candidate_id = ? AND status = 'staged'",
    )
    .bind::<Text, _>(candidate_id.to_string())
    .execute(conn)
    .map_err(database_error)?;
    if changed == 1 {
        Ok(())
    } else {
        Err(StorageError::CandidateUnavailable)
    }
}

/// 读取 source projection 行，供 install、execution pin 与 checkpoint 使用。
pub(crate) fn get_source_row(
    conn: &mut SqliteConnection,
    source_identity: &str,
) -> Result<Option<SourceRow>, StorageError> {
    sql_query(
        "SELECT source_identity, version, profile_json, grant_json, package_artifact_hash, plan_artifact_hash, plan_hash, revision FROM source_projection WHERE source_identity = ?",
    )
    .bind::<Text, _>(source_identity)
    .get_result::<SourceRow>(conn)
    .optional()
    .map_err(database_error)
}

/// 从当前 source projection 读取 author package 与 immutable Plan。
pub(crate) fn get_installed_source_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    source_identity: &str,
) -> Result<Option<InstalledSource>, StorageError> {
    let Some(row) = get_source_row(conn, source_identity)? else {
        return Ok(None);
    };
    let package = deserialize::<lj_rule_model::RulePackage>(&read_body_by_hash(
        conn,
        artifacts,
        &row.package_artifact_hash,
    )?)?;
    let plan = deserialize::<lj_rule_model::ExecutionPlan>(&read_body_by_hash(
        conn,
        artifacts,
        &row.plan_artifact_hash,
    )?)?;
    let profile = deserialize::<SourceProfile>(row.profile_json.as_bytes())?;
    let grant = deserialize::<lj_rule_model::PolicyCapabilities>(row.grant_json.as_bytes())?;
    Ok(Some(InstalledSource {
        source_identity: row.source_identity,
        version: row.version,
        package,
        plan,
        profile,
        grant,
        revision: from_i64(row.revision, "source revision")?,
    }))
}

/// 按 source identity 读取不含 Plan/secret 的安全来源记录。
pub(crate) fn list_installed_sources_sync(
    conn: &mut SqliteConnection,
) -> Result<Vec<InstalledSourceRecord>, StorageError> {
    let rows = sql_query(
        "SELECT source_identity, version, profile_json, grant_json, revision FROM source_projection ORDER BY source_identity ASC",
    )
    .load::<InstalledSourceRecordRow>(conn)
    .map_err(database_error)?;
    rows.into_iter()
        .map(installed_source_record_from_row)
        .collect()
}

fn installed_source_record_from_row(
    row: InstalledSourceRecordRow,
) -> Result<InstalledSourceRecord, StorageError> {
    let profile = deserialize::<SourceProfile>(row.profile_json.as_bytes())?;
    if profile.id.0 != row.source_identity {
        return Err(StorageError::InvalidInput(
            "source projection profile identity 与行键不一致".to_string(),
        ));
    }
    Ok(InstalledSourceRecord {
        source_identity: row.source_identity,
        version: row.version,
        profile,
        grant: deserialize::<PolicyCapabilities>(row.grant_json.as_bytes())?,
        revision: from_i64(row.revision, "source revision")?,
    })
}

fn installed_source_from_event(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    source_identity: &str,
    _revision: u64,
) -> Result<InstalledSource, StorageError> {
    get_installed_source_sync(conn, artifacts, source_identity)?.ok_or(StorageError::SourceMissing)
}

/// 过期 candidate 的 Event 与 staging refs 一起删除；事务可安全重试。
pub(crate) fn expire_candidate(
    conn: &mut SqliteConnection,
    candidate_id: Uuid,
) -> Result<(), StorageError> {
    conn.immediate_transaction::<_, StorageError, _>(|conn| {
        remove_staged_source_credential(conn, candidate_id)?;
        remove_candidate_event_refs(conn, candidate_id)?;
        sql_query("UPDATE candidates SET status = 'expired' WHERE candidate_id = ?")
            .bind::<Text, _>(candidate_id.to_string())
            .execute(conn)
            .map_err(database_error)?;
        Ok(())
    })
}

fn validate_candidate(draft: &CandidateDraft) -> Result<(), StorageError> {
    let source = &draft.package.source_identity.id;
    if source.is_empty()
        || draft.package.definition.source_identity.id != *source
        || draft.profile.id.0 != *source
    {
        return Err(StorageError::InvalidInput(
            "candidate 来源身份不一致".to_string(),
        ));
    }
    validate_candidate_package_and_plan(&draft.package, &draft.plan)
}

pub(crate) fn grant_covers(grant: &PolicyCapabilities, required: &PolicyCapabilities) -> bool {
    (!required.network || grant.network)
        && (!required.system.fs || grant.system.fs)
        && (!required.system.env || grant.system.env)
        && (!required.system.process || grant.system.process)
}

pub(crate) fn canonical_plan_hash(plan: &ExecutionPlan) -> Result<String, StorageError> {
    let mut plan_to_hash = plan.clone();
    plan_to_hash.plan_hash.clear();
    Ok(blake3::hash(
        canonical_json(&plan_to_hash)
            .map_err(|_| StorageError::Serialization)?
            .as_bytes(),
    )
    .to_hex()
    .to_string())
}

pub(crate) fn validate_candidate_package_and_plan(
    package: &lj_rule_model::RulePackage,
    plan: &ExecutionPlan,
) -> Result<(), StorageError> {
    let expected_definition_hash =
        definition_hash(&package.definition).map_err(|_| StorageError::Serialization)?;
    if plan.definition_hash != expected_definition_hash {
        return Err(StorageError::InvalidInput(
            "candidate Definition hash 与 canonical package 不一致".to_string(),
        ));
    }

    let expected_plan_hash = canonical_plan_hash(plan)?;
    if plan.plan_hash != expected_plan_hash {
        return Err(StorageError::InvalidInput(
            "candidate Plan hash 与 canonical plan 不一致".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn candidate_stream_id(candidate_id: Uuid) -> String {
    format!("candidate/{candidate_id}")
}

pub(crate) fn source_stream_id(source_identity: &str) -> String {
    format!("source/{source_identity}")
}

pub(crate) fn source_cookie_namespace(source_identity: &str) -> String {
    format!("source/{source_identity}")
}

fn source_install_links(
    package_artifact_hash: &str,
    plan_artifact_hash: &str,
    secret_artifact_hash: Option<&str>,
) -> Vec<ArtifactLink> {
    let mut links = Vec::with_capacity(usize::from(secret_artifact_hash.is_some()) + 2);
    links.push(ArtifactLink::Existing {
        hash: package_artifact_hash.to_string(),
        kind: ArtifactKind::Body,
    });
    links.push(ArtifactLink::Existing {
        hash: plan_artifact_hash.to_string(),
        kind: ArtifactKind::Body,
    });
    if let Some(hash) = secret_artifact_hash {
        links.push(ArtifactLink::Existing {
            hash: hash.to_string(),
            kind: ArtifactKind::Secret,
        });
    }
    links
}

#[derive(QueryableByName)]
struct CandidateStagedEventRow {
    #[diesel(sql_type = BigInt)]
    stream_version: i64,
    #[diesel(sql_type = Text)]
    event_id: String,
    #[diesel(sql_type = Nullable<Text>)]
    source_identity: Option<String>,
    #[diesel(sql_type = Text)]
    event_type: String,
    #[diesel(sql_type = Integer)]
    schema_version: i32,
    #[diesel(sql_type = Text)]
    payload_json: String,
    #[diesel(sql_type = Text)]
    artifact_refs_json: String,
    #[diesel(sql_type = Text)]
    secret_refs_json: String,
}

#[derive(QueryableByName)]
struct CandidateRow {
    #[diesel(sql_type = Text)]
    source_identity: String,
    #[diesel(sql_type = Text)]
    package_artifact_hash: String,
    #[diesel(sql_type = Text)]
    plan_artifact_hash: String,
    #[diesel(sql_type = Text)]
    definition_hash: String,
    #[diesel(sql_type = Text)]
    plan_hash: String,
    #[diesel(sql_type = Text)]
    profile_json: String,
    #[diesel(sql_type = Text)]
    required_grant_json: String,
    #[diesel(sql_type = Text)]
    diagnostics_json: String,
    #[diesel(sql_type = BigInt)]
    expires_at_ms: i64,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Integer)]
    source_credentials_required: i32,
}

#[derive(QueryableByName)]
struct SourceCredentialStagingRow {
    #[diesel(sql_type = Text)]
    source_identity: String,
    #[diesel(sql_type = Text)]
    cookie_namespace: String,
    #[diesel(sql_type = Text)]
    secret_artifact_hash: String,
    #[diesel(sql_type = BigInt)]
    expires_at_ms: i64,
}

#[derive(QueryableByName)]
pub(crate) struct SourceRow {
    #[diesel(sql_type = Text)]
    pub(crate) source_identity: String,
    #[diesel(sql_type = Text)]
    pub(crate) version: String,
    #[diesel(sql_type = Text)]
    pub(crate) profile_json: String,
    #[diesel(sql_type = Text)]
    pub(crate) grant_json: String,
    #[diesel(sql_type = Text)]
    pub(crate) package_artifact_hash: String,
    #[diesel(sql_type = Text)]
    pub(crate) plan_artifact_hash: String,
    #[diesel(sql_type = Text)]
    pub(crate) plan_hash: String,
    #[diesel(sql_type = BigInt)]
    pub(crate) revision: i64,
}

#[derive(QueryableByName)]
struct InstalledSourceRecordRow {
    #[diesel(sql_type = Text)]
    source_identity: String,
    #[diesel(sql_type = Text)]
    version: String,
    #[diesel(sql_type = Text)]
    profile_json: String,
    #[diesel(sql_type = Text)]
    grant_json: String,
    #[diesel(sql_type = BigInt)]
    revision: i64,
}
