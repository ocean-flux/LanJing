//! Candidate credential ownership、bounded writer 与 artifact orphan 合同。

use super::*;

#[tokio::test]
async fn candidate_bound_runtime_credential_survives_restart_and_is_consumed() {
    init_mock_keyring();
    let temp = TempStore::new("candidate-runtime-credential-restart");
    let storage = temp.open().await;
    let now = current_time_ms();
    let mut draft = candidate(now);
    let candidate_id = draft.candidate_id;
    draft.runtime_credentials = Some(RuntimeCredentialMaterial::new(
        b"candidate-only-source-secret".to_vec(),
    ));
    storage
        .stage_candidate(draft)
        .await
        .expect("atomically stage candidate and runtime credential");
    storage.shutdown().await.expect("close staging writer");

    let storage = temp.open().await;
    assert!(
        storage
            .get_candidate_summary(candidate_id)
            .await
            .expect("read candidate after restart")
            .is_some()
    );
    storage
        .install_candidate(InstallCandidateRequest {
            candidate_id,
            grant: SystemCapabilities::default(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-install-recovered-source-secret".to_string(),
            occurred_at_ms: now + 2,
            correlation_id: None,
        })
        .await
        .expect("consume candidate with its staged runtime credential");
    assert!(
        storage
            .get_candidate_summary(candidate_id)
            .await
            .expect("read consumed candidate")
            .is_none()
    );

    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-runtime-credential-execution".to_string(),
            started_at_ms: now + 3,
            correlation_id: None,
        })
        .await
        .expect("start execution pinned to installed credential");
    assert_eq!(
        storage
            .load_execution_source_credentials(execution_id)
            .await
            .expect("decrypt execution-pinned runtime credential")
            .into_secret_bytes(),
        Some(b"candidate-only-source-secret".to_vec())
    );
    let installed_event = storage
        .source_events_after("source:test", 0)
        .await
        .expect("read source install event")
        .into_iter()
        .find(|event| event.envelope.payload["kind"].as_str() == Some("installed"))
        .expect("source install event exists");
    assert!(installed_event.envelope.secret_refs.is_empty());
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
            .all(|bytes| !bytes
                .windows(b"candidate-only-source-secret".len())
                .any(|window| { window == b"candidate-only-source-secret" }))
    );
    storage.shutdown().await.expect("close installed storage");
}

#[tokio::test]
async fn missing_candidate_runtime_secret_never_downgrades_to_credential_free() {
    init_mock_keyring();
    let temp = TempStore::new("candidate-runtime-credential-ref-loss");
    let storage = temp.open().await;
    let now = current_time_ms();
    let mut draft = candidate(now);
    let candidate_id = draft.candidate_id;
    draft.runtime_credentials = Some(RuntimeCredentialMaterial::new(
        b"must-not-fall-back-to-live".to_vec(),
    ));
    storage
        .stage_candidate(draft)
        .await
        .expect("stage credential-bearing candidate");
    storage
        .shutdown()
        .await
        .expect("close writer before ref loss");

    let conn = open_test_connection(&temp.config.database_path).await;
    test_statement("PRAGMA foreign_keys = OFF")
        .execute(&conn)
        .await
        .expect("disable foreign keys for corruption injection");
    test_statement(
        "DELETE FROM secret_artifact_projection WHERE secret_id = (SELECT runtime_credential_secret_id FROM candidate_projection WHERE candidate_id = ?)",
    )
    .bind(candidate_id.to_string())
    .execute(&conn)
    .await
    .expect("simulate missing runtime credential projection");
    drop(conn);

    let storage = temp.open().await;
    assert!(matches!(
        storage
            .install_candidate(InstallCandidateRequest {
                candidate_id,
                grant: SystemCapabilities::default(),
                event_id: Uuid::new_v4(),
                trace_id: "trace-missing-runtime-secret".to_string(),
                occurred_at_ms: now + 2,
                correlation_id: None,
            })
            .await,
        Err(StorageError::CandidateTampered)
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

#[tokio::test]
#[ignore = "D12 performance calibration; run with cargo test --release on target hardware"]
async fn d12_sixteen_writer_receipt_latency_p95_gate() {
    const WRITERS: usize = 16;
    const P95_LIMIT: std::time::Duration = std::time::Duration::from_millis(250);

    if cfg!(debug_assertions) {
        eprintln!("D12 timing gate requires a release build");
        return;
    }
    let temp = TempStore::new("d12-writer");
    let storage = temp.open().await;
    let mut tasks = Vec::with_capacity(WRITERS);
    for index in 0..WRITERS {
        let storage = storage.clone();
        tasks.push(tokio::spawn(async move {
            let started = std::time::Instant::now();
            let receipt = storage
                .append_event(AppendRequest {
                    stream_id: format!("d12-writer/{index}"),
                    expected_version: 0,
                    event_id: Uuid::new_v4(),
                    event_type: EventType::Other("d12_writer".to_string()),
                    schema_version: 1,
                    correlation_id: None,
                    causation_id: None,
                    trace_id: format!("trace-d12-writer-{index}"),
                    occurred_at_ms: 1_750_000_800_000
                        + i64::try_from(index).expect("writer timestamp"),
                    payload: serde_json::json!({"index": index}),
                    source_id: None,
                    artifacts: Vec::new(),
                })
                .await
                .expect("D12 writer durable receipt");
            (receipt.global_seq, started.elapsed())
        }));
    }
    let mut sequences = Vec::with_capacity(WRITERS);
    let mut elapsed = Vec::with_capacity(WRITERS);
    for task in tasks {
        let (sequence, duration) = task.await.expect("D12 writer task join");
        sequences.push(sequence);
        elapsed.push(duration);
    }
    sequences.sort_unstable();
    elapsed.sort_unstable();
    assert_eq!(sequences, (1_u64..=16).collect::<Vec<_>>());
    let p95 = elapsed[(WRITERS * 95).div_ceil(100) - 1];
    assert!(
        p95 <= P95_LIMIT,
        "D12 16-writer durable receipt latency p95 was {p95:?}, limit is {P95_LIMIT:?}"
    );
    eprintln!(
        "D12 release gate: writer_16_receipt_latency_p95={p95:?}, queue_wait_is_bounded_above_by_receipt_latency=true, payload=synthetic_index_json, artifact_refs=0"
    );
    storage.shutdown().await.expect("writer shutdown");
}
