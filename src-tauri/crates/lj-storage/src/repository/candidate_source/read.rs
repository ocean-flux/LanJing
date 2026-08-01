/// 读取 opaque candidate 的安全 preview。
pub(crate) async fn get_candidate_summary(
    conn: &mut DatabaseSession,
    candidate_id: Uuid,
) -> Result<Option<CandidateSummary>, StorageError> {
    get_candidate_sync(conn, candidate_id).await?
        .map(|candidate| {
            Ok(CandidateSummary {
                candidate_id,
                source_identity: candidate.target_source_identity,
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

async fn get_candidate_sync(
    conn: &mut DatabaseSession,
    candidate_id: Uuid,
) -> Result<Option<CandidateRow>, StorageError> {
    statement(
        "SELECT candidate_schema_version, runtime_credential_secret_id, target_source_identity, expected_installed_revision, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, profile_json, required_grant_json, diagnostics_json, expires_at_ms, status FROM candidate_projection WHERE candidate_id = ?",
    )
    .bind(candidate_id.to_string())
    .get_result::<CandidateRow>(conn).await
    .optional()
    .map_err(database_error)
}

/// 读取 source current row，供 install/execution/checkpoint 使用。
pub(crate) async fn get_source_row(
    conn: &mut DatabaseSession,
    source_identity: &str,
) -> Result<Option<SourceRow>, StorageError> {
    statement(
        "SELECT source_identity, version, profile_json, grant_json, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, cookie_namespace, runtime_credential_secret_id, revision FROM source_projection WHERE source_identity = ?",
    )
    .bind(source_identity)
    .get_result::<SourceRow>(conn).await
    .optional()
    .map_err(database_error)
}

/// 从 current source projection 读取 immutable package/Plan。
pub(crate) async fn get_installed_source_sync(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    source_identity: &str,
) -> Result<Option<InstalledSource>, StorageError> {
    let Some(row) = get_source_row(conn, source_identity).await? else {
        return Ok(None);
    };
    let package = read_rule_package_artifact(&read_body_by_hash(
        conn,
        artifacts,
        &row.package_artifact_hash,
    ).await?)?;
    let plan = read_execution_plan_artifact(&read_body_by_hash(
        conn,
        artifacts,
        &row.plan_artifact_hash,
    ).await?)?;
    Ok(Some(InstalledSource {
        source_identity: row.source_identity,
        version: row.version,
        package,
        plan,
        profile: deserialize(row.profile_json.as_bytes())?,
        grant: deserialize(row.grant_json.as_bytes())?,
        source_revision: from_i64(row.revision, "source revision")?,
    }))
}

/// 按 source identity 读取不含 Plan/secret 的安全来源记录。
pub(crate) async fn list_installed_sources_sync(
    conn: &mut DatabaseSession,
) -> Result<Vec<InstalledSourceRecord>, StorageError> {
    statement(
        "SELECT source_identity, version, profile_json, grant_json, revision FROM source_projection ORDER BY source_identity ASC",
    )
    .load::<InstalledSourceRecordRow>(conn).await
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
    Ok(InstalledSourceRecord {
        source_identity: row.source_identity,
        version: row.version,
        profile,
        grant: deserialize::<PolicyCapabilities>(row.grant_json.as_bytes())?,
        source_revision: from_i64(row.revision, "source revision")?,
    })
}

async fn installed_source_from_revision(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    source_identity: &str,
    source_revision: u64,
) -> Result<InstalledSource, StorageError> {
    let row = statement(
        "SELECT source_identity, source_revision, version, profile_json, grant_json, package_artifact_hash, plan_artifact_hash FROM source_versions WHERE source_identity = ? AND source_revision = ?",
    )
    .bind(source_identity)
    .bind(to_i64(source_revision)?)
    .get_result::<InstalledSourceVersionRow>(conn).await
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::SourceMissing)?;
    let package = read_rule_package_artifact(&read_body_by_hash(
        conn,
        artifacts,
        &row.package_artifact_hash,
    ).await?)?;
    let plan = read_execution_plan_artifact(&read_body_by_hash(
        conn,
        artifacts,
        &row.plan_artifact_hash,
    ).await?)?;
    Ok(InstalledSource {
        source_identity: row.source_identity,
        version: row.version,
        package,
        plan,
        profile: deserialize(row.profile_json.as_bytes())?,
        grant: deserialize(row.grant_json.as_bytes())?,
        source_revision: from_i64(row.source_revision, "source revision")?,
    })
}

/// 启动恢复时淘汰 expired/schema-invalid/非 staged candidate，并释放全部 ownership。
pub(crate) async fn recover_candidates_sync(
    conn: &mut DatabaseSession,
    now_ms: i64,
) -> Result<usize, StorageError> {
    let rows = statement("SELECT candidate_id FROM candidate_projection ORDER BY created_at_ms")
        .load::<CandidateIdRow>(conn).await
        .map_err(database_error)?;
    let mut removed = 0_usize;
    for row in rows {
        let candidate_id =
            Uuid::parse_str(&row.candidate_id).map_err(|_| StorageError::CandidateTampered)?;
        let candidate =
            get_candidate_sync(conn, candidate_id).await?.ok_or(StorageError::CandidateMissing)?;
        let schema_invalid = candidate.candidate_schema_version
            != i32::try_from(INSTALL_CANDIDATE_SCHEMA_VERSION)
                .map_err(|_| StorageError::CandidateSchemaMismatch)?;
        if schema_invalid || candidate.status != "staged" || candidate.expires_at_ms <= now_ms {
            expire_candidate(conn, candidate_id).await?;
            removed = removed.saturating_add(1);
        }
    }
    Ok(removed)
}

/// 过期 candidate 的 body/secret owners 与 projection 一起释放。
pub(crate) async fn expire_candidate(
    conn: &mut DatabaseSession,
    candidate_id: Uuid,
) -> Result<(), StorageError> {
    if get_candidate_sync(conn, candidate_id).await?.is_none() {
        return Ok(());
    }
    release_candidate_secret_owners(conn, candidate_id).await?;
    remove_candidate_event_refs(conn, candidate_id).await?;
    statement("DELETE FROM candidate_projection WHERE candidate_id = ?")
        .bind(candidate_id.to_string())
        .execute(conn)
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn release_candidate_secret_owners(
    conn: &mut DatabaseSession,
    candidate_id: Uuid,
) -> Result<(), StorageError> {
    let owner_id = candidate_id.to_string();
    release_secret_owner(conn, "candidate_runtime", &owner_id).await
}
