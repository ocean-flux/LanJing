//! Candidate-v2 composite publish、source install/update 与 revision-keyed snapshot。
//!
//! 一个 writer command 先 durable 写完 package/Plan 与全部随机 secret 文件，再由同一个 Event
//! transaction 发布 candidate row/owners。install 重新验证 schema、expiry、contract hash、document
//! 与 installed-source baseline，在一个 source Event transaction 中消费 candidate、追加 source
//! revision、固定 document snapshot/runtime credential ownership；失败不推进任一 projection。

use std::str::FromStr;

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Integer, Nullable, Text};
use diesel::sqlite::SqliteConnection;
use lj_media::SourceProfile;
#[cfg(test)]
use lj_rule_model::ContractSchemaVersion;
use lj_rule_model::{
    ArtifactRef, CREDENTIAL_SCHEMA_VERSION, EventType, ExecutionPlan, PolicyCapabilities,
    RulePackage, SchemaReadError, SecretRef, SourceDocumentFormat, definition_hash,
    execution_plan_hash, read_execution_plan, read_rule_package,
};
use uuid::Uuid;

use crate::artifact::{ArtifactStore, PendingSecretArtifact};
use crate::document_vault::{format_as_db, format_from_db, revision_owner_id};
use crate::event_store::{
    ArtifactLink, EventDraft, append_event_transaction, database_error, deserialize, from_i64,
    idempotent_event, read_body_by_hash, remove_candidate_event_refs, serialize, to_i64,
};
use crate::projection_query::upsert_projection_source;
use crate::secret_artifact::{
    read_owned_secret, release_secret_owner, retain_existing_secret, retain_pending_secret,
    write_secret,
};
use crate::types::{
    ArtifactKind, CandidateDocumentInput, CandidateDraft, CandidateSummary,
    DEFAULT_CANDIDATE_TTL_MS, DocumentRef, INSTALL_CANDIDATE_SCHEMA_VERSION,
    InstallCandidateRequest, InstalledSource, InstalledSourceRecord, MAX_SOURCE_DOCUMENT_BYTES,
    SOURCE_DOCUMENT_SCHEMA_VERSION, SecretArtifactId, SourceDocumentId, StorageError,
    TransientSourceDocumentInput,
};

struct SavedCandidateDocument {
    document_ref: DocumentRef,
    format: SourceDocumentFormat,
    masked_hash: String,
    raw_secret_id: SecretArtifactId,
    manifest_secret_id: SecretArtifactId,
}

struct TransientCandidateDocument {
    format: SourceDocumentFormat,
    masked_hash: String,
    raw_secret: PendingSecretArtifact,
    manifest_secret: PendingSecretArtifact,
}

enum PreparedCandidateDocument {
    Saved(SavedCandidateDocument),
    Transient(TransientCandidateDocument),
}

impl PreparedCandidateDocument {
    fn format(&self) -> SourceDocumentFormat {
        match self {
            Self::Saved(value) => value.format,
            Self::Transient(value) => value.format,
        }
    }

    fn document_ref(&self) -> Option<DocumentRef> {
        match self {
            Self::Saved(value) => Some(value.document_ref),
            Self::Transient(_) => None,
        }
    }

    fn is_transient(&self) -> bool {
        matches!(self, Self::Transient(_))
    }

    fn masked_hash(&self) -> &str {
        match self {
            Self::Saved(value) => &value.masked_hash,
            Self::Transient(value) => &value.masked_hash,
        }
    }

    fn raw_secret_id(&self) -> SecretArtifactId {
        match self {
            Self::Saved(value) => value.raw_secret_id,
            Self::Transient(value) => value.raw_secret.secret_id,
        }
    }

    fn manifest_secret_id(&self) -> SecretArtifactId {
        match self {
            Self::Saved(value) => value.manifest_secret_id,
            Self::Transient(value) => value.manifest_secret.secret_id,
        }
    }
}

/// 单 writer composite publish candidate-v2。
pub(crate) fn process_stage_candidate(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    draft: CandidateDraft,
) -> Result<CandidateSummary, StorageError> {
    if get_candidate_sync(conn, draft.candidate_id)?.is_some() {
        return Err(StorageError::CandidateUnavailable);
    }
    let expires_at_ms = validate_candidate_draft(&draft)?;
    let profile_json = serialize(&draft.profile)?;
    let grant_json = serialize(&draft.required_grant)?;
    let diagnostics_json = serialize(&draft.diagnostics)?;
    let package_bytes =
        serde_json::to_vec(&draft.package).map_err(|_| StorageError::Serialization)?;
    let plan_bytes = serde_json::to_vec(&draft.plan).map_err(|_| StorageError::Serialization)?;

    let prepared_document =
        prepare_candidate_document(conn, artifacts, draft.document, draft.created_at_ms)?;
    let runtime_secret = draft
        .runtime_credentials
        .as_ref()
        .map(|material| {
            write_secret(
                conn,
                artifacts,
                material.expose_bytes(),
                draft.created_at_ms,
            )
        })
        .transpose()?;
    let package_artifact = artifacts.write(ArtifactKind::Body, &package_bytes)?;
    let plan_artifact = artifacts.write(ArtifactKind::Body, &plan_bytes)?;
    let source_identity = draft.package.source_identity().id.clone();
    let document_ref = prepared_document.document_ref();
    let contract_hash = candidate_contract_hash(&CandidateContractHashInput {
        candidate_id: draft.candidate_id,
        source_identity: &source_identity,
        document_ref,
        transient: prepared_document.is_transient(),
        format: prepared_document.format(),
        masked_hash: prepared_document.masked_hash(),
        raw_secret_id: prepared_document.raw_secret_id(),
        manifest_secret_id: prepared_document.manifest_secret_id(),
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
            "document_schema_version": SOURCE_DOCUMENT_SCHEMA_VERSION,
            "contract_hash": contract_hash,
            "expires_at_ms": expires_at_ms,
        }),
        source_identity: Some(source_identity.clone()),
    };
    let links = vec![
        ArtifactLink::New(package_artifact),
        ArtifactLink::New(plan_artifact),
    ];
    append_event_transaction(conn, &event, &links, |conn, _, stream_revision| {
        retain_candidate_document(
            conn,
            artifacts,
            draft.candidate_id,
            &prepared_document,
            draft.created_at_ms,
        )?;
        if let Some(runtime_secret) = &runtime_secret {
            retain_pending_secret(
                conn,
                runtime_secret,
                "candidate_runtime",
                &draft.candidate_id.to_string(),
                draft.created_at_ms,
            )?;
        }
        sql_query(
            "INSERT INTO candidate_projection (candidate_id, candidate_schema_version, document_schema_version, source_format, document_id, document_revision, transient, document_masked_hash, raw_secret_id, manifest_secret_id, runtime_credential_secret_id, target_source_identity, expected_installed_revision, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, profile_json, required_grant_json, diagnostics_json, expires_at_ms, status, stream_version, created_at_ms, consumed_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'staged', ?, ?, NULL)",
        )
        .bind::<Text, _>(draft.candidate_id.to_string())
        .bind::<Integer, _>(i32::try_from(INSTALL_CANDIDATE_SCHEMA_VERSION).map_err(|_| StorageError::CandidateSchemaMismatch)?)
        .bind::<Integer, _>(i32::try_from(SOURCE_DOCUMENT_SCHEMA_VERSION).map_err(|_| StorageError::CandidateSchemaMismatch)?)
        .bind::<Text, _>(format_as_db(prepared_document.format()))
        .bind::<Nullable<Text>, _>(document_ref.map(|value| value.document_id.to_string()).as_deref())
        .bind::<Nullable<BigInt>, _>(document_ref.map(|value| to_i64(value.document_revision)).transpose()?)
        .bind::<Integer, _>(i32::from(prepared_document.is_transient()))
        .bind::<Text, _>(prepared_document.masked_hash())
        .bind::<Text, _>(prepared_document.raw_secret_id().to_string())
        .bind::<Text, _>(prepared_document.manifest_secret_id().to_string())
        .bind::<Nullable<Text>, _>(runtime_secret.as_ref().map(|value| value.secret_id.to_string()).as_deref())
        .bind::<Text, _>(&source_identity)
        .bind::<BigInt, _>(to_i64(draft.expected_installed_revision)?)
        .bind::<Text, _>(match &links[0] { ArtifactLink::New(value) => &value.hash, ArtifactLink::Existing { .. } => unreachable!() })
        .bind::<Text, _>(match &links[1] { ArtifactLink::New(value) => &value.hash, ArtifactLink::Existing { .. } => unreachable!() })
        .bind::<Text, _>(draft.plan.definition_hash())
        .bind::<Text, _>(draft.plan.plan_hash())
        .bind::<Text, _>(&profile_json)
        .bind::<Text, _>(&grant_json)
        .bind::<Text, _>(&diagnostics_json)
        .bind::<BigInt, _>(expires_at_ms)
        .bind::<BigInt, _>(to_i64(stream_revision)?)
        .bind::<BigInt, _>(draft.created_at_ms)
        .execute(conn)
        .map_err(database_error)?;
        Ok(())
    })?;
    Ok(CandidateSummary {
        candidate_id: draft.candidate_id,
        source_identity,
        document_ref,
        transient: prepared_document.is_transient(),
        expected_installed_revision: draft.expected_installed_revision,
        profile: draft.profile,
        required_grant: draft.required_grant,
        diagnostics: draft.diagnostics,
        definition_hash: draft.plan.definition_hash().to_string(),
        plan_hash: draft.plan.plan_hash().to_string(),
        expires_at_ms,
    })
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

fn prepare_candidate_document(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    input: CandidateDocumentInput,
    created_at_ms: i64,
) -> Result<PreparedCandidateDocument, StorageError> {
    match input {
        CandidateDocumentInput::Saved(document_ref) => {
            if document_ref.document_revision == 0 {
                return Err(StorageError::InvalidInput(
                    "document revision 必须大于 0".to_string(),
                ));
            }
            let row = source_document_candidate_row(conn, document_ref.document_id)?
                .ok_or(StorageError::DocumentMissing)?;
            let current_revision = from_i64(row.current_document_revision, "document revision")?;
            if current_revision != document_ref.document_revision
                || row.schema_version != i64::from(SOURCE_DOCUMENT_SCHEMA_VERSION)
            {
                return Err(StorageError::CandidateStale);
            }
            let raw_secret_id = SecretArtifactId::from_str(&row.raw_secret_id)?;
            let masked_secret_id = SecretArtifactId::from_str(&row.masked_secret_id)?;
            read_owned_secret(
                conn,
                artifacts,
                masked_secret_id,
                "document_masked",
                &revision_owner_id(
                    document_ref.document_id,
                    document_ref.document_revision,
                    "masked",
                ),
            )?;
            let manifest_secret_id = SecretArtifactId::from_str(&row.manifest_secret_id)?;
            read_owned_secret(
                conn,
                artifacts,
                raw_secret_id,
                "document_raw",
                &revision_owner_id(
                    document_ref.document_id,
                    document_ref.document_revision,
                    "raw",
                ),
            )?;
            read_owned_secret(
                conn,
                artifacts,
                manifest_secret_id,
                "document_manifest",
                &revision_owner_id(
                    document_ref.document_id,
                    document_ref.document_revision,
                    "manifest",
                ),
            )?;
            Ok(PreparedCandidateDocument::Saved(SavedCandidateDocument {
                document_ref,
                format: format_from_db(&row.format)?,
                masked_hash: row.masked_hash,
                raw_secret_id,
                manifest_secret_id,
            }))
        }
        CandidateDocumentInput::Transient(input) => {
            validate_transient_document(&input)?;
            let manifest_bytes =
                serde_json::to_vec(&input.manifest).map_err(|_| StorageError::Serialization)?;
            let raw_secret =
                write_secret(conn, artifacts, input.raw_text.as_bytes(), created_at_ms)?;
            let manifest_secret = write_secret(conn, artifacts, &manifest_bytes, created_at_ms)?;
            Ok(PreparedCandidateDocument::Transient(
                TransientCandidateDocument {
                    format: input.format,
                    masked_hash: blake3::hash(input.masked_text.as_bytes())
                        .to_hex()
                        .to_string(),
                    raw_secret,
                    manifest_secret,
                },
            ))
        }
    }
}

fn validate_transient_document(input: &TransientSourceDocumentInput) -> Result<(), StorageError> {
    if input.masked_text.len() > MAX_SOURCE_DOCUMENT_BYTES
        || input.raw_text.len() > MAX_SOURCE_DOCUMENT_BYTES
        || input.manifest.schema_version != CREDENTIAL_SCHEMA_VERSION
        || input.manifest.target.format != input.format
        || input.manifest.target.revision == 0
    {
        return Err(StorageError::InvalidInput(
            "transient 来源文档无效或超过 2 MiB".to_string(),
        ));
    }
    Ok(())
}

fn retain_candidate_document(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    candidate_id: Uuid,
    document: &PreparedCandidateDocument,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    let owner_id = candidate_id.to_string();
    match document {
        PreparedCandidateDocument::Saved(value) => {
            retain_existing_secret(
                conn,
                artifacts,
                value.raw_secret_id,
                "candidate_raw",
                &owner_id,
                created_at_ms,
            )?;
            retain_existing_secret(
                conn,
                artifacts,
                value.manifest_secret_id,
                "candidate_manifest",
                &owner_id,
                created_at_ms,
            )
        }
        PreparedCandidateDocument::Transient(value) => {
            retain_pending_secret(
                conn,
                &value.raw_secret,
                "candidate_raw",
                &owner_id,
                created_at_ms,
            )?;
            retain_pending_secret(
                conn,
                &value.manifest_secret,
                "candidate_manifest",
                &owner_id,
                created_at_ms,
            )
        }
    }
}

/// 原子消费 candidate-v2 并追加权威 source revision。
pub(crate) fn process_install_candidate(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    request: InstallCandidateRequest,
) -> Result<InstalledSource, StorageError> {
    let candidate =
        get_candidate_sync(conn, request.candidate_id)?.ok_or(StorageError::CandidateMissing)?;
    if candidate.expires_at_ms <= request.occurred_at_ms {
        expire_candidate(conn, request.candidate_id)?;
        return Err(StorageError::CandidateExpired);
    }
    validate_candidate_status_and_schema(&candidate, request.occurred_at_ms)?;
    validate_staged_candidate_event(conn, request.candidate_id, &candidate)?;
    let current_source = get_source_row(conn, &candidate.target_source_identity)?;
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
    let document_ref = candidate_document_ref(&candidate)?;
    let document_row = if let Some(document_ref) = document_ref {
        let row = source_document_candidate_row(conn, document_ref.document_id)?
            .ok_or(StorageError::CandidateStale)?;
        if from_i64(row.current_document_revision, "document revision")?
            != document_ref.document_revision
            || row.schema_version != i64::from(SOURCE_DOCUMENT_SCHEMA_VERSION)
            || row.masked_hash != candidate.document_masked_hash
            || row.raw_secret_id != candidate.raw_secret_id
            || row.manifest_secret_id != candidate.manifest_secret_id
            || row
                .source_identity
                .as_deref()
                .is_some_and(|identity| identity != candidate.target_source_identity)
        {
            return Err(StorageError::CandidateStale);
        }
        Some(row)
    } else {
        None
    };
    ensure_candidate_secrets(conn, artifacts, request.candidate_id, &candidate)?;
    let (package, plan, profile, required_grant) =
        load_and_validate_candidate_artifacts(conn, artifacts, &candidate)?;
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
            "document_id": document_ref.map(|value| value.document_id),
            "document_revision": document_ref.map(|value| value.document_revision),
            "document_masked_hash": &candidate.document_masked_hash,
            "schema_version": INSTALL_CANDIDATE_SCHEMA_VERSION,
        }),
        source_identity: Some(source_identity.clone()),
    };
    if let Some(receipt) = idempotent_event(conn, &event)? {
        return installed_source_from_revision(
            conn,
            artifacts,
            &source_identity,
            receipt.stream_version,
        );
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
        artifacts,
        profile: &profile,
        source_identity: &source_identity,
        version: &version,
        profile_json: &profile_json,
        grant_json: &grant_json,
        base_url: &base_url,
        package_hash: &package_hash,
        plan_artifact_hash: &plan_artifact_hash,
        definition_hash: &definition_hash,
        plan_hash: &plan_hash,
        cookie_namespace: &cookie_namespace,
        runtime_secret_id,
        document_ref,
        document_row: document_row.as_ref(),
        document_masked_hash: &candidate.document_masked_hash,
        candidate_id: request.candidate_id,
        occurred_at_ms: event.occurred_at_ms,
    };
    let receipt =
        append_event_transaction(conn, &event, &links, |conn, global_seq, source_revision| {
            commit_candidate_install(conn, global_seq, source_revision, &commit)
        })?;
    Ok(InstalledSource {
        source_identity,
        version,
        package,
        plan,
        profile,
        grant: request.grant,
        source_revision: receipt.stream_version,
        document_id: document_ref.map(|value| value.document_id),
        document_revision: document_ref.map(|value| value.document_revision),
    })
}

struct CandidateInstallCommit<'a> {
    artifacts: &'a ArtifactStore,
    profile: &'a SourceProfile,
    source_identity: &'a str,
    version: &'a str,
    profile_json: &'a str,
    grant_json: &'a str,
    base_url: &'a str,
    package_hash: &'a str,
    plan_artifact_hash: &'a str,
    definition_hash: &'a str,
    plan_hash: &'a str,
    cookie_namespace: &'a str,
    runtime_secret_id: Option<SecretArtifactId>,
    document_ref: Option<DocumentRef>,
    document_row: Option<&'a SourceDocumentCandidateRow>,
    document_masked_hash: &'a str,
    candidate_id: Uuid,
    occurred_at_ms: i64,
}

fn commit_candidate_install(
    conn: &mut SqliteConnection,
    global_seq: u64,
    source_revision: u64,
    commit: &CandidateInstallCommit<'_>,
) -> Result<(), StorageError> {
    // 已关联来源只能继续由同一 editable document 更新；transient 或另一 draft 都不能
    // 偷换唯一 working-copy 关系。必须在 source Event transaction 内重新读取 projection，
    // 不能信任 prepare 时的 UI/candidate 状态。
    if let Some(linked_document_id) =
        get_source_row(conn, commit.source_identity)?.and_then(|source| source.document_id)
    {
        let linked_document_id = SourceDocumentId::from_str(&linked_document_id)?;
        if commit.document_ref.map(|value| value.document_id) != Some(linked_document_id) {
            return Err(StorageError::CandidateStale);
        }
    }

    upsert_projection_source(conn, commit.profile, global_seq)?;
    sql_query(
        "INSERT INTO source_projection (source_identity, version, profile_json, grant_json, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, cookie_namespace, runtime_credential_secret_id, document_id, document_revision, revision, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(source_identity) DO UPDATE SET version = excluded.version, profile_json = excluded.profile_json, grant_json = excluded.grant_json, package_artifact_hash = excluded.package_artifact_hash, plan_artifact_hash = excluded.plan_artifact_hash, definition_hash = excluded.definition_hash, plan_hash = excluded.plan_hash, cookie_namespace = excluded.cookie_namespace, runtime_credential_secret_id = excluded.runtime_credential_secret_id, document_id = excluded.document_id, document_revision = excluded.document_revision, revision = excluded.revision, updated_global_seq = excluded.updated_global_seq",
    )
    .bind::<Text, _>(commit.source_identity)
    .bind::<Text, _>(commit.version)
    .bind::<Text, _>(commit.profile_json)
    .bind::<Text, _>(commit.grant_json)
    .bind::<Text, _>(commit.package_hash)
    .bind::<Text, _>(commit.plan_artifact_hash)
    .bind::<Text, _>(commit.definition_hash)
    .bind::<Text, _>(commit.plan_hash)
    .bind::<Text, _>(commit.cookie_namespace)
    .bind::<Nullable<Text>, _>(
        commit
            .runtime_secret_id
            .map(|value| value.to_string())
            .as_deref(),
    )
    .bind::<Nullable<Text>, _>(
        commit
            .document_ref
            .map(|value| value.document_id.to_string())
            .as_deref(),
    )
    .bind::<Nullable<BigInt>, _>(
        commit
            .document_ref
            .map(|value| to_i64(value.document_revision))
            .transpose()?,
    )
    .bind::<BigInt, _>(to_i64(source_revision)?)
    .bind::<BigInt, _>(to_i64(global_seq)?)
    .execute(conn)
    .map_err(database_error)?;
    sql_query(
        "INSERT INTO source_versions (source_identity, source_revision, version, profile_json, grant_json, base_url, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, cookie_namespace, runtime_credential_secret_id, document_id, document_revision, document_masked_hash, schema_version, installed_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(commit.source_identity)
    .bind::<BigInt, _>(to_i64(source_revision)?)
    .bind::<Text, _>(commit.version)
    .bind::<Text, _>(commit.profile_json)
    .bind::<Text, _>(commit.grant_json)
    .bind::<Text, _>(commit.base_url)
    .bind::<Text, _>(commit.package_hash)
    .bind::<Text, _>(commit.plan_artifact_hash)
    .bind::<Text, _>(commit.definition_hash)
    .bind::<Text, _>(commit.plan_hash)
    .bind::<Text, _>(commit.cookie_namespace)
    .bind::<Nullable<Text>, _>(
        commit
            .runtime_secret_id
            .map(|value| value.to_string())
            .as_deref(),
    )
    .bind::<Nullable<Text>, _>(
        commit
            .document_ref
            .map(|value| value.document_id.to_string())
            .as_deref(),
    )
    .bind::<Nullable<BigInt>, _>(
        commit
            .document_ref
            .map(|value| to_i64(value.document_revision))
            .transpose()?,
    )
    .bind::<Nullable<Text>, _>(commit.document_ref.map(|_| commit.document_masked_hash))
    .bind::<Integer, _>(
        i32::try_from(SOURCE_DOCUMENT_SCHEMA_VERSION)
            .map_err(|_| StorageError::InvalidInput("source schema version 无效".to_string()))?,
    )
    .bind::<BigInt, _>(commit.occurred_at_ms)
    .execute(conn)
    .map_err(database_error)?;
    release_secret_owner(conn, "source_projection_runtime", commit.source_identity)?;
    if let Some(runtime_secret_id) = commit.runtime_secret_id {
        retain_existing_secret(
            conn,
            commit.artifacts,
            runtime_secret_id,
            "source_projection_runtime",
            commit.source_identity,
            commit.occurred_at_ms,
        )?;
        retain_existing_secret(
            conn,
            commit.artifacts,
            runtime_secret_id,
            "source_version_runtime",
            &source_version_owner_id(commit.source_identity, source_revision),
            commit.occurred_at_ms,
        )?;
    }
    if let (Some(document_ref), Some(document_row)) = (commit.document_ref, commit.document_row) {
        pin_document_snapshot(
            conn,
            commit.artifacts,
            document_ref,
            document_row,
            commit.source_identity,
            source_revision,
            commit.occurred_at_ms,
        )?;
    }
    release_candidate_secret_owners(conn, commit.candidate_id)?;
    remove_candidate_event_refs(conn, commit.candidate_id)?;
    sql_query("DELETE FROM candidate_projection WHERE candidate_id = ?")
        .bind::<Text, _>(commit.candidate_id.to_string())
        .execute(conn)
        .map_err(database_error)?;
    Ok(())
}

fn pin_document_snapshot(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    document_ref: DocumentRef,
    document: &SourceDocumentCandidateRow,
    source_identity: &str,
    source_revision: u64,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    let masked_secret_id = SecretArtifactId::from_str(&document.masked_secret_id)?;
    let raw_secret_id = SecretArtifactId::from_str(&document.raw_secret_id)?;
    let manifest_secret_id = SecretArtifactId::from_str(&document.manifest_secret_id)?;
    let owner_id = source_version_owner_id(source_identity, source_revision);
    retain_existing_secret(
        conn,
        artifacts,
        masked_secret_id,
        "document_snapshot_masked",
        &owner_id,
        created_at_ms,
    )?;
    retain_existing_secret(
        conn,
        artifacts,
        raw_secret_id,
        "document_snapshot_raw",
        &owner_id,
        created_at_ms,
    )?;
    retain_existing_secret(
        conn,
        artifacts,
        manifest_secret_id,
        "document_snapshot_manifest",
        &owner_id,
        created_at_ms,
    )?;
    sql_query(
        "INSERT INTO source_document_snapshots (document_id, document_revision, source_identity, source_revision, masked_secret_id, raw_secret_id, manifest_secret_id, masked_hash, schema_version, created_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(document_ref.document_id.to_string())
    .bind::<BigInt, _>(to_i64(document_ref.document_revision)?)
    .bind::<Text, _>(source_identity)
    .bind::<BigInt, _>(to_i64(source_revision)?)
    .bind::<Text, _>(masked_secret_id.to_string())
    .bind::<Text, _>(raw_secret_id.to_string())
    .bind::<Text, _>(manifest_secret_id.to_string())
    .bind::<Text, _>(&document.masked_hash)
    .bind::<Integer, _>(i32::try_from(SOURCE_DOCUMENT_SCHEMA_VERSION).map_err(|_| StorageError::InvalidInput("document schema version 无效".to_string()))?)
    .bind::<BigInt, _>(created_at_ms)
    .execute(conn)
    .map_err(database_error)?;
    sql_query(
        "UPDATE source_document_projection SET state = 'linked', source_identity = ?, installed_revision = ?, updated_at_ms = ? WHERE document_id = ? AND current_document_revision = ?",
    )
    .bind::<Text, _>(source_identity)
    .bind::<BigInt, _>(to_i64(source_revision)?)
    .bind::<BigInt, _>(created_at_ms)
    .bind::<Text, _>(document_ref.document_id.to_string())
    .bind::<BigInt, _>(to_i64(document_ref.document_revision)?)
    .execute(conn)
    .map_err(database_error)?;
    Ok(())
}

fn ensure_candidate_secrets(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    candidate_id: Uuid,
    candidate: &CandidateRow,
) -> Result<(), StorageError> {
    let owner_id = candidate_id.to_string();
    let raw = SecretArtifactId::from_str(&candidate.raw_secret_id)?;
    let manifest = SecretArtifactId::from_str(&candidate.manifest_secret_id)?;
    read_owned_secret(conn, artifacts, raw, "candidate_raw", &owner_id)
        .map_err(map_candidate_secret_error)?;
    read_owned_secret(conn, artifacts, manifest, "candidate_manifest", &owner_id)
        .map_err(map_candidate_secret_error)?;
    if let Some(runtime) = candidate.runtime_credential_secret_id.as_deref() {
        read_owned_secret(
            conn,
            artifacts,
            SecretArtifactId::from_str(runtime)?,
            "candidate_runtime",
            &owner_id,
        )
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

fn load_and_validate_candidate_artifacts(
    conn: &mut SqliteConnection,
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
        .map_err(|_| StorageError::CandidateTampered)?;
    let plan_bytes = read_body_by_hash(conn, artifacts, &candidate.plan_artifact_hash)
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
        || candidate.document_schema_version
            != i32::try_from(SOURCE_DOCUMENT_SCHEMA_VERSION)
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

fn validate_staged_candidate_event(
    conn: &mut SqliteConnection,
    candidate_id: Uuid,
    candidate: &CandidateRow,
) -> Result<(), StorageError> {
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
    let document_ref = candidate_document_ref(candidate)?;
    let contract_hash = candidate_contract_hash(&CandidateContractHashInput {
        candidate_id,
        source_identity: &candidate.target_source_identity,
        document_ref,
        transient: candidate.transient != 0,
        format: format_from_db(&candidate.source_format)?,
        masked_hash: &candidate.document_masked_hash,
        raw_secret_id: SecretArtifactId::from_str(&candidate.raw_secret_id)?,
        manifest_secret_id: SecretArtifactId::from_str(&candidate.manifest_secret_id)?,
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
    document_ref: Option<DocumentRef>,
    transient: bool,
    format: SourceDocumentFormat,
    masked_hash: &'a str,
    raw_secret_id: SecretArtifactId,
    manifest_secret_id: SecretArtifactId,
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
        "document_id": input.document_ref.map(|value| value.document_id),
        "document_revision": input.document_ref.map(|value| value.document_revision),
        "transient": input.transient,
        "format": input.format,
        "masked_hash": input.masked_hash,
        "raw_secret_id": input.raw_secret_id,
        "manifest_secret_id": input.manifest_secret_id,
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
        "document_schema_version": SOURCE_DOCUMENT_SCHEMA_VERSION,
    });
    let bytes = serde_json::to_vec(&value).map_err(|_| StorageError::Serialization)?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

/// 读取 opaque candidate 的安全 preview。
pub(crate) fn get_candidate_summary(
    conn: &mut SqliteConnection,
    candidate_id: Uuid,
) -> Result<Option<CandidateSummary>, StorageError> {
    get_candidate_sync(conn, candidate_id)?
        .map(|candidate| {
            let document_ref = candidate_document_ref(&candidate)?;
            Ok(CandidateSummary {
                candidate_id,
                source_identity: candidate.target_source_identity,
                document_ref,
                transient: candidate.transient != 0,
                expected_installed_revision: from_i64(
                    candidate.expected_installed_revision,
                    "candidate expected revision",
                )?,
                profile: deserialize(candidate.profile_json.as_bytes())?,
                required_grant: deserialize(candidate.required_grant_json.as_bytes())?,
                diagnostics: deserialize(candidate.diagnostics_json.as_bytes())?,
                definition_hash: candidate.definition_hash,
                plan_hash: candidate.plan_hash,
                expires_at_ms: candidate.expires_at_ms,
            })
        })
        .transpose()
}

fn get_candidate_sync(
    conn: &mut SqliteConnection,
    candidate_id: Uuid,
) -> Result<Option<CandidateRow>, StorageError> {
    sql_query(
        "SELECT candidate_schema_version, document_schema_version, source_format, document_id, document_revision, transient, document_masked_hash, raw_secret_id, manifest_secret_id, runtime_credential_secret_id, target_source_identity, expected_installed_revision, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, profile_json, required_grant_json, diagnostics_json, expires_at_ms, status FROM candidate_projection WHERE candidate_id = ?",
    )
    .bind::<Text, _>(candidate_id.to_string())
    .get_result::<CandidateRow>(conn)
    .optional()
    .map_err(database_error)
}

fn candidate_document_ref(candidate: &CandidateRow) -> Result<Option<DocumentRef>, StorageError> {
    match (
        candidate.transient != 0,
        candidate.document_id.as_deref(),
        candidate.document_revision,
    ) {
        (true, None, None) => Ok(None),
        (false, Some(document_id), Some(document_revision)) => Ok(Some(DocumentRef {
            document_id: SourceDocumentId::from_str(document_id)?,
            document_revision: from_i64(document_revision, "document revision")?,
        })),
        _ => Err(StorageError::CandidateTampered),
    }
}

/// 读取 source current row，供 install/execution/checkpoint 使用。
pub(crate) fn get_source_row(
    conn: &mut SqliteConnection,
    source_identity: &str,
) -> Result<Option<SourceRow>, StorageError> {
    sql_query(
        "SELECT source_identity, version, profile_json, grant_json, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, cookie_namespace, runtime_credential_secret_id, document_id, document_revision, revision FROM source_projection WHERE source_identity = ?",
    )
    .bind::<Text, _>(source_identity)
    .get_result::<SourceRow>(conn)
    .optional()
    .map_err(database_error)
}

/// 从 current source projection 读取 immutable package/Plan。
pub(crate) fn get_installed_source_sync(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    source_identity: &str,
) -> Result<Option<InstalledSource>, StorageError> {
    let Some(row) = get_source_row(conn, source_identity)? else {
        return Ok(None);
    };
    let package = read_rule_package_artifact(&read_body_by_hash(
        conn,
        artifacts,
        &row.package_artifact_hash,
    )?)?;
    let plan = read_execution_plan_artifact(&read_body_by_hash(
        conn,
        artifacts,
        &row.plan_artifact_hash,
    )?)?;
    let document_id = row
        .document_id
        .as_deref()
        .map(SourceDocumentId::from_str)
        .transpose()?;
    let document_revision = row
        .document_revision
        .map(|value| from_i64(value, "document revision"))
        .transpose()?;
    Ok(Some(InstalledSource {
        source_identity: row.source_identity,
        version: row.version,
        package,
        plan,
        profile: deserialize(row.profile_json.as_bytes())?,
        grant: deserialize(row.grant_json.as_bytes())?,
        source_revision: from_i64(row.revision, "source revision")?,
        document_id,
        document_revision,
    }))
}

/// 按 source identity 读取不含 Plan/secret 的安全来源记录。
pub(crate) fn list_installed_sources_sync(
    conn: &mut SqliteConnection,
) -> Result<Vec<InstalledSourceRecord>, StorageError> {
    sql_query(
        "SELECT source_identity, version, profile_json, grant_json, document_id, document_revision, revision FROM source_projection ORDER BY source_identity ASC",
    )
    .load::<InstalledSourceRecordRow>(conn)
    .map_err(database_error)?
    .into_iter()
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
    let document_id = row
        .document_id
        .as_deref()
        .map(SourceDocumentId::from_str)
        .transpose()?;
    let document_revision = row
        .document_revision
        .map(|value| from_i64(value, "document revision"))
        .transpose()?;
    if document_id.is_some() != document_revision.is_some() {
        return Err(StorageError::InvalidInput(
            "source document 关联不完整".to_string(),
        ));
    }
    Ok(InstalledSourceRecord {
        source_identity: row.source_identity,
        version: row.version,
        profile,
        grant: deserialize::<PolicyCapabilities>(row.grant_json.as_bytes())?,
        source_revision: from_i64(row.revision, "source revision")?,
        has_editable_source: document_id.is_some(),
        document_id,
        document_revision,
    })
}

fn installed_source_from_revision(
    conn: &mut SqliteConnection,
    artifacts: &ArtifactStore,
    source_identity: &str,
    source_revision: u64,
) -> Result<InstalledSource, StorageError> {
    let row = sql_query(
        "SELECT source_identity, source_revision, version, profile_json, grant_json, package_artifact_hash, plan_artifact_hash, document_id, document_revision FROM source_versions WHERE source_identity = ? AND source_revision = ?",
    )
    .bind::<Text, _>(source_identity)
    .bind::<BigInt, _>(to_i64(source_revision)?)
    .get_result::<InstalledSourceVersionRow>(conn)
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::SourceMissing)?;
    let package = read_rule_package_artifact(&read_body_by_hash(
        conn,
        artifacts,
        &row.package_artifact_hash,
    )?)?;
    let plan = read_execution_plan_artifact(&read_body_by_hash(
        conn,
        artifacts,
        &row.plan_artifact_hash,
    )?)?;
    Ok(InstalledSource {
        source_identity: row.source_identity,
        version: row.version,
        package,
        plan,
        profile: deserialize(row.profile_json.as_bytes())?,
        grant: deserialize(row.grant_json.as_bytes())?,
        source_revision: from_i64(row.source_revision, "source revision")?,
        document_id: row
            .document_id
            .as_deref()
            .map(SourceDocumentId::from_str)
            .transpose()?,
        document_revision: row
            .document_revision
            .map(|value| from_i64(value, "document revision"))
            .transpose()?,
    })
}

/// 启动恢复时淘汰 expired/schema-invalid/非 staged candidate，并释放全部 ownership。
pub(crate) fn recover_candidates_sync(
    conn: &mut SqliteConnection,
    now_ms: i64,
) -> Result<usize, StorageError> {
    let rows = sql_query("SELECT candidate_id FROM candidate_projection ORDER BY created_at_ms")
        .load::<CandidateIdRow>(conn)
        .map_err(database_error)?;
    let legacy_rows = sql_query(
        "SELECT candidate_id FROM candidates WHERE status = 'schema_invalid' ORDER BY created_at_ms",
    )
    .load::<CandidateIdRow>(conn)
    .map_err(database_error)?;
    let mut removed = 0_usize;
    conn.immediate_transaction::<_, StorageError, _>(|conn| {
        for row in &legacy_rows {
            let candidate_id =
                Uuid::parse_str(&row.candidate_id).map_err(|_| StorageError::CandidateTampered)?;
            remove_candidate_event_refs(conn, candidate_id)?;
            sql_query("DELETE FROM candidates WHERE candidate_id = ?")
                .bind::<Text, _>(candidate_id.to_string())
                .execute(conn)
                .map_err(database_error)?;
        }
        Ok(())
    })?;
    removed = removed.saturating_add(legacy_rows.len());
    for row in rows {
        let candidate_id =
            Uuid::parse_str(&row.candidate_id).map_err(|_| StorageError::CandidateTampered)?;
        let candidate =
            get_candidate_sync(conn, candidate_id)?.ok_or(StorageError::CandidateMissing)?;
        let schema_invalid = candidate.candidate_schema_version
            != i32::try_from(INSTALL_CANDIDATE_SCHEMA_VERSION)
                .map_err(|_| StorageError::CandidateSchemaMismatch)?
            || candidate.document_schema_version
                != i32::try_from(SOURCE_DOCUMENT_SCHEMA_VERSION)
                    .map_err(|_| StorageError::CandidateSchemaMismatch)?;
        if schema_invalid || candidate.status != "staged" || candidate.expires_at_ms <= now_ms {
            expire_candidate(conn, candidate_id)?;
            removed = removed.saturating_add(1);
        }
    }
    Ok(removed)
}

/// 过期 candidate 的 body/secret owners 与 projection 一起释放。
pub(crate) fn expire_candidate(
    conn: &mut SqliteConnection,
    candidate_id: Uuid,
) -> Result<(), StorageError> {
    if get_candidate_sync(conn, candidate_id)?.is_none() {
        return Ok(());
    }
    conn.immediate_transaction::<_, StorageError, _>(|conn| {
        release_candidate_secret_owners(conn, candidate_id)?;
        remove_candidate_event_refs(conn, candidate_id)?;
        sql_query("DELETE FROM candidate_projection WHERE candidate_id = ?")
            .bind::<Text, _>(candidate_id.to_string())
            .execute(conn)
            .map_err(database_error)?;
        Ok(())
    })
}

fn release_candidate_secret_owners(
    conn: &mut SqliteConnection,
    candidate_id: Uuid,
) -> Result<(), StorageError> {
    let owner_id = candidate_id.to_string();
    release_secret_owner(conn, "candidate_raw", &owner_id)?;
    release_secret_owner(conn, "candidate_manifest", &owner_id)?;
    release_secret_owner(conn, "candidate_runtime", &owner_id)
}

pub(crate) fn grant_covers(grant: &PolicyCapabilities, required: &PolicyCapabilities) -> bool {
    (!required.network || grant.network)
        && (!required.system.fs || grant.system.fs)
        && (!required.system.env || grant.system.env)
        && (!required.system.process || grant.system.process)
}

/// 读取 versioned RulePackage artifact，并保留未知版本与损坏 JSON 的区别。
///
/// # Errors
///
/// 未知 schema 返回 [`StorageError::ContractSchemaIncompatible`]；其余 JSON/shape 错误返回
/// [`StorageError::Serialization`]。
pub(crate) fn read_rule_package_artifact(bytes: &[u8]) -> Result<RulePackage, StorageError> {
    read_rule_package(bytes).map_err(schema_read_error)
}

/// 读取 versioned ExecutionPlan artifact，并保留 v1 legacy hash material。
///
/// # Errors
///
/// 未知 schema 返回 [`StorageError::ContractSchemaIncompatible`]；其余 JSON/shape 错误返回
/// [`StorageError::Serialization`]。
pub(crate) fn read_execution_plan_artifact(bytes: &[u8]) -> Result<ExecutionPlan, StorageError> {
    read_execution_plan(bytes).map_err(schema_read_error)
}

fn schema_read_error(error: SchemaReadError) -> StorageError {
    match error {
        SchemaReadError::IncompatibleVersion { contract, version } => {
            StorageError::ContractSchemaIncompatible { contract, version }
        }
        SchemaReadError::Malformed(_)
        | SchemaReadError::ContractMismatch { .. }
        | SchemaReadError::InvalidData { .. } => StorageError::Serialization,
    }
}

fn candidate_contract_artifact_error(error: StorageError) -> StorageError {
    match error {
        StorageError::ContractSchemaIncompatible { .. } => error,
        _ => StorageError::CandidateTampered,
    }
}

pub(crate) fn canonical_plan_hash(plan: &ExecutionPlan) -> Result<String, StorageError> {
    execution_plan_hash(plan).map_err(|error| StorageError::InvalidInput(error.to_string()))
}

pub(crate) fn validate_candidate_package_and_plan(
    package: &RulePackage,
    plan: &ExecutionPlan,
) -> Result<(), StorageError> {
    let expected_definition_hash = definition_hash(package.definition())
        .map_err(|error| StorageError::InvalidInput(error.to_string()))?;
    if expected_definition_hash != plan.definition_hash()
        || plan.plan_hash() != canonical_plan_hash(plan)?
        || package.source_identity() != package.definition().source_identity()
    {
        return Err(StorageError::InvalidInput(
            "candidate package/Plan hash 不一致".to_string(),
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

pub(crate) fn source_version_owner_id(source_identity: &str, source_revision: u64) -> String {
    format!("{source_identity}:{source_revision}")
}

fn source_document_candidate_row(
    conn: &mut SqliteConnection,
    document_id: SourceDocumentId,
) -> Result<Option<SourceDocumentCandidateRow>, StorageError> {
    sql_query(
        "SELECT format, source_identity, current_document_revision, masked_secret_id, raw_secret_id, manifest_secret_id, masked_hash, schema_version FROM source_document_projection WHERE document_id = ?",
    )
    .bind::<Text, _>(document_id.to_string())
    .get_result::<SourceDocumentCandidateRow>(conn)
    .optional()
    .map_err(database_error)
}

#[derive(QueryableByName)]
struct CandidateIdRow {
    #[diesel(sql_type = Text)]
    candidate_id: String,
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
    #[diesel(sql_type = Integer)]
    candidate_schema_version: i32,
    #[diesel(sql_type = Integer)]
    document_schema_version: i32,
    #[diesel(sql_type = Text)]
    source_format: String,
    #[diesel(sql_type = Nullable<Text>)]
    document_id: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    document_revision: Option<i64>,
    #[diesel(sql_type = Integer)]
    transient: i32,
    #[diesel(sql_type = Text)]
    document_masked_hash: String,
    #[diesel(sql_type = Text)]
    raw_secret_id: String,
    #[diesel(sql_type = Text)]
    manifest_secret_id: String,
    #[diesel(sql_type = Nullable<Text>)]
    runtime_credential_secret_id: Option<String>,
    #[diesel(sql_type = Text)]
    target_source_identity: String,
    #[diesel(sql_type = BigInt)]
    expected_installed_revision: i64,
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
}

#[derive(QueryableByName)]
struct SourceDocumentCandidateRow {
    #[diesel(sql_type = Text)]
    format: String,
    #[diesel(sql_type = Nullable<Text>)]
    source_identity: Option<String>,
    #[diesel(sql_type = BigInt)]
    current_document_revision: i64,
    #[diesel(sql_type = Text)]
    masked_secret_id: String,
    #[diesel(sql_type = Text)]
    raw_secret_id: String,
    #[diesel(sql_type = Text)]
    manifest_secret_id: String,
    #[diesel(sql_type = Text)]
    masked_hash: String,
    #[diesel(sql_type = BigInt)]
    schema_version: i64,
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
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) document_id: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub(crate) document_revision: Option<i64>,
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
    #[diesel(sql_type = Nullable<Text>)]
    document_id: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    document_revision: Option<i64>,
    #[diesel(sql_type = BigInt)]
    revision: i64,
}

#[derive(QueryableByName)]
struct InstalledSourceVersionRow {
    #[diesel(sql_type = Text)]
    source_identity: String,
    #[diesel(sql_type = BigInt)]
    source_revision: i64,
    #[diesel(sql_type = Text)]
    version: String,
    #[diesel(sql_type = Text)]
    profile_json: String,
    #[diesel(sql_type = Text)]
    grant_json: String,
    #[diesel(sql_type = Text)]
    package_artifact_hash: String,
    #[diesel(sql_type = Text)]
    plan_artifact_hash: String,
    #[diesel(sql_type = Nullable<Text>)]
    document_id: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    document_revision: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_v1_plan_reader_preserves_legacy_hash_material() {
        let mut value = serde_json::json!({
            "contract": "execution_plan",
            "schema_version": ContractSchemaVersion::V1.as_u32(),
            "compiler_version": "legacy-storage-test@1",
            "definition_hash": "legacy-definition-hash",
            "plan_hash": "",
            "nodes": [],
            "edges": [],
            "intent_entries": {},
            "effects": [],
            "capability_requirements": []
        });
        let unsealed = read_execution_plan_artifact(
            &serde_json::to_vec(&value).expect("serialize unsealed v1 Plan"),
        )
        .expect("read unsealed v1 Plan");
        value["plan_hash"] = serde_json::Value::String(
            canonical_plan_hash(&unsealed).expect("hash exact v1 Plan material"),
        );
        let plan = read_execution_plan_artifact(
            &serde_json::to_vec(&value).expect("serialize sealed v1 Plan"),
        )
        .expect("read sealed v1 Plan");

        assert_eq!(plan.schema_version(), ContractSchemaVersion::V1);
        assert_eq!(plan.plan_hash(), canonical_plan_hash(&plan).unwrap());
    }

    #[test]
    fn unknown_package_and_plan_versions_are_typed_incompatible() {
        let unknown_version = u32::MAX;
        let package_error = read_rule_package_artifact(
            &serde_json::to_vec(&serde_json::json!({
                "contract": "rule_package",
                "schema_version": unknown_version
            }))
            .expect("serialize unknown package"),
        )
        .expect_err("unknown package version must fail");
        assert!(matches!(
            package_error,
            StorageError::ContractSchemaIncompatible {
                contract: lj_rule_model::SchemaContract::RulePackage,
                version,
            } if version == unknown_version
        ));

        let plan_error = read_execution_plan_artifact(
            &serde_json::to_vec(&serde_json::json!({
                "contract": "execution_plan",
                "schema_version": unknown_version
            }))
            .expect("serialize unknown Plan"),
        )
        .expect_err("unknown Plan version must fail");
        assert!(matches!(
            plan_error,
            StorageError::ContractSchemaIncompatible {
                contract: lj_rule_model::SchemaContract::ExecutionPlan,
                version,
            } if version == unknown_version
        ));
    }
}
