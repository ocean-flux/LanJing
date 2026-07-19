//! Candidate credential ownership、bounded writer 与 artifact orphan 合同。

use super::*;

#[tokio::test]
async fn candidate_bound_source_credential_ref_survives_restart_and_is_consumed() {
    init_mock_keyring();
    let temp = TempStore::new("candidate-source-credential-restart");
    let storage = temp.open().await;
    let now = 1_750_000_150_000;
    let draft = candidate(now);
    let candidate_id = draft.candidate_id;
    storage
        .stage_candidate(draft)
        .await
        .expect("stage candidate before its credential snapshot");
    let snapshot = storage
        .stage_source_credentials(SourceCredentialInput {
            candidate_id,
            source_identity: "source:test".to_string(),
            secret_bytes: b"candidate-only-source-secret".to_vec(),
            created_at_ms: now + 1,
        })
        .await
        .expect("encrypt candidate-bound credential snapshot");
    assert_eq!(snapshot.cookie_namespace, "source/source:test");
    storage.shutdown().await.expect("close staging writer");

    let storage = temp.open().await;
    let recovered = storage
        .get_candidate_source_credentials_ref(candidate_id)
        .await
        .expect("read durable candidate credential ref after restart")
        .expect("credential ref remains staged");
    assert_eq!(recovered, snapshot);
    storage
        .install_candidate(InstallCandidateRequest {
            candidate_id,
            grant: PolicyCapabilities::default(),
            expected_source_version: 0,
            event_id: Uuid::new_v4(),
            trace_id: "trace-install-recovered-source-secret".to_string(),
            occurred_at_ms: now + 2,
            correlation_id: None,
            source_credentials: Some(recovered.clone()),
        })
        .await
        .expect("install with a restart-recovered credential ref");
    assert!(
        storage
            .get_candidate_source_credentials_ref(candidate_id)
            .await
            .expect("read consumed candidate credential ref")
            .is_none()
    );
    let installed_event = storage
        .source_events_after("source:test", 0)
        .await
        .expect("read source install event")
        .into_iter()
        .find(|event| {
            event.envelope.event_type == EventType::Source
                && event.envelope.payload["kind"].as_str() == Some("installed")
        })
        .expect("source install event exists");
    assert_eq!(
        installed_event.envelope.secret_refs,
        vec![recovered.secret_ref]
    );
    assert!(
        !installed_event
            .envelope
            .payload
            .to_string()
            .contains("candidate-only-source-secret")
    );
    assert!(
        collect_file_bytes(&temp.config.artifact_root)
            .iter()
            .all(|bytes| {
                !bytes
                    .windows(b"candidate-only-source-secret".len())
                    .any(|window| window == b"candidate-only-source-secret")
            })
    );
    storage.shutdown().await.expect("close installed storage");
}

#[tokio::test]
async fn source_credential_snapshot_rejects_tampering_and_wrong_candidate() {
    init_mock_keyring();
    let temp = TempStore::new("candidate-source-credential-ownership");
    let storage = temp.open().await;
    let now = 1_750_000_175_000;
    let first = candidate(now);
    let first_id = first.candidate_id;
    let second = candidate(now + 1);
    let second_id = second.candidate_id;
    storage
        .stage_candidate(first)
        .await
        .expect("stage first candidate");
    storage
        .stage_candidate(second)
        .await
        .expect("stage second candidate");
    assert!(matches!(
        storage
            .stage_source_credentials(SourceCredentialInput {
                candidate_id: first_id,
                source_identity: "source:other".to_string(),
                secret_bytes: b"wrong-source-secret".to_vec(),
                created_at_ms: now + 2,
            })
            .await,
        Err(StorageError::SourceCredentialUnavailable)
    ));
    let snapshot = storage
        .stage_source_credentials(SourceCredentialInput {
            candidate_id: first_id,
            source_identity: "source:test".to_string(),
            secret_bytes: b"owned-source-secret".to_vec(),
            created_at_ms: now + 3,
        })
        .await
        .expect("stage first candidate credential");
    let mut tampered = snapshot.clone();
    tampered.secret_ref.hash = hash("tampered-source-secret-ref");
    assert!(matches!(
        storage
            .install_candidate(InstallCandidateRequest {
                candidate_id: first_id,
                grant: PolicyCapabilities::default(),
                expected_source_version: 0,
                event_id: Uuid::new_v4(),
                trace_id: "trace-tampered-source-secret".to_string(),
                occurred_at_ms: now + 4,
                correlation_id: None,
                source_credentials: Some(tampered),
            })
            .await,
        Err(StorageError::SourceCredentialUnavailable)
    ));
    assert!(matches!(
        storage
            .install_candidate(InstallCandidateRequest {
                candidate_id: second_id,
                grant: PolicyCapabilities::default(),
                expected_source_version: 0,
                event_id: Uuid::new_v4(),
                trace_id: "trace-wrong-candidate-source-secret".to_string(),
                occurred_at_ms: now + 4,
                correlation_id: None,
                source_credentials: Some(snapshot.clone()),
            })
            .await,
        Err(StorageError::SourceCredentialUnavailable)
    ));
    assert_eq!(
        storage
            .get_candidate_source_credentials_ref(first_id)
            .await
            .expect("failed installs retain staged credential"),
        Some(snapshot.clone())
    );
    storage
        .install_candidate(InstallCandidateRequest {
            candidate_id: first_id,
            grant: PolicyCapabilities::default(),
            expected_source_version: 0,
            event_id: Uuid::new_v4(),
            trace_id: "trace-owned-source-secret".to_string(),
            occurred_at_ms: now + 5,
            correlation_id: None,
            source_credentials: Some(snapshot),
        })
        .await
        .expect("matching candidate credential installs");
    storage
        .shutdown()
        .await
        .expect("close ownership test storage");
}

#[tokio::test]
async fn required_source_credential_never_downgrades_to_credential_free_after_ref_loss() {
    init_mock_keyring();
    let temp = TempStore::new("candidate-source-credential-ref-loss");
    let storage = temp.open().await;
    let now = 1_750_000_190_000;
    let draft = candidate(now);
    let candidate_id = draft.candidate_id;
    storage
        .stage_candidate(draft)
        .await
        .expect("stage credential-bearing candidate");
    storage
        .stage_source_credentials(SourceCredentialInput {
            candidate_id,
            source_identity: "source:test".to_string(),
            secret_bytes: b"must-not-fall-back-to-live".to_vec(),
            created_at_ms: now + 1,
        })
        .await
        .expect("mark candidate as credential-required");
    storage
        .shutdown()
        .await
        .expect("close writer before ref loss");

    let database_url = temp.config.database_path.to_string_lossy().into_owned();
    let mut conn = SqliteConnection::establish(&database_url).expect("open real SQLite database");
    sql_query("DELETE FROM source_credential_staging WHERE candidate_id = ?")
        .bind::<diesel::sql_types::Text, _>(candidate_id.to_string())
        .execute(&mut conn)
        .expect("simulate missing staged credential ref");
    drop(conn);

    let storage = temp.open().await;
    assert!(matches!(
        storage
            .get_candidate_source_credentials_ref(candidate_id)
            .await,
        Err(StorageError::SourceCredentialUnavailable)
    ));
    assert!(matches!(
        storage
            .install_candidate(InstallCandidateRequest {
                candidate_id,
                grant: PolicyCapabilities::default(),
                expected_source_version: 0,
                event_id: Uuid::new_v4(),
                trace_id: "trace-missing-source-credential-ref".to_string(),
                occurred_at_ms: now + 2,
                correlation_id: None,
                source_credentials: None,
            })
            .await,
        Err(StorageError::SourceCredentialUnavailable)
    ));
    storage.shutdown().await.expect("close ref-loss storage");
}

#[tokio::test]
async fn writer_is_bounded_and_sixteen_writers_receive_durable_receipts() {
    let temp = TempStore::new("writer");
    let storage = temp.open().await;
    assert_eq!(storage.writer_capacity(), WRITER_CAPACITY);
    assert_eq!(WRITER_CAPACITY, 256);

    let mut tasks = Vec::new();
    for index in 0..16 {
        let storage = storage.clone();
        tasks.push(tokio::spawn(async move {
            storage
                .append_event(AppendRequest {
                    stream_id: format!("writer/{index}"),
                    expected_version: 0,
                    event_id: Uuid::new_v4(),
                    event_type: EventType::Other("writer_test".to_string()),
                    schema_version: 1,
                    correlation_id: None,
                    causation_id: None,
                    trace_id: format!("trace-writer-{index}"),
                    occurred_at_ms: 1_750_000_200_000 + index,
                    payload: serde_json::json!({"index": index}),
                    source_id: None,
                    artifacts: Vec::new(),
                })
                .await
        }));
    }
    let mut sequences = tasks
        .into_iter()
        .map(|task| async move {
            task.await
                .expect("writer task join")
                .expect("bounded writer receipt")
                .global_seq
        })
        .collect::<Vec<_>>();
    let mut results = Vec::new();
    for sequence in sequences.drain(..) {
        results.push(sequence.await);
    }
    results.sort_unstable();
    assert_eq!(results, (1_u64..=16).collect::<Vec<_>>());
    storage.shutdown().await.expect("writer shutdown");
}

#[tokio::test]
async fn failed_event_transaction_leaves_recoverable_artifact_orphan() {
    let temp = TempStore::new("orphan");
    let storage = temp.open().await;
    let failed = storage
        .append_event(AppendRequest {
            stream_id: "orphan/test".to_string(),
            expected_version: 1,
            event_id: Uuid::new_v4(),
            event_type: EventType::Other("orphan_test".to_string()),
            schema_version: 1,
            correlation_id: None,
            causation_id: None,
            trace_id: "trace-orphan".to_string(),
            occurred_at_ms: 1_750_000_300_000,
            payload: serde_json::json!({"kind": "expected_conflict"}),
            source_id: None,
            artifacts: vec![ArtifactInput {
                kind: ArtifactKind::Body,
                bytes: b"orphan body".to_vec(),
            }],
        })
        .await
        .expect_err("wrong expected version fails after artifact durable write");
    assert!(matches!(failed, StorageError::VersionConflict { .. }));
    storage.shutdown().await.expect("writer shutdown");

    let restarted = temp.open().await;
    assert!(collect_file_bytes(&temp.config.artifact_root).is_empty());
    restarted.shutdown().await.expect("writer shutdown");
}
