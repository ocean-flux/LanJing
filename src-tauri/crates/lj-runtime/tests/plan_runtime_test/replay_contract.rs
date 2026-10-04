//! Plan 编译校验、live/replay 一致性与归档篡改合同。

use super::*;

#[test]
fn compiler_produced_plan_passes_runtime_hash_validation() {
    let compiler = Compiler::with_version("runtime-test-compiler@1".to_string());
    let plan = compiler
        .compile(&compiler_definition())
        .expect("valid Definition must compile");
    assert_eq!(runtime(4).check_plan_support(&plan), PlanSupport::Linear);
    runtime(4)
        .validate_plan(&plan)
        .expect("runtime must accept compiler canonical Plan hash");
}

#[test]
fn runtime_rejects_tampered_plan_hash_and_compiler_identity() {
    let runtime = runtime(4);
    let hash_mismatch = rewrite_plan(
        &sample_plan(),
        |value| value["plan_hash"] = serde_json::json!("tampered"),
        false,
    );
    assert!(matches!(
        runtime.validate_plan(&hash_mismatch),
        Err(lj_runtime::PlanRuntimeError::PlanHashMismatch)
    ));

    let compiler_mismatch = rewrite_plan(
        &sample_plan(),
        |value| value["compiler_version"] = serde_json::json!("other-compiler@1"),
        true,
    );
    assert!(matches!(
        runtime.validate_plan(&compiler_mismatch),
        Err(lj_runtime::PlanRuntimeError::CompilerVersionMismatch)
    ));
}

#[test]
fn runtime_rejects_a_plan_sealed_against_a_different_descriptor_set() {
    // reseal: Plan 自身 hash 自洽（等于另一个 descriptor 表 build 出的 Plan），唯一差异是
    // 封存 Plan 时那张节点声明表，因此必须命中 descriptor 不匹配而不是 hash 不匹配。
    let drift = rewrite_plan(
        &sample_plan(),
        |value| value["descriptor_digest"] = serde_json::json!("0".repeat(64)),
        true,
    );
    assert!(matches!(
        runtime(4).validate_plan(&drift),
        Err(lj_runtime::PlanRuntimeError::DescriptorDigestMismatch)
    ));
}

#[test]
fn control_support_classification_still_requires_a_valid_program() {
    let plan = control_plan();
    let runtime = runtime(4);
    assert_eq!(runtime.check_plan_support(&plan), PlanSupport::ControlFlow);
    assert!(matches!(
        runtime.validate_plan(&plan),
        Err(lj_runtime::PlanRuntimeError::InvalidPlan(_))
    ));
}

#[tokio::test]
async fn live_and_replay_preserve_typed_outputs_without_live_fallback() {
    let runtime = runtime(8);
    let archive = Arc::new(DurableFileArchive::new());
    let http_calls = Arc::new(AtomicUsize::new(0));
    let extract_calls = Arc::new(AtomicUsize::new(0));
    let live_execution = Uuid::new_v4();
    let live_events = collect_events(
        runtime
            .execute(
                request(
                    sample_plan(),
                    live_execution,
                    lj_runtime::ExecutionMode::Live,
                ),
                handlers(
                    FixtureHttp::success(http_calls.clone()),
                    extract_calls.clone(),
                ),
                archive.clone(),
            )
            .expect("live session"),
    )
    .await;

    assert_eq!(
        terminal_count(&live_events),
        1,
        "每个 execution 只能有一个终态"
    );
    assert!(matches!(
        live_events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Completed)
    ));
    let captures = archive.captures();
    assert_eq!(
        captures.len(),
        2,
        "HTTP 与 Extract 都必须先 durable capture"
    );
    assert!(matches!(captures[0].output.as_ref(), EffectOutput::Http(_)));
    assert!(matches!(
        captures[1].output.as_ref(),
        EffectOutput::Extract(_)
    ));
    let live_hashes: Vec<String> = live_events
        .iter()
        .filter_map(|event| match &event.kind {
            lj_runtime::ExecutionEventKind::EffectCaptured { output_hash, .. } => {
                Some(output_hash.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(live_hashes.len(), 2);

    let replay_events = collect_events(
        runtime
            .execute(
                request_with_credentials(
                    sample_plan(),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Replay {
                        archived_execution_id: live_execution,
                    },
                    HttpExecutionCredentials::from_source_secret(
                        "replay-source-namespace".to_string(),
                        Some(b"not-a-json-header-map".to_vec()),
                    ),
                ),
                handlers(
                    FixtureHttp::success(http_calls.clone()),
                    extract_calls.clone(),
                ),
                archive,
            )
            .expect("replay session"),
    )
    .await;
    let replay_hashes: Vec<String> = replay_events
        .iter()
        .filter_map(|event| match &event.kind {
            lj_runtime::ExecutionEventKind::EffectReplayed { output_hash, .. } => {
                Some(output_hash.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        replay_hashes, live_hashes,
        "replay 必须返回同一类型化 capture 输出"
    );
    assert_eq!(
        http_calls.load(Ordering::SeqCst),
        1,
        "replay 不得调用 live HTTP"
    );
    assert_eq!(
        extract_calls.load(Ordering::SeqCst),
        1,
        "replay 不得调用 live Extract"
    );
    assert_eq!(terminal_count(&replay_events), 1);
    assert!(matches!(
        replay_events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Completed)
    ));
}

#[tokio::test]
async fn replay_missing_capture_fails_with_single_attributed_terminal() {
    let runtime = runtime(4);
    let archive = Arc::new(DurableFileArchive::new());
    let events = collect_events(
        runtime
            .execute(
                request(
                    sample_plan(),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Replay {
                        archived_execution_id: Uuid::new_v4(),
                    },
                ),
                handlers(
                    FixtureHttp::success(Arc::new(AtomicUsize::new(0))),
                    Arc::new(AtomicUsize::new(0)),
                ),
                archive,
            )
            .expect("replay session"),
    )
    .await;

    assert_eq!(terminal_count(&events), 1);
    let Some(lj_runtime::ExecutionEventKind::Failed { failure }) =
        events.last().map(|event| &event.kind)
    else {
        panic!("缺 capture 必须进入 Failed 终态");
    };
    assert_eq!(failure.code, RuntimeFailureCode::ReplayCaptureMissing);
    assert_eq!(failure.node_id, Some(Uuid::from_u128(101)));
    assert!(failure.effect_id.is_some());
    assert_eq!(failure.trace_id, "runtime-test-trace");
}

#[tokio::test]
async fn replay_output_hash_mismatch_is_a_hard_failure() {
    let runtime = runtime(4);
    let archive = Arc::new(DurableFileArchive::new());
    let live_execution = Uuid::new_v4();
    let _ = collect_events(
        runtime
            .execute(
                request(
                    sample_plan(),
                    live_execution,
                    lj_runtime::ExecutionMode::Live,
                ),
                handlers(
                    FixtureHttp::success(Arc::new(AtomicUsize::new(0))),
                    Arc::new(AtomicUsize::new(0)),
                ),
                archive.clone(),
            )
            .expect("live session"),
    )
    .await;
    archive.corrupt_first_output_hash();

    let events = collect_events(
        runtime
            .execute(
                request(
                    sample_plan(),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Replay {
                        archived_execution_id: live_execution,
                    },
                ),
                handlers(
                    FixtureHttp::success(Arc::new(AtomicUsize::new(0))),
                    Arc::new(AtomicUsize::new(0)),
                ),
                archive,
            )
            .expect("replay session"),
    )
    .await;

    let Some(lj_runtime::ExecutionEventKind::Failed { failure }) =
        events.last().map(|event| &event.kind)
    else {
        panic!("hash mismatch 必须进入 Failed 终态");
    };
    assert_eq!(failure.code, RuntimeFailureCode::ReplayOutputHashMismatch);
    assert_eq!(terminal_count(&events), 1);
}

#[tokio::test]
async fn replay_fingerprint_mismatch_is_a_hard_failure() {
    let runtime = runtime(4);
    let archive = Arc::new(DurableFileArchive::new());
    let live_execution = Uuid::new_v4();
    let _ = collect_events(
        runtime
            .execute(
                request(
                    sample_plan(),
                    live_execution,
                    lj_runtime::ExecutionMode::Live,
                ),
                handlers(
                    FixtureHttp::success(Arc::new(AtomicUsize::new(0))),
                    Arc::new(AtomicUsize::new(0)),
                ),
                archive.clone(),
            )
            .expect("live session"),
    )
    .await;
    archive.corrupt_first_fingerprint();

    let events = collect_events(
        runtime
            .execute(
                request(
                    sample_plan(),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Replay {
                        archived_execution_id: live_execution,
                    },
                ),
                handlers(
                    FixtureHttp::success(Arc::new(AtomicUsize::new(0))),
                    Arc::new(AtomicUsize::new(0)),
                ),
                archive,
            )
            .expect("replay session"),
    )
    .await;

    let Some(lj_runtime::ExecutionEventKind::Failed { failure }) =
        events.last().map(|event| &event.kind)
    else {
        panic!("fingerprint mismatch 必须进入 Failed 终态");
    };
    assert_eq!(failure.code, RuntimeFailureCode::ReplayFingerprintMismatch);
    assert_eq!(terminal_count(&events), 1);
}

#[tokio::test]
async fn replay_rejects_tampered_quickjs_script_input_and_output_witness_hashes() {
    for field in [
        QuickJsWitnessHashField::Script,
        QuickJsWitnessHashField::Input,
        QuickJsWitnessHashField::Output,
    ] {
        let runtime = runtime(4);
        let archive = Arc::new(DurableFileArchive::new());
        let live_execution = Uuid::new_v4();
        let live_events = collect_events(
            runtime
                .execute(
                    request(
                        quickjs_plan(),
                        live_execution,
                        lj_runtime::ExecutionMode::Live,
                    ),
                    handlers(
                        FixtureHttp::success(Arc::new(AtomicUsize::new(0))),
                        Arc::new(AtomicUsize::new(0)),
                    ),
                    archive.clone(),
                )
                .expect("QuickJS live session"),
        )
        .await;
        assert!(matches!(
            live_events.last().map(|event| &event.kind),
            Some(lj_runtime::ExecutionEventKind::Completed)
        ));
        archive.corrupt_first_quickjs_witness_hash(field);

        let events = collect_events(
            runtime
                .execute(
                    request(
                        quickjs_plan(),
                        Uuid::new_v4(),
                        lj_runtime::ExecutionMode::Replay {
                            archived_execution_id: live_execution,
                        },
                    ),
                    handlers(
                        FixtureHttp::success(Arc::new(AtomicUsize::new(0))),
                        Arc::new(AtomicUsize::new(0)),
                    ),
                    archive,
                )
                .expect("QuickJS replay session"),
        )
        .await;
        let Some(lj_runtime::ExecutionEventKind::Failed { failure }) =
            events.last().map(|event| &event.kind)
        else {
            panic!("tampered QuickJS witness 必须进入 Failed 终态");
        };
        assert_eq!(failure.code, RuntimeFailureCode::ReplayWitnessMismatch);
        assert_eq!(terminal_count(&events), 1);
    }
}

#[tokio::test]
async fn replay_rejects_tampered_extract_input_witness_hash() {
    let runtime = runtime(4);
    let archive = Arc::new(DurableFileArchive::new());
    let http_calls = Arc::new(AtomicUsize::new(0));
    let extract_calls = Arc::new(AtomicUsize::new(0));
    let live_execution = Uuid::new_v4();
    let _ = collect_events(
        runtime
            .execute(
                request(
                    sample_plan(),
                    live_execution,
                    lj_runtime::ExecutionMode::Live,
                ),
                handlers(
                    FixtureHttp::success(http_calls.clone()),
                    extract_calls.clone(),
                ),
                archive.clone(),
            )
            .expect("Extract live session"),
    )
    .await;
    archive.corrupt_first_extract_witness_input_hash();

    let events = collect_events(
        runtime
            .execute(
                request(
                    sample_plan(),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Replay {
                        archived_execution_id: live_execution,
                    },
                ),
                handlers(
                    FixtureHttp::success(http_calls.clone()),
                    extract_calls.clone(),
                ),
                archive,
            )
            .expect("Extract replay session"),
    )
    .await;

    assert_eq!(http_calls.load(Ordering::SeqCst), 1);
    assert_eq!(extract_calls.load(Ordering::SeqCst), 1);
    let Some(lj_runtime::ExecutionEventKind::Failed { failure }) =
        events.last().map(|event| &event.kind)
    else {
        panic!("tampered Extract witness 必须进入 Failed 终态");
    };
    assert_eq!(failure.code, RuntimeFailureCode::ReplayWitnessMismatch);
    assert_eq!(terminal_count(&events), 1);
}

#[tokio::test]
async fn replay_preserves_typed_http_failure_without_live_fallback() {
    let runtime = runtime(4);
    let archive = Arc::new(DurableFileArchive::new());
    let http_calls = Arc::new(AtomicUsize::new(0));
    let live_execution = Uuid::new_v4();
    let live_events = collect_events(
        runtime
            .execute(
                request(
                    sample_plan(),
                    live_execution,
                    lj_runtime::ExecutionMode::Live,
                ),
                handlers(
                    FixtureHttp::failure(http_calls.clone()),
                    Arc::new(AtomicUsize::new(0)),
                ),
                archive.clone(),
            )
            .expect("failed HTTP live session"),
    )
    .await;
    assert!(matches!(
        archive
            .captures()
            .first()
            .map(|capture| capture.output.as_ref()),
        Some(EffectOutput::Failure(EffectFailure::Http { .. }))
    ));
    assert!(matches!(
        live_events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Failed { failure })
            if failure.code == RuntimeFailureCode::EffectFailed
    ));

    let replay_events = collect_events(
        runtime
            .execute(
                request(
                    sample_plan(),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Replay {
                        archived_execution_id: live_execution,
                    },
                ),
                handlers(
                    FixtureHttp::success(http_calls.clone()),
                    Arc::new(AtomicUsize::new(0)),
                ),
                archive,
            )
            .expect("failed HTTP replay session"),
    )
    .await;
    assert_eq!(http_calls.load(Ordering::SeqCst), 1);
    assert!(replay_events.iter().any(|event| matches!(
        event.kind,
        lj_runtime::ExecutionEventKind::EffectReplayed { .. }
    )));
    assert!(matches!(
        replay_events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Failed { failure })
            if failure.code == RuntimeFailureCode::EffectFailed
    ));
    assert_eq!(terminal_count(&replay_events), 1);
}
