//! Historical Plan pin、source version、credential 与 snapshot replay 合同。

use super::*;

#[tokio::test]
async fn execution_replay_pin_uses_verified_artifact_and_rejects_tampering() {
    let temp = TempStore::new("replay-pin");
    let storage = temp.open().await;
    let now = 1_750_000_800_000;
    install_source(&storage, now).await;
    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-replay-pin-start".to_string(),
            started_at_ms: now + 1,
            correlation_id: None,
        })
        .await
        .expect("start execution that pins current Plan");

    let pin = storage
        .load_execution_replay_pin(execution_id)
        .await
        .expect("load verified execution replay pin");
    assert_eq!(pin.execution_id, execution_id);
    assert_eq!(pin.source_identity, "source:test");
    assert_eq!(pin.source_version, "v1");
    assert_eq!(pin.plan.plan_hash, pin.plan_hash);
    assert_eq!(
        pin.mode,
        ExecutionMode::Replay {
            archived_execution_id: execution_id
        }
    );

    let artifact_path = temp
        .root
        .join("artifacts")
        .join("body")
        .join(&pin.plan_artifact_hash[..2])
        .join(&pin.plan_artifact_hash[2..4])
        .join(format!("{}.zst", pin.plan_artifact_hash));
    let tampered = zstd::stream::encode_all(std::io::Cursor::new(b"tampered"), 3)
        .expect("compress tampered Plan body");
    fs::write(&artifact_path, tampered).expect("replace Plan artifact with tampered body");
    let tampered_result = storage.load_execution_replay_pin(execution_id).await;
    assert!(matches!(
        &tampered_result,
        Err(StorageError::ArtifactUnavailable(hash)) if hash == &pin.plan_artifact_hash
    ));

    fs::remove_file(&artifact_path).expect("remove pinned Plan artifact");
    let missing_result = storage.load_execution_replay_pin(execution_id).await;
    assert!(matches!(
        &missing_result,
        Err(StorageError::ArtifactUnavailable(hash)) if hash == &pin.plan_artifact_hash
    ));
    storage.shutdown().await.expect("writer shutdown");
}

#[tokio::test]
async fn replay_start_keeps_historical_source_snapshot_after_source_update() {
    let temp = TempStore::new("replay-source-version");
    let storage = temp.open().await;
    let now = 1_750_000_900_000;
    let first = candidate(now);
    let first_profile = first.profile.clone();
    let first_grant = first.required_grant.clone();
    let first_base_url = first.package.definition.base_url.clone();
    let first_plan_hash = first.plan.plan_hash.clone();
    install_draft(&storage, first, 0, first_grant.clone(), now + 1).await;

    let archived_execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id: archived_execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-original-live".to_string(),
            started_at_ms: now + 2,
            correlation_id: None,
        })
        .await
        .expect("start original execution");
    let original_pin = storage
        .load_execution_replay_pin(archived_execution_id)
        .await
        .expect("load original replay pin");

    let mut forged_pin = original_pin.clone();
    forged_pin.base_url = "https://forged.example.test".to_string();
    assert!(matches!(
        storage
            .start_replay_execution(ReplayExecutionStart {
                execution_id: Uuid::new_v4(),
                pin: forged_pin,
                event_id: Uuid::new_v4(),
                trace_id: "trace-forged-replay".to_string(),
                started_at_ms: now + 3,
                correlation_id: None,
            })
            .await,
        Err(StorageError::ReplayUnavailable(_))
    ));

    let updated = updated_candidate(now + 4);
    let updated_grant = updated.required_grant.clone();
    install_draft(&storage, updated, 1, updated_grant, now + 5).await;
    let current = storage
        .get_installed_source("source:test")
        .await
        .expect("read updated source")
        .expect("updated source exists");
    assert_eq!(current.version, "v2");
    assert_eq!(
        current.package.definition.base_url,
        "https://updated.example.test"
    );

    let historical_pin = storage
        .load_execution_replay_pin(archived_execution_id)
        .await
        .expect("old execution still loads its source-version pin");
    assert_eq!(historical_pin.source_version, "v1");
    assert_eq!(historical_pin.profile, first_profile);
    assert_eq!(historical_pin.grant, first_grant);
    assert_eq!(historical_pin.base_url, first_base_url);
    assert_eq!(historical_pin.plan_hash, first_plan_hash);

    let replay_execution_id = Uuid::new_v4();
    let replay_record = storage
        .start_replay_execution(ReplayExecutionStart {
            execution_id: replay_execution_id,
            pin: historical_pin.clone(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-historical-replay".to_string(),
            started_at_ms: now + 6,
            correlation_id: None,
        })
        .await
        .expect("start replay from historical pin");
    assert_eq!(replay_record.plan_hash, historical_pin.plan_hash);

    let replay_pin = storage
        .load_execution_replay_pin(replay_execution_id)
        .await
        .expect("new replay archive retains historical pin");
    assert_eq!(replay_pin.source_version, "v1");
    assert_eq!(replay_pin.profile, first_profile);
    assert_eq!(replay_pin.grant, first_grant);
    assert_eq!(replay_pin.base_url, first_base_url);
    assert_eq!(replay_pin.plan_hash, first_plan_hash);
    assert_eq!(
        replay_pin.mode,
        ExecutionMode::Replay {
            archived_execution_id: replay_execution_id
        }
    );
    storage.shutdown().await.expect("writer shutdown");
}

#[tokio::test]
async fn execution_source_credentials_follow_pinned_source_version_for_replay_and_key_loss() {
    init_mock_keyring();
    let temp = TempStore::new("execution-source-credential-version-pin");
    let storage = temp.open().await;
    let now = 1_750_001_100_000;

    let first_secret_hash = install_draft_with_source_credentials(
        &storage,
        candidate(now),
        0,
        b"source-secret-v1".to_vec(),
        now + 2,
    )
    .await;

    let original_execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id: original_execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-original-source-secret".to_string(),
            started_at_ms: now + 3,
            correlation_id: None,
        })
        .await
        .expect("start v1 execution");
    let original_credentials = storage
        .load_execution_source_credentials(original_execution_id)
        .await
        .expect("read execution-pinned v1 secret");
    assert_eq!(
        original_credentials.cookie_namespace(),
        "source/source:test"
    );
    assert!(
        original_credentials.into_secret_bytes().as_deref() == Some(b"source-secret-v1".as_slice())
    );
    let original_pin = storage
        .load_execution_replay_pin(original_execution_id)
        .await
        .expect("load v1 replay pin");

    install_draft_with_source_credentials(
        &storage,
        updated_candidate(now + 4),
        1,
        b"source-secret-v2".to_vec(),
        now + 6,
    )
    .await;

    let current_execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id: current_execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-current-source-secret".to_string(),
            started_at_ms: now + 7,
            correlation_id: None,
        })
        .await
        .expect("start v2 execution");
    assert!(
        storage
            .load_execution_source_credentials(current_execution_id)
            .await
            .expect("read execution-pinned v2 secret")
            .into_secret_bytes()
            .as_deref()
            == Some(b"source-secret-v2".as_slice())
    );

    let replay_execution_id = Uuid::new_v4();
    storage
        .start_replay_execution(ReplayExecutionStart {
            execution_id: replay_execution_id,
            pin: original_pin.clone(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-replay-source-secret-v1".to_string(),
            started_at_ms: now + 8,
            correlation_id: None,
        })
        .await
        .expect("start replay pinned to v1");
    assert!(matches!(
        storage
            .load_execution_source_credentials(replay_execution_id)
            .await,
        Err(StorageError::ReplayUnavailable(_))
    ));

    storage
        .shutdown()
        .await
        .expect("close before key-loss read");
    let restarted = temp.open().await;
    assert!(matches!(
        restarted
            .load_execution_source_credentials(original_execution_id)
            .await,
        Err(StorageError::MasterKeyUnavailable)
    ));
    assert!(matches!(
        restarted
            .load_execution_replay_pin(original_execution_id)
            .await,
        Err(StorageError::ReplayUnavailable(_))
    ));
    assert!(matches!(
        restarted
            .start_replay_execution(ReplayExecutionStart {
                execution_id: Uuid::new_v4(),
                pin: original_pin,
                event_id: Uuid::new_v4(),
                trace_id: "trace-key-loss-source-secret-replay".to_string(),
                started_at_ms: now + 9,
                correlation_id: None,
            })
            .await,
        Err(StorageError::ReplayUnavailable(_))
    ));
    restarted.shutdown().await.expect("close key-loss storage");
    let secret_path = temp
        .config
        .artifact_root
        .join("secret")
        .join(&first_secret_hash[..2])
        .join(&first_secret_hash[2..4])
        .join(format!("{first_secret_hash}.secret"));
    fs::remove_file(secret_path).expect("remove pinned source secret artifact");
    let missing_secret = temp.open().await;
    assert!(matches!(
        missing_secret
            .load_execution_replay_pin(original_execution_id)
            .await,
        Err(StorageError::ReplayUnavailable(_))
    ));
    missing_secret
        .shutdown()
        .await
        .expect("close missing-secret storage");
}

#[tokio::test]
async fn replay_pin_backfills_current_snapshot_and_rejects_missing_or_tampered_snapshot() {
    let temp = TempStore::new("replay-snapshot-backfill");
    let storage = temp.open().await;
    let now = 1_750_001_000_000;
    install_source(&storage, now).await;
    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-backfill-source".to_string(),
            started_at_ms: now + 1,
            correlation_id: None,
        })
        .await
        .expect("start execution before simulated migration gap");
    let fresh_pin = storage
        .load_execution_replay_pin(execution_id)
        .await
        .expect("fresh source version has a replay snapshot");
    let package_path = temp
        .root
        .join("artifacts")
        .join("body")
        .join(&fresh_pin.package_artifact_hash[..2])
        .join(&fresh_pin.package_artifact_hash[2..4])
        .join(format!("{}.zst", fresh_pin.package_artifact_hash));
    assert!(
        package_path.exists(),
        "source package artifact exists before restart"
    );
    storage
        .shutdown()
        .await
        .expect("close writer before SQL setup");

    let database_url = temp.config.database_path.to_string_lossy().into_owned();
    let mut conn = SqliteConnection::establish(&database_url).expect("open real SQLite database");
    let metadata = sql_query(
        "SELECT relative_path, ref_count FROM artifact_metadata WHERE hash = ? AND artifact_kind = 'body'",
    )
    .bind::<diesel::sql_types::Text, _>(&fresh_pin.package_artifact_hash)
    .get_result::<ArtifactMetadataTestRow>(&mut conn)
    .expect("source package metadata retained before restart");
    assert_eq!(
        metadata.relative_path,
        package_path
            .strip_prefix(temp.root.join("artifacts"))
            .expect("artifact is below root")
            .to_string_lossy()
            .replace('\\', "/")
    );
    assert!(metadata.ref_count >= 1, "source package has a durable ref");
    sql_query(
        "UPDATE source_versions SET profile_json = NULL, grant_json = NULL, base_url = NULL WHERE source_identity = 'source:test' AND version = 'v1'",
    )
    .execute(&mut conn)
    .expect("simulate legacy source version without replay snapshot");
    drop(conn);

    let storage = temp.open().await;
    assert!(
        package_path.exists(),
        "source package artifact survives restart recovery"
    );
    let pin = storage
        .load_execution_replay_pin(execution_id)
        .await
        .expect("startup backfill restores current source snapshot");
    assert_eq!(pin.profile.title, "测试来源");
    assert_eq!(pin.grant, PolicyCapabilities::default());
    assert_eq!(pin.base_url, "https://example.test");

    let mut conn = SqliteConnection::establish(&database_url).expect("open SQLite for tamper test");
    sql_query(
        "UPDATE source_versions SET base_url = 'https://tampered.example.test' WHERE source_identity = 'source:test' AND version = 'v1'",
    )
    .execute(&mut conn)
    .expect("tamper source snapshot base URL");
    drop(conn);
    let tampered = storage.load_execution_replay_pin(execution_id).await;
    assert!(matches!(&tampered, Err(StorageError::ReplayUnavailable(_))));

    let mut conn =
        SqliteConnection::establish(&database_url).expect("open SQLite for missing test");
    sql_query(
        "UPDATE source_versions SET base_url = 'https://example.test', profile_json = NULL WHERE source_identity = 'source:test' AND version = 'v1'",
    )
    .execute(&mut conn)
    .expect("remove source profile snapshot");
    drop(conn);
    let missing = storage.load_execution_replay_pin(execution_id).await;
    assert!(matches!(&missing, Err(StorageError::ReplayUnavailable(_))));
    storage.shutdown().await.expect("writer shutdown");
}
