//! Projection checkpoint、query、retention 与 D12 合同。

use super::*;

#[tokio::test]
async fn source_and_library_checkpoints_rebuild_from_subsequent_events() {
    let temp = TempStore::new("checkpoint");
    let storage = temp.open().await;
    let now = 1_750_000_400_000;
    install_source(&storage, now).await;
    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-checkpoint".to_string(),
            started_at_ms: now + 1,
            correlation_id: None,
        })
        .await
        .expect("execution start");
    storage
        .commit_execution_delta(DeltaCommit {
            execution_id,
            expected_version: 1,
            event_id: Uuid::new_v4(),
            trace_id: "trace-checkpoint-first".to_string(),
            occurred_at_ms: now + 2,
            delta: delta_with_item("source:test", "checkpoint 之前"),
        })
        .await
        .expect("first Delta");
    let checkpoint = storage
        .checkpoint_source("source:test", now + 3)
        .await
        .expect("source checkpoint");
    let saved = storage
        .load_source_checkpoint("source:test")
        .await
        .expect("read source checkpoint")
        .expect("source checkpoint exists");
    assert_eq!(saved.global_seq, checkpoint.global_seq);
    assert_eq!(saved.delta.items[0].title, "checkpoint 之前");

    storage
        .commit_execution_delta(DeltaCommit {
            execution_id,
            expected_version: 2,
            event_id: Uuid::new_v4(),
            trace_id: "trace-checkpoint-second".to_string(),
            occurred_at_ms: now + 4,
            delta: delta_with_item("source:test", "checkpoint 之后"),
        })
        .await
        .expect("second Delta");
    let rebuilt = storage
        .recover_source_from_checkpoint("source:test")
        .await
        .expect("checkpoint plus source events rebuild");
    assert_eq!(rebuilt.delta.items[0].title, "checkpoint 之后");
    assert_eq!(
        storage
            .source_events_after("source:test", checkpoint.global_seq)
            .await
            .expect("source catch-up")
            .len(),
        1
    );

    storage
        .update_library(LibraryUpdate {
            entry: LibraryEntry {
                resource_id: MediaResourceId("item:test:1".to_string()),
                favorite: true,
                pinned: false,
                last_opened_at: Some("2026-07-18T00:00:00Z".to_string()),
                progress: None,
            },
            expected_version: 0,
            event_id: Uuid::new_v4(),
            occurred_at_ms: now + 5,
            trace_id: "trace-library".to_string(),
        })
        .await
        .expect("library event and projection");
    storage
        .checkpoint_library(now + 6)
        .await
        .expect("library checkpoint");
    assert!(
        storage
            .load_library_checkpoint()
            .await
            .expect("read library checkpoint")
            .expect("library checkpoint exists")
            .entries[0]
            .favorite
    );
    storage.shutdown().await.expect("writer shutdown");
}

#[tokio::test]
async fn safe_source_and_library_projection_queries_are_ordered_and_revisioned() {
    let temp = TempStore::new("safe-projection-queries");
    let storage = temp.open().await;
    let now = 1_750_000_450_000;

    let alpha = candidate_for_source(now, "source:alpha");
    let alpha_grant = alpha.required_grant.clone();
    install_draft(&storage, alpha, 0, alpha_grant, now + 1).await;
    let mut beta = candidate_for_source(now + 2, "source:beta");
    beta.required_grant.network = true;
    let beta_grant = beta.required_grant.clone();
    install_draft(&storage, beta, 0, beta_grant, now + 3).await;

    let sources = storage
        .list_installed_sources()
        .await
        .expect("read safe installed-source projection");
    assert_eq!(
        sources
            .iter()
            .map(|source| source.source_identity.as_str())
            .collect::<Vec<_>>(),
        vec!["source:alpha", "source:beta"]
    );
    assert!(sources.iter().all(|source| {
        source.profile.id.0 == source.source_identity
            && source.revision == 1
            && !source.version.is_empty()
    }));
    assert!(!sources[0].grant.network);
    assert!(sources[1].grant.network);

    let empty_library = storage
        .get_library_projection()
        .await
        .expect("read empty user-owned library projection");
    assert!(empty_library.entries.is_empty());
    storage
        .update_library(LibraryUpdate {
            entry: LibraryEntry {
                resource_id: MediaResourceId("item:library:alpha".to_string()),
                favorite: true,
                pinned: false,
                last_opened_at: Some("2026-07-18T01:00:00Z".to_string()),
                progress: Some(LibraryProgress {
                    unit_id: Some(MediaResourceId("unit:library:alpha".to_string())),
                    position: 3,
                    total: Some(10),
                }),
            },
            expected_version: 0,
            event_id: Uuid::new_v4(),
            occurred_at_ms: now + 4,
            trace_id: "trace-library-alpha".to_string(),
        })
        .await
        .expect("write first library entry");
    storage
        .update_library(LibraryUpdate {
            entry: LibraryEntry {
                resource_id: MediaResourceId("item:library:beta".to_string()),
                favorite: false,
                pinned: true,
                last_opened_at: None,
                progress: None,
            },
            expected_version: 0,
            event_id: Uuid::new_v4(),
            occurred_at_ms: now + 5,
            trace_id: "trace-library-beta".to_string(),
        })
        .await
        .expect("write second library entry");
    let library = storage
        .get_library_projection()
        .await
        .expect("read complete library projection");
    assert!(library.global_seq > empty_library.global_seq);
    assert_eq!(
        library
            .entries
            .iter()
            .map(|entry| entry.resource_id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["item:library:alpha", "item:library:beta"]
    );
    assert_eq!(library.entries[0].revision, 1);
    assert_eq!(library.entries[1].revision, 1);
    assert!(
        library
            .entries
            .iter()
            .all(|entry| entry.updated_global_seq <= library.global_seq)
    );
    assert_eq!(
        library.entries[0]
            .progress
            .as_ref()
            .and_then(|progress| progress.unit_id.as_ref())
            .map(|unit_id| unit_id.0.as_str()),
        Some("unit:library:alpha")
    );

    storage
        .update_library(LibraryUpdate {
            entry: LibraryEntry {
                resource_id: library.entries[0].resource_id.clone(),
                favorite: false,
                pinned: true,
                last_opened_at: Some("2026-07-18T02:00:00Z".to_string()),
                progress: Some(LibraryProgress {
                    unit_id: Some(MediaResourceId("unit:library:alpha".to_string())),
                    position: 9,
                    total: Some(10),
                }),
            },
            expected_version: library.entries[0].revision,
            event_id: Uuid::new_v4(),
            occurred_at_ms: now + 6,
            trace_id: "trace-library-alpha-update".to_string(),
        })
        .await
        .expect("reuse projection revision for optimistic update");
    let updated_library = storage
        .get_library_projection()
        .await
        .expect("read revised library projection");
    assert!(updated_library.global_seq > library.global_seq);
    assert_eq!(updated_library.entries[0].revision, 2);
    assert!(!updated_library.entries[0].favorite);
    assert!(updated_library.entries[0].pinned);
    assert_eq!(updated_library.entries[1].revision, 1);
    assert_eq!(
        updated_library.entries[0]
            .progress
            .as_ref()
            .map(|progress| progress.position),
        Some(9)
    );
    storage
        .shutdown()
        .await
        .expect("close projection query storage");
}

#[tokio::test]
async fn installed_source_listing_rejects_cross_source_profile_ownership() {
    let temp = TempStore::new("installed-source-ownership");
    let storage = temp.open().await;
    let now = 1_750_000_475_000;
    let draft = candidate(now);
    let grant = draft.required_grant.clone();
    install_draft(&storage, draft, 0, grant, now + 1).await;
    storage
        .shutdown()
        .await
        .expect("close before corrupting source projection");

    let database_url = temp.config.database_path.to_string_lossy().into_owned();
    let mut conn = SqliteConnection::establish(&database_url).expect("open real SQLite database");
    let mut foreign_profile = SourceProfile {
        id: MediaResourceId("source:foreign".to_string()),
        title: "foreign source".to_string(),
        icon_url: None,
        version: Some("v1".to_string()),
        supported_intents: Vec::new(),
        risk_notes: Vec::new(),
    };
    foreign_profile.title.push_str(" profile");
    let profile_json = serde_json::to_string(&foreign_profile).expect("serialize foreign profile");
    sql_query(
        "UPDATE source_projection SET profile_json = ? WHERE source_identity = 'source:test'",
    )
    .bind::<diesel::sql_types::Text, _>(profile_json)
    .execute(&mut conn)
    .expect("simulate cross-source profile corruption");
    drop(conn);

    let storage = temp.open().await;
    assert!(matches!(
        storage.list_installed_sources().await,
        Err(StorageError::InvalidInput(_))
    ));
    storage.shutdown().await.expect("close ownership storage");
}

#[tokio::test]
async fn policy_gc_expires_candidates_and_honors_pins() {
    let temp = TempStore::new("gc");
    let storage = temp.open().await;
    let now = 1_750_000_500_000;

    let expired = candidate(now - DEFAULT_CANDIDATE_TTL_MS - 1);
    let expired_id = expired.candidate_id;
    storage
        .stage_candidate(expired)
        .await
        .expect("stage expiring candidate");
    let candidate_report = storage
        .run_gc(
            RetentionPolicy {
                quota_bytes: u64::MAX,
                archive_ttl_ms: None,
            },
            now,
        )
        .await
        .expect("candidate retention scan");
    assert_eq!(candidate_report.expired_candidates, 1);
    assert!(matches!(
        storage
            .install_candidate(InstallCandidateRequest {
                candidate_id: expired_id,
                grant: PolicyCapabilities::default(),
                expected_source_version: 0,
                event_id: Uuid::new_v4(),
                trace_id: "trace-expired".to_string(),
                occurred_at_ms: now,
                correlation_id: None,
                source_credentials: None,
            })
            .await,
        Err(StorageError::CandidateExpired)
    ));

    install_source(&storage, now).await;
    let unpinned = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id: unpinned,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-unpinned".to_string(),
            started_at_ms: now - 100,
            correlation_id: None,
        })
        .await
        .expect("start unpinned execution");
    storage
        .finish_execution(ExecutionFinish {
            execution_id: unpinned,
            expected_version: 1,
            event_id: Uuid::new_v4(),
            status: ExecutionStatus::Completed,
            finished_at_ms: now - 99,
            trace_id: "trace-unpinned-finish".to_string(),
        })
        .await
        .expect("finish unpinned execution");

    let pinned = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id: pinned,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-pinned".to_string(),
            started_at_ms: now - 100,
            correlation_id: None,
        })
        .await
        .expect("start pinned execution");
    storage
        .finish_execution(ExecutionFinish {
            execution_id: pinned,
            expected_version: 1,
            event_id: Uuid::new_v4(),
            status: ExecutionStatus::Completed,
            finished_at_ms: now - 99,
            trace_id: "trace-pinned-finish".to_string(),
        })
        .await
        .expect("finish pinned execution");
    storage
        .set_execution_pin(ExecutionPin {
            execution_id: pinned,
            expected_version: 2,
            event_id: Uuid::new_v4(),
            pinned: true,
            occurred_at_ms: now - 98,
            trace_id: "trace-pin".to_string(),
        })
        .await
        .expect("pin execution");

    let ttl_report = storage
        .run_gc(
            RetentionPolicy {
                quota_bytes: u64::MAX,
                archive_ttl_ms: Some(1),
            },
            now,
        )
        .await
        .expect("TTL retention GC");
    assert_eq!(ttl_report.finalized, 1);
    let unpinned_record = storage
        .get_execution(unpinned)
        .await
        .expect("unpinned summary")
        .expect("summary retained after GC");
    assert_eq!(unpinned_record.gc_state, GcState::Finalized);
    assert!(!unpinned_record.replayable);
    let pinned_record = storage
        .get_execution(pinned)
        .await
        .expect("pinned summary")
        .expect("pinned summary retained");
    assert_eq!(pinned_record.gc_state, GcState::Active);
    assert!(pinned_record.replayable);
    storage.shutdown().await.expect("writer shutdown");
}

#[tokio::test]
async fn policy_gc_evicts_over_quota() {
    let temp = TempStore::new("gc-quota");
    let storage = temp.open().await;
    let now = 1_750_000_500_000;
    install_source(&storage, now).await;

    let quota_execution = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id: quota_execution,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-quota".to_string(),
            started_at_ms: now,
            correlation_id: None,
        })
        .await
        .expect("start quota execution");
    storage
        .finish_execution(ExecutionFinish {
            execution_id: quota_execution,
            expected_version: 1,
            event_id: Uuid::new_v4(),
            status: ExecutionStatus::Completed,
            finished_at_ms: now,
            trace_id: "trace-quota-finish".to_string(),
        })
        .await
        .expect("finish quota execution");
    let quota_report = storage
        .run_gc(
            RetentionPolicy {
                quota_bytes: 1,
                archive_ttl_ms: None,
            },
            now + 1,
        )
        .await
        .expect("quota retention GC");
    assert_eq!(quota_report.finalized, 1);
    assert_eq!(
        storage
            .get_execution(quota_execution)
            .await
            .expect("quota summary")
            .expect("quota execution summary")
            .gc_state,
        GcState::Finalized
    );
    storage.shutdown().await.expect("writer shutdown");
}

#[tokio::test]
#[ignore = "D12 performance calibration; run with cargo test --release on target hardware"]
async fn d12_thousand_resource_event_projection_transaction_gate() {
    const SAMPLES: usize = 20;
    const RESOURCE_COUNT: usize = 1_000;
    const P95_LIMIT: std::time::Duration = std::time::Duration::from_millis(25);

    if cfg!(debug_assertions) {
        eprintln!("D12 timing gate requires a release build");
        return;
    }
    let temp = TempStore::new("d12-transaction");
    let storage = temp.open().await;
    let now = 1_750_000_600_000;
    install_source(&storage, now).await;
    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-d12-start".to_string(),
            started_at_ms: now + 1,
            correlation_id: None,
        })
        .await
        .expect("start D12 execution");

    let delta = ProjectionDelta {
        upserts: MediaGraphDelta {
            items: (0..RESOURCE_COUNT)
                .map(|index| {
                    let mut resource = item("source:test", &format!("D12 item {index}"));
                    resource.id = MediaResourceId(format!("item:d12:{index}"));
                    resource
                })
                .collect(),
            ..MediaGraphDelta::default()
        },
        tombstones: ProjectionTombstones::default(),
    };
    let mut elapsed = Vec::with_capacity(SAMPLES);
    for index in 0..SAMPLES {
        let request = DeltaCommit {
            execution_id,
            expected_version: u64::try_from(index + 1).expect("sample revision"),
            event_id: Uuid::new_v4(),
            trace_id: format!("trace-d12-{index}"),
            occurred_at_ms: now + 2 + i64::try_from(index).expect("sample timestamp"),
            delta: delta.clone(),
        };
        let started = std::time::Instant::now();
        storage
            .commit_execution_delta(request)
            .await
            .expect("commit D12 projection transaction");
        elapsed.push(started.elapsed());
    }
    elapsed.sort_unstable();
    let p95 = elapsed[(SAMPLES * 95).div_ceil(100) - 1];
    assert!(
        p95 <= P95_LIMIT,
        "D12 1,000-resource event+projection p95 was {p95:?}, limit is {P95_LIMIT:?}"
    );
    storage.shutdown().await.expect("writer shutdown");
}
