//! Event projection 原子性与 effect archive 安全合同。

use super::*;

fn invocation(node_id: Uuid, ordinal: u64) -> InvocationPath {
    InvocationPath::new(node_id, Vec::new(), ordinal).expect("valid invocation fixture")
}

fn loop_invocation(
    node_id: Uuid,
    loop_id: Uuid,
    iteration_index: u32,
    ordinal: u64,
) -> InvocationPath {
    InvocationPath::new(
        node_id,
        vec![LoopInvocationSegment {
            loop_id,
            iteration_index,
        }],
        ordinal,
    )
    .expect("valid Loop invocation fixture")
}

fn http_capture(
    execution_id: Uuid,
    effect_id: Uuid,
    invocation_path: InvocationPath,
    label: &str,
) -> EffectCapture {
    EffectCapture::from_live(
        execution_id,
        effect_id,
        invocation_path,
        hash(label),
        CapturedEffectOutput::new(
            EffectOutput::Http(HttpResponse {
                status: 200,
                headers: HashMap::new(),
                body: label.as_bytes().to_vec(),
                charset: Some("utf-8".to_string()),
            }),
            http_witness(HttpMethod::Get, None),
        ),
    )
    .expect("valid invocation capture fixture")
}

async fn tamper_control_trace(temp: &TempStore, execution_id: Uuid) {
    let conn = open_test_connection(&temp.config.database_path).await;
    test_statement(
        "UPDATE control_traces SET trace_json = '{\"kind\":\"loop\",\"iteration_count\":1}' WHERE execution_id = ? AND invocation_ordinal = 1",
    )
    .bind(execution_id.to_string())
    .execute(&conn)
    .await
    .expect("tamper control trace without matching hash");
}

#[tokio::test]
async fn event_projection_is_atomic_idempotent_and_catchable() {
    let temp = TempStore::new("atomic");
    let storage = temp.open().await;
    let now = 1_750_000_000_000;
    install_source(&storage, now).await;

    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-execution".to_string(),
            started_at_ms: now + 2,
            correlation_id: None,
        })
        .await
        .expect("execution start");

    let delta_event_id = Uuid::new_v4();
    let first = storage
        .commit_execution_delta(DeltaCommit {
            execution_id,
            expected_version: 1,
            event_id: delta_event_id,
            trace_id: "trace-delta".to_string(),
            occurred_at_ms: now + 3,
            delta: delta_with_item("source:test", "第一版"),
        })
        .await
        .expect("event and normalized projection commit together");
    assert_eq!(first.stream_version, 2);
    assert_eq!(
        storage
            .get_item(MediaResourceId("item:test:1".to_string()))
            .await
            .expect("indexed item query")
            .expect("item projection")
            .title,
        "第一版"
    );
    assert_eq!(
        storage
            .list_units_for_item(MediaResourceId("item:test:1".to_string()), 0, 50)
            .await
            .expect("item unit index")
            .len(),
        1
    );
    assert_eq!(
        storage
            .list_assets_for_unit(MediaResourceId("unit:test:1".to_string()), 0, 50)
            .await
            .expect("unit asset index")
            .len(),
        1
    );

    let replay = storage
        .commit_execution_delta(DeltaCommit {
            execution_id,
            expected_version: 1,
            event_id: delta_event_id,
            trace_id: "trace-delta".to_string(),
            occurred_at_ms: now + 3,
            delta: delta_with_item("source:test", "第一版"),
        })
        .await
        .expect("same event ID must be idempotent");
    assert_eq!(replay, first);
    assert_eq!(
        storage
            .catch_up_execution(execution_id, 0)
            .await
            .expect("session catch-up")
            .len(),
        2
    );

    let tombstone_event_id = Uuid::new_v4();
    let tombstone = ProjectionDelta {
        upserts: MediaGraphDelta::default(),
        tombstones: ProjectionTombstones {
            items: vec![MediaResourceId("item:test:1".to_string())],
            units: vec![MediaResourceId("unit:test:1".to_string())],
            assets: vec![MediaResourceId("asset:test:1".to_string())],
            ..ProjectionTombstones::default()
        },
    };
    storage
        .commit_execution_delta(DeltaCommit {
            execution_id,
            expected_version: 2,
            event_id: tombstone_event_id,
            trace_id: "trace-tombstone".to_string(),
            occurred_at_ms: now + 4,
            delta: tombstone.clone(),
        })
        .await
        .expect("tombstone event and projection delete");
    assert!(
        storage
            .get_item(MediaResourceId("item:test:1".to_string()))
            .await
            .expect("item query after tombstone")
            .is_none()
    );
    storage
        .commit_execution_delta(DeltaCommit {
            execution_id,
            expected_version: 2,
            event_id: tombstone_event_id,
            trace_id: "trace-tombstone".to_string(),
            occurred_at_ms: now + 4,
            delta: tombstone,
        })
        .await
        .expect("tombstone replay idempotency");

    let conflict = storage
        .commit_execution_delta(DeltaCommit {
            execution_id,
            expected_version: 0,
            event_id: Uuid::new_v4(),
            trace_id: "trace-conflict".to_string(),
            occurred_at_ms: now + 5,
            delta: delta_with_item("source:test", "不得出现"),
        })
        .await
        .expect_err("过期版本不得半提交 Event 或 projection");
    assert!(matches!(conflict, StorageError::VersionConflict { .. }));
    assert!(
        storage
            .get_item(MediaResourceId("item:test:1".to_string()))
            .await
            .expect("conflict rollback projection")
            .is_none()
    );
    assert_eq!(
        storage
            .catch_up_execution(execution_id, 0)
            .await
            .expect("conflict rollback event")
            .len(),
        3
    );
    storage.shutdown().await.expect("writer shutdown");
}

#[tokio::test]
async fn effect_archive_is_durable_redacted_and_explicit_when_master_key_is_lost() {
    init_mock_keyring();
    let temp = TempStore::new("effect");
    let storage = temp.open().await;
    let now = 1_750_000_100_000;
    install_source(&storage, now).await;
    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-effect".to_string(),
            started_at_ms: now + 2,
            correlation_id: None,
        })
        .await
        .expect("execution start");

    let effect_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let output = std::sync::Arc::new(EffectOutput::Http(HttpResponse {
        status: 200,
        headers: HashMap::from([
            ("content-type".to_string(), "application/json".to_string()),
            ("set-cookie".to_string(), "session=supersecret".to_string()),
        ]),
        body: b"body".to_vec(),
        charset: Some("utf-8".to_string()),
    }));
    let witness = http_witness(HttpMethod::Get, None);
    let output_hash = effect_output_hash(output.as_ref()).expect("canonical output hash");
    let witness_hash = witness.canonical_hash().expect("canonical witness hash");
    let capture = EffectCapture::from_archived(ArchivedEffectCapture {
        execution_id,
        effect_id,
        invocation_path: invocation(node_id, 1),
        kind: EffectKind::Http,
        fingerprint: hash("fingerprint"),
        output_hash,
        witness_hash,
        output,
        witness,
    })
    .expect("valid archived capture fixture");
    let receipt = EffectArchive::persist_durable(&storage, capture.clone())
        .await
        .expect("effect body, secret and event durable receipt");
    assert_eq!(receipt.effect_id, effect_id);
    assert_eq!(receipt.fingerprint, capture.fingerprint);
    assert_eq!(receipt.output_hash, capture.output_hash);
    assert_eq!(receipt.witness_hash, capture.witness_hash);

    let replay = EffectArchive::load_replay(
        &storage,
        EffectReplayLookup {
            archived_execution_id: execution_id,
            invocation_path: invocation(node_id, 1),
            kind: EffectKind::Http,
        },
    )
    .await
    .expect("read durable replay")
    .expect("capture exists");
    assert_eq!(replay.output, capture.output);
    assert_eq!(replay.witness, capture.witness);
    assert_eq!(replay.witness_hash, capture.witness_hash);

    let disk_bytes = collect_file_bytes(&temp.config.artifact_root);
    assert!(disk_bytes.iter().all(|bytes| {
        !bytes
            .windows(b"supersecret".len())
            .any(|window| window == b"supersecret")
    }));

    storage.shutdown().await.expect("writer shutdown");
    wipe_master_key(&temp).await;
    let restarted = temp.open().await;
    assert!(
        EffectArchive::load_replay(
            &restarted,
            EffectReplayLookup {
                archived_execution_id: execution_id,
                invocation_path: invocation(node_id, 1),
                kind: EffectKind::Http,
            },
        )
        .await
        .is_err()
    );
    restarted.shutdown().await.expect("writer shutdown");
}

#[test]
fn invocation_path_round_trips_nested_segments_without_opening_nested_execution() {
    let outer = Uuid::new_v4();
    let inner = Uuid::new_v4();
    let path = InvocationPath::new(
        Uuid::new_v4(),
        vec![
            LoopInvocationSegment {
                loop_id: outer,
                iteration_index: 2,
            },
            LoopInvocationSegment {
                loop_id: inner,
                iteration_index: 7,
            },
        ],
        9,
    )
    .expect("nested identity wire is representable");
    let wire = serde_json::to_vec(&path).expect("serialize nested invocation path");
    let decoded: InvocationPath =
        serde_json::from_slice(&wire).expect("deserialize nested invocation path");
    assert_eq!(decoded, path);
    assert_eq!(decoded.loop_iterations()[0].loop_id, outer);
    assert_eq!(decoded.loop_iterations()[1].loop_id, inner);
}

#[tokio::test]
async fn invocation_ledger_replays_same_node_iterations_and_control_trace_exactly() {
    init_mock_keyring();
    let temp = TempStore::new("invocation-ledger");
    let storage = temp.open().await;
    let now = 1_750_000_110_000;
    install_source(&storage, now).await;
    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-invocation-ledger".to_string(),
            started_at_ms: now + 1,
            correlation_id: None,
        })
        .await
        .expect("execution start");

    let loop_id = Uuid::new_v4();
    let effect_node_id = Uuid::new_v4();
    let trace_path = invocation(loop_id, 1);
    let trace = ControlTraceCapture::new(
        execution_id,
        trace_path.clone(),
        ControlTrace::Loop { iteration_count: 2 },
    )
    .expect("valid Loop trace");
    EffectArchive::persist_control_trace(&storage, trace.clone())
        .await
        .expect("persist Loop trace");

    let first_path = loop_invocation(effect_node_id, loop_id, 0, 2);
    let second_path = loop_invocation(effect_node_id, loop_id, 1, 3);
    let first = http_capture(
        execution_id,
        Uuid::new_v4(),
        first_path.clone(),
        "iteration-0",
    );
    let second = http_capture(
        execution_id,
        Uuid::new_v4(),
        second_path.clone(),
        "iteration-1",
    );
    EffectArchive::persist_durable(&storage, first.clone())
        .await
        .expect("persist first iteration");
    EffectArchive::persist_durable(&storage, second.clone())
        .await
        .expect("persist second iteration");
    let mut mismatched_current = first.clone();
    mismatched_current.invocation_path =
        loop_invocation(effect_node_id, loop_id, 0, second_path.ordinal());
    let mismatch = EffectArchive::persist_durable(&storage, mismatched_current).await;
    assert!(matches!(
        mismatch,
        Err(error) if error.code == EffectArchiveErrorCode::Integrity
    ));

    let replayed_trace = EffectArchive::load_control_trace(
        &storage,
        ControlReplayLookup {
            archived_execution_id: execution_id,
            invocation_path: trace_path.clone(),
        },
    )
    .await
    .expect("read Loop trace")
    .expect("Loop trace exists");
    assert_eq!(replayed_trace.trace, trace.trace);
    for (path, expected) in [(first_path.clone(), &first), (second_path.clone(), &second)] {
        let replay = EffectArchive::load_replay(
            &storage,
            EffectReplayLookup {
                archived_execution_id: execution_id,
                invocation_path: path,
                kind: EffectKind::Http,
            },
        )
        .await
        .expect("read exact iteration")
        .expect("iteration capture exists");
        assert_eq!(replay.effect_id, expected.effect_id);
        assert_eq!(replay.output, expected.output);
    }

    let wrong_order = EffectArchive::load_replay(
        &storage,
        EffectReplayLookup {
            archived_execution_id: execution_id,
            invocation_path: loop_invocation(effect_node_id, loop_id, 0, 3),
            kind: EffectKind::Http,
        },
    )
    .await;
    assert!(matches!(
        wrong_order,
        Err(error) if error.code == EffectArchiveErrorCode::Integrity
    ));
    let missing = EffectArchive::load_replay(
        &storage,
        EffectReplayLookup {
            archived_execution_id: execution_id,
            invocation_path: loop_invocation(effect_node_id, loop_id, 2, 4),
            kind: EffectKind::Http,
        },
    )
    .await
    .expect("missing exact iteration is represented as None");
    assert!(missing.is_none());
    EffectArchive::validate_replay_complete(
        &storage,
        ReplayCompletionLookup {
            archived_execution_id: execution_id,
            observed_invocation_count: 3,
        },
    )
    .await
    .expect("complete invocation sequence");
    let extra = EffectArchive::validate_replay_complete(
        &storage,
        ReplayCompletionLookup {
            archived_execution_id: execution_id,
            observed_invocation_count: 2,
        },
    )
    .await;
    assert!(matches!(
        extra,
        Err(error) if error.code == EffectArchiveErrorCode::Integrity
    ));
    storage.shutdown().await.expect("close before trace tamper");
    tamper_control_trace(&temp, execution_id).await;
    let storage = temp.open().await;
    let tampered_trace = EffectArchive::load_control_trace(
        &storage,
        ControlReplayLookup {
            archived_execution_id: execution_id,
            invocation_path: trace_path,
        },
    )
    .await;
    assert!(matches!(
        tampered_trace,
        Err(error) if error.code == EffectArchiveErrorCode::Integrity
    ));
    storage
        .shutdown()
        .await
        .expect("close tampered trace storage");
}

#[tokio::test]
async fn invocation_ledger_tamper_is_a_typed_rejection() {
    init_mock_keyring();
    let temp = TempStore::new("invocation-ledger-tamper");
    let storage = temp.open().await;
    let now = 1_750_000_112_000;
    install_source(&storage, now).await;
    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-invocation-tamper".to_string(),
            started_at_ms: now + 1,
            correlation_id: None,
        })
        .await
        .expect("execution start");
    let node_id = Uuid::new_v4();
    let path = invocation(node_id, 1);
    let capture = http_capture(execution_id, Uuid::new_v4(), path.clone(), "tamper-ledger");
    EffectArchive::persist_durable(&storage, capture)
        .await
        .expect("persist current capture");
    storage.shutdown().await.expect("close before tamper");

    let conn = open_test_connection(&temp.config.database_path).await;
    test_statement(
        "UPDATE execution_invocation_ledger SET payload_id = 'tampered-effect-id' WHERE execution_id = ?",
    )
    .bind(execution_id.to_string())
    .execute(&conn)
    .await
    .expect("tamper invocation ledger payload ownership");
    drop(conn);
    let storage = temp.open().await;
    let tampered = EffectArchive::load_replay(
        &storage,
        EffectReplayLookup {
            archived_execution_id: execution_id,
            invocation_path: path,
            kind: EffectKind::Http,
        },
    )
    .await;
    assert!(matches!(
        tampered,
        Err(error) if error.code == EffectArchiveErrorCode::Integrity
    ));
    storage.shutdown().await.expect("close tampered storage");
}

#[tokio::test]
async fn live_http_request_body_is_encrypted_and_events_only_carry_its_ref() {
    init_mock_keyring();
    let temp = TempStore::new("effect-request-body-secret");
    let storage = temp.open().await;
    let now = 1_750_000_115_000;
    install_source(&storage, now).await;
    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-effect-request-body".to_string(),
            started_at_ms: now + 1,
            correlation_id: None,
        })
        .await
        .expect("execution start");

    let request_body = b"access_token=body-supersecret".to_vec();
    let request_body_hash = effect_bytes_hash(&request_body);
    let effect_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let witness = http_witness(
        HttpMethod::Post,
        Some(HttpRequestBodyWitness {
            hash: request_body_hash.clone(),
            byte_len: u64::try_from(request_body.len()).expect("request body length"),
        }),
    );
    let captured = CapturedEffectOutput::new(
        EffectOutput::Http(HttpResponse {
            status: 201,
            headers: HashMap::from([("content-type".to_string(), "text/plain".to_string())]),
            body: b"created".to_vec(),
            charset: Some("utf-8".to_string()),
        }),
        witness,
    )
    .with_http_request_body(Some(request_body.clone()));
    let capture = EffectCapture::from_live(
        execution_id,
        effect_id,
        invocation(node_id, 1),
        hash("request-body-secret-fingerprint"),
        captured,
    )
    .expect("valid live HTTP capture");
    let receipt = EffectArchive::persist_durable(&storage, capture.clone())
        .await
        .expect("durable encrypted request body receipt");
    assert_eq!(receipt.witness_hash, capture.witness_hash);

    let events = storage
        .catch_up_execution(execution_id, 0)
        .await
        .expect("read durable execution events");
    let event = events
        .iter()
        .find(|event| event.envelope.event_id == effect_id)
        .expect("effect capture event exists");
    assert_eq!(
        event.envelope.payload["has_request_body"].as_bool(),
        Some(true)
    );
    assert!(
        !event
            .envelope
            .payload
            .to_string()
            .contains("body-supersecret")
    );
    assert!(
        event
            .envelope
            .artifact_refs
            .iter()
            .all(|artifact| artifact.hash != request_body_hash),
        "request body must not be a plaintext Body Artifact ref"
    );
    assert!(
        event.envelope.secret_refs.is_empty(),
        "random vault secret IDs are owned in SQLite and never published in Events"
    );

    let replay = EffectArchive::load_replay(
        &storage,
        EffectReplayLookup {
            archived_execution_id: execution_id,
            invocation_path: invocation(node_id, 1),
            kind: EffectKind::Http,
        },
    )
    .await
    .expect("read replay from encrypted request body archive")
    .expect("capture exists");
    assert_eq!(replay.output, capture.output);
    assert_eq!(replay.witness, capture.witness);
    assert!(
        replay.request_body().is_none(),
        "replay must not deliver raw request material"
    );

    let persisted_bytes = collect_file_bytes(&temp.root);
    assert!(persisted_bytes.iter().all(|bytes| {
        !bytes
            .windows(b"body-supersecret".len())
            .any(|window| window == b"body-supersecret")
    }));

    storage
        .shutdown()
        .await
        .expect("close request body storage");
    let conn = open_test_connection(&temp.config.database_path).await;
    let artifact = test_statement(
        "SELECT secret.key_id, secret.blob_locator, secret.ciphertext_hash FROM effect_captures AS effect JOIN secret_artifact_projection AS secret ON secret.secret_id = effect.request_body_secret_id WHERE effect.execution_id = ? AND effect.effect_id = ?",
    )
    .bind(execution_id.to_string())
    .bind(effect_id.to_string())
    .get_result::<ArtifactSecurityTestRow>(&conn)
    .await
    .expect("request body random secret artifact metadata");
    assert!(Uuid::parse_str(&artifact.key_id).is_ok());
    assert!(artifact.blob_locator.starts_with("vault/"));
    assert!(
        std::path::Path::new(&artifact.blob_locator)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("vault"))
    );
    assert_eq!(artifact.ciphertext_hash.len(), 64);
    assert_ne!(artifact.ciphertext_hash, request_body_hash);
}

#[tokio::test]
async fn request_body_material_mismatch_is_rejected_before_a_durable_receipt() {
    init_mock_keyring();
    let temp = TempStore::new("effect-request-body-mismatch");
    let storage = temp.open().await;
    let now = 1_750_000_118_000;
    install_source(&storage, now).await;
    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-effect-request-body-mismatch".to_string(),
            started_at_ms: now + 1,
            correlation_id: None,
        })
        .await
        .expect("execution start");

    let request_body = b"actual-sensitive-body".to_vec();
    let witness = http_witness(
        HttpMethod::Post,
        Some(HttpRequestBodyWitness {
            hash: effect_bytes_hash(b"different-sensitive-body"),
            byte_len: u64::try_from(request_body.len()).expect("request body length"),
        }),
    );
    let capture = EffectCapture::from_live(
        execution_id,
        Uuid::new_v4(),
        invocation(Uuid::new_v4(), 1),
        hash("request-body-mismatch-fingerprint"),
        CapturedEffectOutput::new(
            EffectOutput::Http(HttpResponse {
                status: 200,
                headers: HashMap::new(),
                body: b"ignored".to_vec(),
                charset: None,
            }),
            witness,
        )
        .with_http_request_body(Some(request_body)),
    )
    .expect("live capture retains opaque request material");
    assert!(
        EffectArchive::persist_durable(&storage, capture)
            .await
            .is_err(),
        "C2 must bind raw request material to its witness before accepting a receipt"
    );
    assert_eq!(
        storage
            .catch_up_execution(execution_id, 0)
            .await
            .expect("read execution after rejected capture")
            .len(),
        1,
        "rejected capture must not append an execution event"
    );
    storage.shutdown().await.expect("close mismatch storage");
}

#[tokio::test]
async fn typed_http_failure_is_archived_without_a_fake_response() {
    init_mock_keyring();
    let temp = TempStore::new("effect-http-failure");
    let storage = temp.open().await;
    let now = 1_750_000_120_000;
    install_source(&storage, now).await;
    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-effect-http-failure".to_string(),
            started_at_ms: now + 1,
            correlation_id: None,
        })
        .await
        .expect("execution start");

    let effect_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let mut witness = http_witness(HttpMethod::Get, None);
    let EffectWitness::Http(http_witness) = &mut witness else {
        panic!("HTTP fixture must create an HTTP witness");
    };
    http_witness.error = Some(HttpEffectErrorKind::Request);
    let capture = EffectCapture::from_live(
        execution_id,
        effect_id,
        invocation(node_id, 1),
        hash("typed-http-failure-fingerprint"),
        CapturedEffectOutput::new(
            EffectOutput::Failure(EffectFailure::Http {
                error: HttpEffectErrorKind::Request,
            }),
            witness,
        ),
    )
    .expect("valid typed HTTP failure capture");
    EffectArchive::persist_durable(&storage, capture.clone())
        .await
        .expect("durably archive typed HTTP failure");

    let replay = EffectArchive::load_replay(
        &storage,
        EffectReplayLookup {
            archived_execution_id: execution_id,
            invocation_path: invocation(node_id, 1),
            kind: EffectKind::Http,
        },
    )
    .await
    .expect("read durable typed HTTP failure")
    .expect("typed failure capture exists");
    assert_eq!(replay.output, capture.output);
    assert_eq!(replay.witness, capture.witness);
    assert!(matches!(
        replay.output.as_ref(),
        EffectOutput::Failure(EffectFailure::Http {
            error: HttpEffectErrorKind::Request
        })
    ));
    storage
        .shutdown()
        .await
        .expect("close HTTP failure storage");
}

#[tokio::test]
async fn effect_witness_artifact_tampering_and_loss_block_replay() {
    init_mock_keyring();
    let temp = TempStore::new("effect-witness-integrity");
    let storage = temp.open().await;
    let now = 1_750_000_125_000;
    install_source(&storage, now).await;
    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-effect-witness-integrity".to_string(),
            started_at_ms: now + 1,
            correlation_id: None,
        })
        .await
        .expect("execution start");

    let effect_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    let output = std::sync::Arc::new(EffectOutput::Http(HttpResponse {
        status: 204,
        headers: HashMap::new(),
        body: Vec::new(),
        charset: None,
    }));
    let witness = http_witness(HttpMethod::Get, None);
    let capture = EffectCapture::from_archived(ArchivedEffectCapture {
        execution_id,
        effect_id,
        invocation_path: invocation(node_id, 1),
        kind: EffectKind::Http,
        fingerprint: hash("witness-integrity-fingerprint"),
        output_hash: effect_output_hash(output.as_ref()).expect("canonical output hash"),
        witness_hash: witness.canonical_hash().expect("canonical witness hash"),
        output,
        witness,
    })
    .expect("valid archived capture fixture");
    EffectArchive::persist_durable(&storage, capture.clone())
        .await
        .expect("persist effect witness artifact");
    storage
        .shutdown()
        .await
        .expect("close writer before file tamper");

    let conn = open_test_connection(&temp.config.database_path).await;
    let witness_artifact = test_statement(
        "SELECT artifact_metadata.relative_path, artifact_metadata.ref_count FROM effect_captures INNER JOIN artifact_metadata ON artifact_metadata.hash = effect_captures.witness_artifact_hash AND artifact_metadata.artifact_kind = 'body' WHERE effect_captures.execution_id = ? AND effect_captures.effect_id = ?",
    )
    .bind(execution_id.to_string())
    .bind(effect_id.to_string())
    .get_result::<ArtifactMetadataTestRow>(&conn)
    .await
    .expect("find witness artifact metadata");
    drop(conn);
    let witness_path = temp
        .config
        .artifact_root
        .join(witness_artifact.relative_path);
    let original = fs::read(&witness_path).expect("read durable witness artifact");
    fs::write(&witness_path, b"tampered witness artifact").expect("tamper witness artifact");

    let storage = temp.open().await;
    assert!(
        EffectArchive::load_replay(
            &storage,
            EffectReplayLookup {
                archived_execution_id: execution_id,
                invocation_path: invocation(node_id, 1),
                kind: EffectKind::Http,
            },
        )
        .await
        .is_err(),
        "tampered witness must not become a live replay"
    );
    assert!(
        EffectArchive::persist_durable(&storage, capture.clone())
            .await
            .is_err(),
        "idempotent receipt must not hide a corrupted durable witness"
    );
    storage.shutdown().await.expect("close tampered storage");

    fs::write(&witness_path, original).expect("restore witness artifact for loss check");
    let storage = temp.open().await;
    assert!(
        EffectArchive::load_replay(
            &storage,
            EffectReplayLookup {
                archived_execution_id: execution_id,
                invocation_path: invocation(node_id, 1),
                kind: EffectKind::Http,
            },
        )
        .await
        .expect("restored witness replay result")
        .is_some()
    );
    storage.shutdown().await.expect("close restored storage");

    fs::remove_file(&witness_path).expect("remove witness artifact");
    let storage = temp.open().await;
    assert!(
        EffectArchive::load_replay(
            &storage,
            EffectReplayLookup {
                archived_execution_id: execution_id,
                invocation_path: invocation(node_id, 1),
                kind: EffectKind::Http,
            },
        )
        .await
        .is_err(),
        "missing witness must not become a live replay"
    );
    storage
        .shutdown()
        .await
        .expect("close missing-witness storage");
}

#[tokio::test]
async fn delta_replay_after_terminal_is_idempotent_and_cross_source_ids_are_protected() {
    let temp = TempStore::new("delta-ownership-and-terminal-replay");
    let storage = temp.open().await;
    let now = 1_750_001_300_000;

    let alpha = candidate_for_source(now, "source:alpha");
    let alpha_grant = alpha.required_grant.clone();
    install_draft(&storage, alpha, 0, alpha_grant, now + 1).await;
    let alpha_execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id: alpha_execution_id,
            source_identity: "source:alpha".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-alpha-start".to_string(),
            started_at_ms: now + 2,
            correlation_id: None,
        })
        .await
        .expect("start alpha execution");
    let delta_event_id = Uuid::new_v4();
    let delta = delta_with_item("source:alpha", "owned by alpha");
    let first = storage
        .commit_execution_delta(DeltaCommit {
            execution_id: alpha_execution_id,
            expected_version: 1,
            event_id: delta_event_id,
            trace_id: "trace-alpha-delta".to_string(),
            occurred_at_ms: now + 3,
            delta: delta.clone(),
        })
        .await
        .expect("commit alpha delta");
    let same_owner_no_op = storage
        .commit_execution_delta(DeltaCommit {
            execution_id: alpha_execution_id,
            expected_version: 2,
            event_id: Uuid::new_v4(),
            trace_id: "trace-alpha-no-op".to_string(),
            occurred_at_ms: now + 3,
            delta: delta.clone(),
        })
        .await
        .expect("same-source identical upsert is a legal no-op");
    assert!(same_owner_no_op.global_seq > first.global_seq);
    storage
        .finish_execution(ExecutionFinish {
            execution_id: alpha_execution_id,
            expected_version: 3,
            event_id: Uuid::new_v4(),
            status: ExecutionStatus::Completed,
            finished_at_ms: now + 4,
            trace_id: "trace-alpha-finish".to_string(),
        })
        .await
        .expect("finish alpha execution");
    let replay = storage
        .commit_execution_delta(DeltaCommit {
            execution_id: alpha_execution_id,
            expected_version: 1,
            event_id: delta_event_id,
            trace_id: "trace-alpha-delta".to_string(),
            occurred_at_ms: now + 3,
            delta,
        })
        .await
        .expect("terminal replay returns original receipt");
    assert_eq!(replay, first);

    let beta = candidate_for_source(now + 5, "source:beta");
    let beta_grant = beta.required_grant.clone();
    install_draft(&storage, beta, 0, beta_grant, now + 6).await;
    let beta_execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id: beta_execution_id,
            source_identity: "source:beta".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-beta-start".to_string(),
            started_at_ms: now + 7,
            correlation_id: None,
        })
        .await
        .expect("start beta execution");
    let cross_source = storage
        .commit_execution_delta(DeltaCommit {
            execution_id: beta_execution_id,
            expected_version: 1,
            event_id: Uuid::new_v4(),
            trace_id: "trace-cross-source-upsert".to_string(),
            occurred_at_ms: now + 8,
            delta: delta_with_item("source:beta", "must not steal alpha item"),
        })
        .await;
    assert!(matches!(cross_source, Err(StorageError::InvalidInput(_))));
    let cross_source_tombstone = storage
        .commit_execution_delta(DeltaCommit {
            execution_id: beta_execution_id,
            expected_version: 1,
            event_id: Uuid::new_v4(),
            trace_id: "trace-cross-source-tombstone".to_string(),
            occurred_at_ms: now + 9,
            delta: ProjectionDelta {
                upserts: lj_media::MediaGraphDelta::default(),
                tombstones: ProjectionTombstones {
                    items: vec![lj_media::MediaResourceId("item:test:1".to_string())],
                    ..ProjectionTombstones::default()
                },
            },
        })
        .await;
    assert!(matches!(
        cross_source_tombstone,
        Err(StorageError::InvalidInput(_))
    ));
    assert_eq!(
        storage
            .get_item(lj_media::MediaResourceId("item:test:1".to_string()))
            .await
            .expect("read alpha-owned item")
            .expect("alpha item remains")
            .title,
        "owned by alpha"
    );
    storage.shutdown().await.expect("writer shutdown");
}

#[tokio::test]
async fn append_event_retries_require_matching_artifact_refs_and_integrity() {
    let temp = TempStore::new("append-idempotency-artifact");
    let storage = temp.open().await;
    let event_id = Uuid::new_v4();
    let request = |bytes: &[u8]| AppendRequest {
        stream_id: "append/idempotency".to_string(),
        expected_version: 0,
        event_id,
        event_type: lj_rule_model::EventType::Other("artifact-idempotency".to_string()),
        schema_version: 1,
        correlation_id: None,
        causation_id: None,
        trace_id: "trace-append-idempotency".to_string(),
        occurred_at_ms: 1_750_001_400_000,
        payload: serde_json::json!({"kind": "artifact-idempotency"}),
        source_id: None,
        artifacts: vec![ArtifactInput {
            bytes: bytes.to_vec(),
        }],
    };
    storage
        .append_event(request(b"first artifact"))
        .await
        .expect("first append");
    assert!(matches!(
        storage.append_event(request(b"different artifact")).await,
        Err(StorageError::IdempotencyMismatch)
    ));
    storage.shutdown().await.expect("writer shutdown");
}
