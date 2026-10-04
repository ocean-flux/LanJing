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

/// P4 覆盖的两类受控 effect: 网络 (来源规则 Http + Extract) 与脚本 (受控 `QuickJS`)。
///
/// 三类 effect 共用同一条 replay 校验路径, 因此新增类别用参数化覆盖, 而不是复制用例。
fn network_and_script_plans() -> [(&'static str, ExecutionPlan, Uuid); 2] {
    [
        ("网络与提取", sample_plan(), Uuid::from_u128(101)),
        ("受控脚本", quickjs_plan(), Uuid::from_u128(1)),
    ]
}

/// (标签, Plan, Search 意图路径上真正执行的受控 effect 类别及其顺序)。
fn capture_scenarios() -> [(&'static str, ExecutionPlan, Vec<EffectKind>); 2] {
    [
        (
            "网络与提取",
            sample_plan(),
            vec![EffectKind::Http, EffectKind::Extract],
        ),
        ("受控脚本", quickjs_plan(), vec![EffectKind::QuickJs]),
    ]
}

/// 一次 live 运行后断言每个实际跑过的受控 effect 都进了 durable archive, 不多不少。
async fn assert_live_capture_matches_execution(
    label: &str,
    plan: ExecutionPlan,
    expected_kinds: &[EffectKind],
) {
    let runtime = runtime(8);
    let archive = Arc::new(DurableFileArchive::new());
    let calls = LiveCalls::new();
    let events = collect_events(
        runtime
            .execute(
                request(plan, Uuid::new_v4(), lj_runtime::ExecutionMode::Live),
                calls.registry(),
                archive.clone(),
            )
            .expect("live session"),
    )
    .await;
    assert!(
        matches!(
            events.last().map(|event| &event.kind),
            Some(lj_runtime::ExecutionEventKind::Completed)
        ),
        "{label} live 必须先正常完成: {events:?}"
    );

    let event_kinds: Vec<EffectKind> = events
        .iter()
        .filter_map(|event| match &event.kind {
            lj_runtime::ExecutionEventKind::EffectCaptured { kind, .. } => Some(kind.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        event_kinds, expected_kinds,
        "{label}: 每个实际执行的外部 effect 都必须在交付前 capture"
    );

    let captures = archive.captures();
    assert_eq!(
        captures
            .iter()
            .map(|capture| capture.kind.clone())
            .collect::<Vec<_>>(),
        expected_kinds,
        "{label}: durable archive 必须与交付事件一一对应"
    );
    let expected_count = expected_kinds.len();
    assert_eq!(
        calls.total(),
        expected_count,
        "{label}: capture 数必须等于实际执行数, 否则存在静默漏 capture"
    );
    for (kind, actual) in [
        (EffectKind::Http, calls.http.load(Ordering::SeqCst)),
        (EffectKind::QuickJs, calls.quickjs.load(Ordering::SeqCst)),
        (EffectKind::Extract, calls.extract.load(Ordering::SeqCst)),
    ] {
        assert_eq!(
            actual,
            expected_kinds.iter().filter(|item| **item == kind).count(),
            "{label}: {kind:?} 的 live 调用数与 capture 数不一致"
        );
    }
    let mut ordinals: Vec<u64> = captures
        .iter()
        .map(|capture| capture.invocation_path.ordinal())
        .collect();
    ordinals.sort_unstable();
    assert_eq!(
        ordinals,
        (1..=u64::try_from(expected_count).expect("用例 capture 数可转换为 u64"))
            .collect::<Vec<_>>(),
        "{label}: invocation ordinal 必须无洞"
    );
}

#[tokio::test]
async fn live_run_captures_every_network_and_script_effect_it_ran() {
    for (label, plan, expected_kinds) in capture_scenarios() {
        assert_live_capture_matches_execution(label, plan, &expected_kinds).await;
    }
}

#[tokio::test]
async fn replay_missing_capture_fails_with_single_attributed_terminal() {
    for (label, plan, entry_node) in network_and_script_plans() {
        let runtime = runtime(4);
        let archive = Arc::new(DurableFileArchive::new());
        let calls = LiveCalls::new();
        let events = collect_events(
            runtime
                .execute(
                    request(
                        plan,
                        Uuid::new_v4(),
                        lj_runtime::ExecutionMode::Replay {
                            archived_execution_id: Uuid::new_v4(),
                        },
                    ),
                    calls.registry(),
                    archive,
                )
                .expect("replay session"),
        )
        .await;

        assert_eq!(terminal_count(&events), 1, "{label} 只能有一个终态");
        let Some(lj_runtime::ExecutionEventKind::Failed { failure }) =
            events.last().map(|event| &event.kind)
        else {
            panic!("{label} 缺 capture 必须进入 Failed 终态");
        };
        assert_eq!(failure.code, RuntimeFailureCode::ReplayCaptureMissing);
        assert_eq!(failure.node_id, Some(entry_node));
        assert!(failure.effect_id.is_some());
        assert_eq!(failure.trace_id, "runtime-test-trace");
        assert_eq!(
            calls.total(),
            0,
            "{label}: 缺 capture 必须硬失败, 绝不能回退到 live handler"
        );
    }
}

#[tokio::test]
async fn replay_rejects_tampered_historical_output_payload_for_network_and_script_effects() {
    for (label, plan, entry_node) in network_and_script_plans() {
        let runtime = runtime(4);
        let archive = Arc::new(DurableFileArchive::new());
        let calls = LiveCalls::new();
        let live_execution = Uuid::new_v4();
        let live_events = collect_events(
            runtime
                .execute(
                    request(
                        plan.clone(),
                        live_execution,
                        lj_runtime::ExecutionMode::Live,
                    ),
                    calls.registry(),
                    archive.clone(),
                )
                .expect("live session"),
        )
        .await;
        assert!(
            matches!(
                live_events.last().map(|event| &event.kind),
                Some(lj_runtime::ExecutionEventKind::Completed)
            ),
            "{label} live 必须先正常完成"
        );
        let live_calls = calls.total();
        assert!(live_calls > 0, "{label} live 必须真的跑过受控 effect");
        // 只改历史输出内容, 不动任何 hash 字段。
        archive.tamper_first_output_payload();

        let events = collect_events(
            runtime
                .execute(
                    request(
                        plan,
                        Uuid::new_v4(),
                        lj_runtime::ExecutionMode::Replay {
                            archived_execution_id: live_execution,
                        },
                    ),
                    calls.registry(),
                    archive,
                )
                .expect("replay session"),
        )
        .await;

        assert_eq!(terminal_count(&events), 1, "{label} 只能有一个终态");
        let Some(lj_runtime::ExecutionEventKind::Failed { failure }) =
            events.last().map(|event| &event.kind)
        else {
            panic!("{label} 篡改的历史输出必须硬失败");
        };
        assert_eq!(
            failure.code,
            RuntimeFailureCode::ReplayOutputHashMismatch,
            "{label}: replay 必须从输出内容重算 hash, 而不是相信 archive 记的 hash"
        );
        assert_eq!(failure.node_id, Some(entry_node));
        assert_eq!(
            calls.total(),
            live_calls,
            "{label}: 校验失败不得回退到 live handler"
        );
    }
}

#[tokio::test]
async fn replay_rejects_mismatched_historical_input_for_network_and_script_effects() {
    for (label, plan, entry_node) in network_and_script_plans() {
        let runtime = runtime(4);
        let archive = Arc::new(DurableFileArchive::new());
        let calls = LiveCalls::new();
        let live_execution = Uuid::new_v4();
        let live_events = collect_events(
            runtime
                .execute(
                    request(
                        plan.clone(),
                        live_execution,
                        lj_runtime::ExecutionMode::Live,
                    ),
                    calls.registry(),
                    archive.clone(),
                )
                .expect("live session"),
        )
        .await;
        assert!(
            matches!(
                live_events.last().map(|event| &event.kind),
                Some(lj_runtime::ExecutionEventKind::Completed)
            ),
            "{label} live 必须先正常完成"
        );
        let live_calls = calls.total();

        // 同一份固定 archive, 但 replay 的输入已不是当初那次运行的历史输入。
        let mut replay_request = request(
            plan,
            Uuid::new_v4(),
            lj_runtime::ExecutionMode::Replay {
                archived_execution_id: live_execution,
            },
        );
        replay_request.input = IntentInput::Query("另一个输入".to_string());
        let events = collect_events(
            runtime
                .execute(replay_request, calls.registry(), archive)
                .expect("replay session"),
        )
        .await;

        assert_eq!(terminal_count(&events), 1, "{label} 只能有一个终态");
        let Some(lj_runtime::ExecutionEventKind::Failed { failure }) =
            events.last().map(|event| &event.kind)
        else {
            panic!("{label} 输入不匹配必须硬失败");
        };
        assert_eq!(
            failure.code,
            RuntimeFailureCode::ReplayFingerprintMismatch,
            "{label}: replay 必须把历史输入与当前输入绑定校验"
        );
        assert_eq!(failure.node_id, Some(entry_node));
        assert_eq!(
            calls.total(),
            live_calls,
            "{label}: 输入不匹配不得回退到 live handler"
        );
    }
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
