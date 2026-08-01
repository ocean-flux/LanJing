/// 更新 execution summary revision；只可从同一个 Event transaction closure 调用。
pub(crate) async fn update_execution_revision(
    conn: &mut DatabaseSession,
    execution_id: Uuid,
    revision: u64,
    global_seq: u64,
) -> Result<(), StorageError> {
    let changed = statement(
        "UPDATE execution_projection SET revision = ?, updated_global_seq = ? WHERE execution_id = ?",
    )
    .bind(to_i64(revision)?)
    .bind(to_i64(global_seq)?)
    .bind(execution_id.to_string())
    .execute(conn).await
    .map_err(database_error)?;
    if changed == 1 {
        Ok(())
    } else {
        Err(StorageError::ExecutionMissing)
    }
}

/// 读取 execution summary；未知状态或损坏的 current revision 时失败。
pub(crate) async fn get_execution_sync(
    conn: &mut DatabaseSession,
    execution_id: Uuid,
) -> Result<Option<ExecutionRecord>, StorageError> {
    let row = statement(
        "SELECT execution_id, source_identity, source_revision, plan_hash, status, pinned, archive_available, gc_state, started_at_ms, finished_at_ms, revision FROM execution_projection WHERE execution_id = ?",
    )
    .bind(execution_id.to_string())
    .get_result::<ExecutionRow>(conn).await
    .optional()
    .map_err(database_error)?;
    row.map(execution_from_row).transpose()
}

/// 从 `SQLite` row 转换公开 execution summary。
pub(crate) fn execution_from_row(row: ExecutionRow) -> Result<ExecutionRecord, StorageError> {
    Ok(ExecutionRecord {
        execution_id: Uuid::parse_str(&row.execution_id)
            .map_err(|_| StorageError::InvalidInput("损坏的 execution ID".to_string()))?,
        source_identity: row.source_identity,
        source_revision: from_i64(row.source_revision, "source revision")?,
        plan_hash: row.plan_hash,
        status: ExecutionStatus::from_db(&row.status)?,
        pinned: row.pinned != 0,
        replayable: row.archive_available != 0,
        gc_state: GcState::from_db(&row.gc_state)?,
        started_at_ms: row.started_at_ms,
        finished_at_ms: row.finished_at_ms,
        revision: from_i64(row.revision, "execution revision")?,
    })
}

/// 加载并完整验证 historical revision pin；不能以 current source 替代。
pub(crate) async fn load_execution_replay_pin_sync(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    execution_id: Uuid,
) -> Result<ExecutionReplayPin, StorageError> {
    let row = statement(
        "SELECT execution_id, source_identity, source_version, source_revision, plan_hash, plan_artifact_hash, archive_available, gc_state FROM execution_projection WHERE execution_id = ?",
    )
    .bind(execution_id.to_string())
    .get_result::<ExecutionReplayPinRow>(conn).await
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::ExecutionMissing)?;
    if row.archive_available == 0 || row.gc_state != GcState::Active.as_db() {
        return Err(StorageError::ReplayUnavailable(
            "execution archive 已被 GC 或不可 replay".to_string(),
        ));
    }
    let source_revision = from_i64(row.source_revision, "source revision")?;
    let loaded = load_source_version(conn, artifacts, &row.source_identity, source_revision)
        .await
        .map_err(map_replay_snapshot_error)?;
    if loaded.snapshot.version != row.source_version
        || loaded.plan_hash != row.plan_hash
        || loaded.plan_artifact_hash != row.plan_artifact_hash
    {
        return Err(StorageError::ReplayUnavailable(
            "execution pin 与 immutable source snapshot 不一致".to_string(),
        ));
    }
    let execution_id = Uuid::parse_str(&row.execution_id)
        .map_err(|_| StorageError::InvalidInput("损坏的 execution ID".to_string()))?;
    Ok(ExecutionReplayPin {
        execution_id,
        source_identity: row.source_identity,
        source_revision,
        source_version: loaded.snapshot.version,
        profile: loaded.snapshot.profile,
        grant: loaded.snapshot.grant,
        base_url: loaded.snapshot.base_url,
        package_artifact_hash: loaded.package_artifact_hash,
        plan: loaded.snapshot.plan,
        plan_hash: loaded.plan_hash,
        plan_artifact_hash: loaded.plan_artifact_hash,
        mode: ExecutionMode::Replay {
            archived_execution_id: execution_id,
        },
    })
}

fn map_replay_snapshot_error(error: StorageError) -> StorageError {
    match error {
        StorageError::KeyringLocked
        | StorageError::KeyringUnavailable
        | StorageError::KeyLost
        | StorageError::ContractSchemaUnsupported { .. } => error,
        _ => StorageError::ReplayUnavailable(
            "execution pin source snapshot 缺失、损坏或不一致".to_string(),
        ),
    }
}

async fn load_source_version(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    source_identity: &str,
    source_revision: u64,
) -> Result<LoadedSourceVersion, StorageError> {
    let row = statement(
        "SELECT source_identity, source_revision, version, profile_json, grant_json, base_url, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, cookie_namespace, runtime_credential_secret_id FROM source_versions WHERE source_identity = ? AND source_revision = ?",
    )
    .bind(source_identity)
    .bind(to_i64(source_revision)?)
    .get_result::<SourceVersionRow>(conn).await
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::SourceMissing)?;
    if row.source_identity != source_identity
        || from_i64(row.source_revision, "source revision")? != source_revision
        || row.cookie_namespace.is_empty()
    {
        return Err(StorageError::ArtifactCorrupt);
    }
    ensure_blake3_hash(&row.package_artifact_hash, "package artifact hash")?;
    ensure_blake3_hash(&row.plan_artifact_hash, "Plan artifact hash")?;
    ensure_blake3_hash(&row.definition_hash, "definition hash")?;
    ensure_blake3_hash(&row.plan_hash, "Plan hash")?;
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
    let profile = deserialize::<SourceProfile>(row.profile_json.as_bytes())?;
    let grant = deserialize::<PolicyCapabilities>(row.grant_json.as_bytes())?;
    if package.source_identity().id != source_identity
        || package.version() != row.version
        || package.definition().base_url() != row.base_url
        || !grant_covers(&grant, &package.definition().capability_manifest().required)
        || profile.id.0 != source_identity
        || profile.version.as_deref() != Some(row.version.as_str())
        || plan.definition_hash() != row.definition_hash
        || plan.plan_hash() != row.plan_hash
        || canonical_plan_hash(&plan)? != row.plan_hash
    {
        return Err(StorageError::ArtifactCorrupt);
    }
    validate_candidate_package_and_plan(&package, &plan)
        .map_err(|_| StorageError::ArtifactCorrupt)?;
    let secret_bytes = match row.runtime_credential_secret_id.as_deref() {
        Some(secret_id) => Some(
            read_owned_secret(
                conn,
                artifacts,
                SecretArtifactId::from_str(secret_id)?,
                "source_version_runtime",
                &source_version_owner_id(source_identity, source_revision),
            )
            .await?,
        ),
        None => None,
    };
    Ok(LoadedSourceVersion {
        snapshot: InstalledSourceSnapshot {
            source_identity: source_identity.to_string(),
            source_revision,
            version: row.version,
            profile,
            grant,
            base_url: row.base_url,
            package,
            plan,
            runtime_credentials: ExecutionSourceCredentials::new(
                row.cookie_namespace,
                secret_bytes,
            ),
        },
        package_artifact_hash: row.package_artifact_hash,
        plan_artifact_hash: row.plan_artifact_hash,
        definition_hash: row.definition_hash,
        plan_hash: row.plan_hash,
    })
}

async fn execution_is_replay_sync(
    conn: &mut DatabaseSession,
    execution_id: Uuid,
) -> Result<bool, StorageError> {
    let payload =
        statement("SELECT payload_json FROM events WHERE stream_id = ? AND stream_version = 1")
            .bind(execution_stream_id(execution_id))
            .get_result::<JsonRow>(conn).await
            .optional()
            .map_err(database_error)?;
    let Some(payload) = payload else {
        return Err(StorageError::ExecutionMissing);
    };
    let payload: serde_json::Value = deserialize(payload.payload_json.as_bytes())?;
    Ok(payload["kind"].as_str() == Some("replay_started"))
}

/// 仅为 live execution 解密固定 source-revision credential；replay 显式失败。
pub(crate) async fn load_execution_source_credentials_sync(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    execution_id: Uuid,
) -> Result<ExecutionSourceCredentials, StorageError> {
    let execution = statement(
        "SELECT source_identity, source_revision FROM execution_projection WHERE execution_id = ?",
    )
    .bind(execution_id.to_string())
    .get_result::<ExecutionSourceRevisionRow>(conn).await
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::ExecutionMissing)?;
    if execution_is_replay_sync(conn, execution_id).await? {
        return Err(StorageError::ReplayUnavailable(
            "replay execution 不传递 source credential".to_string(),
        ));
    }
    let source_revision = from_i64(execution.source_revision, "source revision")?;
    let row = statement(
        "SELECT cookie_namespace, runtime_credential_secret_id FROM source_versions WHERE source_identity = ? AND source_revision = ?",
    )
    .bind(&execution.source_identity)
    .bind(to_i64(source_revision)?)
    .get_result::<SourceVersionCredentialRow>(conn).await
    .optional()
    .map_err(database_error)?
    .ok_or(StorageError::SourceCredentialUnavailable)?;
    let cookie_namespace = if row.cookie_namespace.is_empty() {
        source_cookie_namespace(&execution.source_identity)
    } else {
        row.cookie_namespace
    };
    let secret_bytes = match row.runtime_credential_secret_id.as_deref() {
        Some(secret_id) => Some(
            read_owned_secret(
                conn,
                artifacts,
                SecretArtifactId::from_str(secret_id)?,
                "source_version_runtime",
                &source_version_owner_id(&execution.source_identity, source_revision),
            )
            .await?,
        ),
        None => None,
    };
    Ok(ExecutionSourceCredentials::new(
        cookie_namespace,
        secret_bytes,
    ))
}
