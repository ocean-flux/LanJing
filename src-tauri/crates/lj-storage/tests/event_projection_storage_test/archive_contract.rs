//! Event projection 原子性与 effect archive 安全合同。

use super::*;

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
            .list_units_for_item(MediaResourceId("item:test:1".to_string()))
            .await
            .expect("item unit index")
            .len(),
        1
    );
    assert_eq!(
        storage
            .list_assets_for_unit(MediaResourceId("unit:test:1".to_string()))
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
        node_id,
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
            node_id,
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
    let restarted = temp.open().await;
    assert!(
        EffectArchive::load_replay(
            &restarted,
            EffectReplayLookup {
                archived_execution_id: execution_id,
                node_id,
                kind: EffectKind::Http,
            },
        )
        .await
        .is_err()
    );
    restarted.shutdown().await.expect("writer shutdown");
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
        node_id,
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
        event.envelope.payload["request_body_artifact_hash"].as_str(),
        Some(request_body_hash.as_str())
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
        event
            .envelope
            .secret_refs
            .iter()
            .any(|secret| secret.hash == request_body_hash && secret.algorithm == "aes-256-gcm"),
        "event must retain only an encrypted Secret Artifact ref"
    );

    let replay = EffectArchive::load_replay(
        &storage,
        EffectReplayLookup {
            archived_execution_id: execution_id,
            node_id,
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
    let database_url = temp.config.database_path.to_string_lossy().into_owned();
    let mut conn = SqliteConnection::establish(&database_url).expect("open real SQLite database");
    let artifact = sql_query(
        "SELECT artifact_kind, encryption, relative_path FROM artifact_metadata WHERE hash = ? AND artifact_kind = 'secret'",
    )
    .bind::<diesel::sql_types::Text, _>(&request_body_hash)
    .get_result::<ArtifactSecurityTestRow>(&mut conn)
    .expect("request body secret artifact metadata");
    assert_eq!(artifact.artifact_kind, "secret");
    assert_eq!(artifact.encryption.as_deref(), Some("aes-256-gcm"));
    assert!(artifact.relative_path.ends_with(".secret"));
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
        Uuid::new_v4(),
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
        node_id,
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
            node_id,
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
        node_id,
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

    let database_url = temp.config.database_path.to_string_lossy().into_owned();
    let mut conn = SqliteConnection::establish(&database_url).expect("open real SQLite database");
    let witness_artifact = sql_query(
        "SELECT artifact_metadata.relative_path, artifact_metadata.ref_count FROM effect_captures INNER JOIN artifact_metadata ON artifact_metadata.hash = effect_captures.witness_artifact_hash AND artifact_metadata.artifact_kind = 'body' WHERE effect_captures.execution_id = ? AND effect_captures.effect_id = ?",
    )
    .bind::<diesel::sql_types::Text, _>(execution_id.to_string())
    .bind::<diesel::sql_types::Text, _>(effect_id.to_string())
    .get_result::<ArtifactMetadataTestRow>(&mut conn)
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
                node_id,
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
                node_id,
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
                node_id,
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
