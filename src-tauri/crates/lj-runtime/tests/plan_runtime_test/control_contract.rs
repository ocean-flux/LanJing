//! structured Condition/Merge/Loop 的 live、replay、资源与失败合同。

use super::*;

fn typed_control_plan(
    max_iterations: u16,
    alpha_code: &str,
    strategy: MergeStrategy,
) -> ExecutionPlan {
    compiled_control_plan(
        typed_control_expression(),
        CollectionSelector::Typed {
            pointer: String::new(),
        },
        alpha_code,
        strategy,
        max_iterations,
    )
}

fn terminal_failure(
    events: &[lj_runtime::ExecutionEvent],
) -> Option<&lj_runtime::ExecutionFailure> {
    events.iter().find_map(|event| match &event.kind {
        lj_runtime::ExecutionEventKind::Failed { failure } => Some(failure),
        _ => None,
    })
}

fn control_trace_kinds(archive: &DurableFileArchive) -> Vec<ControlTrace> {
    archive
        .control_traces()
        .into_iter()
        .map(|capture| capture.trace)
        .collect()
}

#[tokio::test]
async fn typed_condition_merge_loop_runs_selected_branch_and_replays_exact_invocations() {
    let plan = typed_control_plan(4, "control_alpha", MergeStrategy::ConcatArrays);
    let runtime = runtime(32);
    assert_eq!(runtime.check_plan_support(&plan), PlanSupport::ControlFlow);
    runtime
        .validate_plan(&plan)
        .expect("compiled control Plan must pass runtime validation");
    let archive = Arc::new(DurableFileArchive::new());
    let live_execution = Uuid::new_v4();
    let quickjs_calls = Arc::new(AtomicUsize::new(0));
    let http_calls = Arc::new(AtomicUsize::new(0));
    let extract_calls = Arc::new(AtomicUsize::new(0));
    let observed_bindings = Arc::new(Mutex::new(Vec::new()));
    let live_events = collect_events(
        runtime
            .execute(
                request(
                    plan.clone(),
                    live_execution,
                    lj_runtime::ExecutionMode::Live,
                ),
                control_handlers(
                    FixtureHttp::observe_json(http_calls.clone(), observed_bindings.clone()),
                    quickjs_calls.clone(),
                    extract_calls.clone(),
                ),
                archive.clone(),
            )
            .expect("typed control live session"),
    )
    .await;
    assert!(matches!(
        live_events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Completed)
    ));
    assert_eq!(terminal_count(&live_events), 1);
    assert_eq!(quickjs_calls.load(Ordering::SeqCst), 2);
    assert_eq!(http_calls.load(Ordering::SeqCst), 2);
    assert_eq!(extract_calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        *observed_bindings
            .lock()
            .expect("Loop binding fixture mutex"),
        vec![
            serde_json::json!({
                "item": {
                    "enabled": true,
                    "title": "第一项",
                    "url": "https://example.invalid/1"
                },
                "index": 0
            }),
            serde_json::json!({
                "item": {
                    "enabled": true,
                    "title": "第二项",
                    "url": "https://example.invalid/2"
                },
                "index": 1
            }),
        ]
    );
    assert_eq!(
        control_trace_kinds(&archive),
        vec![
            ControlTrace::Condition {
                branch: "alpha".to_string(),
            },
            ControlTrace::Merge {
                active_inputs: vec!["alpha".to_string()],
            },
            ControlTrace::Loop { iteration_count: 2 },
        ]
    );
    let captures = archive.captures();
    assert_eq!(
        captures
            .iter()
            .map(|capture| capture.invocation_path.ordinal())
            .collect::<Vec<_>>(),
        [1, 3, 6, 7, 8, 9]
    );
    let http_paths = captures
        .iter()
        .filter(|capture| capture.invocation_path.node_id() == Uuid::from_u128(1_007))
        .map(|capture| capture.invocation_path.clone())
        .collect::<Vec<_>>();
    assert_eq!(http_paths.len(), 2);
    assert_eq!(http_paths[0].loop_iterations()[0].iteration_index, 0);
    assert_eq!(http_paths[1].loop_iterations()[0].iteration_index, 1);
    assert_ne!(http_paths[0], http_paths[1]);
    let mut all_ordinals = captures
        .iter()
        .map(|capture| capture.invocation_path.ordinal())
        .chain(
            archive
                .control_traces()
                .iter()
                .map(|capture| capture.invocation_path.ordinal()),
        )
        .collect::<Vec<_>>();
    all_ordinals.sort_unstable();
    assert_eq!(all_ordinals, (1..=9).collect::<Vec<_>>());
    let replay_quickjs_calls = Arc::new(AtomicUsize::new(0));
    let replay_http_calls = Arc::new(AtomicUsize::new(0));
    let replay_extract_calls = Arc::new(AtomicUsize::new(0));
    let replay_events = collect_events(
        runtime
            .execute(
                request(
                    plan,
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Replay {
                        archived_execution_id: live_execution,
                    },
                ),
                control_handlers(
                    FixtureHttp::success(replay_http_calls.clone()),
                    replay_quickjs_calls.clone(),
                    replay_extract_calls.clone(),
                ),
                archive,
            )
            .expect("typed control replay session"),
    )
    .await;
    assert!(matches!(
        replay_events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Completed)
    ));
    assert_eq!(replay_quickjs_calls.load(Ordering::SeqCst), 0);
    assert_eq!(replay_http_calls.load(Ordering::SeqCst), 0);
    assert_eq!(replay_extract_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn replay_missing_extra_and_tampered_control_invocations_never_call_live_handlers() {
    let plan = typed_control_plan(4, "control_alpha", MergeStrategy::ConcatArrays);
    let runtime = runtime(32);
    let live_execution = Uuid::new_v4();
    let live_archive = Arc::new(DurableFileArchive::new());
    let live_events = collect_events(
        runtime
            .execute(
                request(
                    plan.clone(),
                    live_execution,
                    lj_runtime::ExecutionMode::Live,
                ),
                control_handlers(
                    FixtureHttp::success(Arc::new(AtomicUsize::new(0))),
                    Arc::new(AtomicUsize::new(0)),
                    Arc::new(AtomicUsize::new(0)),
                ),
                live_archive.clone(),
            )
            .expect("sequence fixture live session"),
    )
    .await;
    assert!(matches!(
        live_events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Completed)
    ));
    let captures = live_archive.captures();
    let traces = live_archive.control_traces();

    let missing_archive = Arc::new(DurableFileArchive::from_records(
        captures.clone(),
        traces.clone(),
    ));
    missing_archive.remove_invocation(7);
    let missing_live_calls = Arc::new(AtomicUsize::new(0));
    let missing_events = collect_events(
        runtime
            .execute(
                request(
                    plan.clone(),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Replay {
                        archived_execution_id: live_execution,
                    },
                ),
                control_handlers(
                    FixtureHttp::success(missing_live_calls.clone()),
                    missing_live_calls.clone(),
                    missing_live_calls.clone(),
                ),
                missing_archive,
            )
            .expect("missing invocation replay session"),
    )
    .await;
    assert_eq!(missing_live_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        terminal_failure(&missing_events).map(|failure| failure.code),
        Some(RuntimeFailureCode::ReplayCaptureMissing)
    );

    let extra_archive = Arc::new(DurableFileArchive::from_records(
        captures.clone(),
        traces.clone(),
    ));
    extra_archive.append_extra_effect_invocation();
    let extra_live_calls = Arc::new(AtomicUsize::new(0));
    let extra_events = collect_events(
        runtime
            .execute(
                request(
                    plan.clone(),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Replay {
                        archived_execution_id: live_execution,
                    },
                ),
                control_handlers(
                    FixtureHttp::success(extra_live_calls.clone()),
                    extra_live_calls.clone(),
                    extra_live_calls.clone(),
                ),
                extra_archive,
            )
            .expect("extra invocation replay session"),
    )
    .await;
    assert_eq!(extra_live_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        terminal_failure(&extra_events).map(|failure| failure.code),
        Some(RuntimeFailureCode::ReplayCaptureMissing)
    );
    assert!(extra_events.iter().all(|event| !matches!(
        event.kind,
        lj_runtime::ExecutionEventKind::DeltaProduced { .. }
    )));

    let tampered_trace_archive = Arc::new(DurableFileArchive::from_records(captures, traces));
    tampered_trace_archive.tamper_first_condition_trace();
    let tampered_live_calls = Arc::new(AtomicUsize::new(0));
    let tampered_events = collect_events(
        runtime
            .execute(
                request(
                    plan,
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Replay {
                        archived_execution_id: live_execution,
                    },
                ),
                control_handlers(
                    FixtureHttp::success(tampered_live_calls.clone()),
                    tampered_live_calls.clone(),
                    tampered_live_calls.clone(),
                ),
                tampered_trace_archive,
            )
            .expect("tampered control replay session"),
    )
    .await;
    assert_eq!(tampered_live_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        terminal_failure(&tampered_events).map(|failure| failure.code),
        Some(RuntimeFailureCode::ReplayRecordMismatch)
    );
}

#[tokio::test]
async fn typed_and_js_control_modes_produce_the_same_branch_merge_and_loop_decisions() {
    let typed_plan = typed_control_plan(4, "control_alpha", MergeStrategy::ConcatArrays);
    let js_plan = compiled_control_plan(
        ControlExpression::Js {
            code: "return 'alpha'; // control_condition_alpha".to_string(),
        },
        CollectionSelector::Js {
            code: "return input; // control_loop_collection".to_string(),
        },
        "control_alpha",
        MergeStrategy::ConcatArrays,
        4,
    );
    let js_expression_plan = compiled_control_plan(
        ControlExpression::Js {
            code: "'alpha' /* control_condition_alpha */".to_string(),
        },
        CollectionSelector::Js {
            code: "input /* control_loop_collection */".to_string(),
        },
        "control_alpha",
        MergeStrategy::ConcatArrays,
        4,
    );
    let runtime = runtime(32);
    for plan in [&typed_plan, &js_plan, &js_expression_plan] {
        runtime
            .validate_plan(plan)
            .expect("typed and JS control Plans must both validate");
    }
    let mut decisions = Vec::new();
    let mut quickjs_counts = Vec::new();
    for plan in [typed_plan, js_plan, js_expression_plan] {
        let archive = Arc::new(DurableFileArchive::new());
        let quickjs_calls = Arc::new(AtomicUsize::new(0));
        let events = collect_events(
            runtime
                .execute(
                    request(plan, Uuid::new_v4(), lj_runtime::ExecutionMode::Live),
                    control_handlers(
                        FixtureHttp::success(Arc::new(AtomicUsize::new(0))),
                        quickjs_calls.clone(),
                        Arc::new(AtomicUsize::new(0)),
                    ),
                    archive.clone(),
                )
                .expect("control parity session"),
        )
        .await;
        assert!(matches!(
            events.last().map(|event| &event.kind),
            Some(lj_runtime::ExecutionEventKind::Completed)
        ));
        decisions.push(control_trace_kinds(&archive));
        quickjs_counts.push(quickjs_calls.load(Ordering::SeqCst));
    }
    assert!(decisions.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(quickjs_counts, [2, 4, 4]);
}

#[tokio::test]
async fn merge_uses_explicit_order_when_physical_input_storage_is_reversed() {
    let plan = rewrite_plan(
        &typed_control_plan(4, "control_alpha", MergeStrategy::ConcatArrays),
        |value| {
            let merge = value["nodes"]
                .as_array_mut()
                .expect("Plan nodes fixture")
                .iter_mut()
                .find(|node| node["config"]["kind"].as_str() == Some("merge"))
                .expect("Merge node fixture");
            merge["config"]["value"]["inputs"]
                .as_array_mut()
                .expect("Merge inputs fixture")
                .reverse();
        },
        true,
    );
    let runtime = runtime(32);
    runtime
        .validate_plan(&plan)
        .expect("explicit Merge order is independent of physical storage");
    let archive = Arc::new(DurableFileArchive::new());
    let events = collect_events(
        runtime
            .execute(
                request(plan, Uuid::new_v4(), lj_runtime::ExecutionMode::Live),
                control_handlers(
                    FixtureHttp::success(Arc::new(AtomicUsize::new(0))),
                    Arc::new(AtomicUsize::new(0)),
                    Arc::new(AtomicUsize::new(0)),
                ),
                archive.clone(),
            )
            .expect("reordered Merge session"),
    )
    .await;
    assert!(matches!(
        events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Completed)
    ));
    assert!(
        control_trace_kinds(&archive).contains(&ControlTrace::Merge {
            active_inputs: vec!["alpha".to_string()],
        })
    );
}

#[tokio::test]
async fn loop_empty_limit_non_array_and_body_failure_are_typed_and_bounded() {
    let runtime = runtime(32);
    let empty_archive = Arc::new(DurableFileArchive::new());
    let empty_http_calls = Arc::new(AtomicUsize::new(0));
    let empty_events = collect_events(
        runtime
            .execute(
                request(
                    typed_control_plan(4, "control_alpha_empty", MergeStrategy::ConcatArrays),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Live,
                ),
                control_handlers(
                    FixtureHttp::success(empty_http_calls.clone()),
                    Arc::new(AtomicUsize::new(0)),
                    Arc::new(AtomicUsize::new(0)),
                ),
                empty_archive.clone(),
            )
            .expect("empty Loop session"),
    )
    .await;
    assert!(matches!(
        empty_events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Completed)
    ));
    assert_eq!(empty_http_calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        control_trace_kinds(&empty_archive).last(),
        Some(ControlTrace::Loop { iteration_count: 0 })
    ));
    let limit_http_calls = Arc::new(AtomicUsize::new(0));
    let limit_events = collect_events(
        runtime
            .execute(
                request(
                    typed_control_plan(1, "control_alpha", MergeStrategy::ConcatArrays),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Live,
                ),
                control_handlers(
                    FixtureHttp::success(limit_http_calls.clone()),
                    Arc::new(AtomicUsize::new(0)),
                    Arc::new(AtomicUsize::new(0)),
                ),
                Arc::new(DurableFileArchive::new()),
            )
            .expect("Loop hard max session"),
    )
    .await;
    assert_eq!(limit_http_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        terminal_failure(&limit_events).map(|failure| failure.code),
        Some(RuntimeFailureCode::InputTypeMismatch)
    );
    let non_array_http_calls = Arc::new(AtomicUsize::new(0));
    let non_array_events = collect_events(
        runtime
            .execute(
                request(
                    typed_control_plan(4, "control_alpha_object", MergeStrategy::SingleActive),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Live,
                ),
                control_handlers(
                    FixtureHttp::success(non_array_http_calls.clone()),
                    Arc::new(AtomicUsize::new(0)),
                    Arc::new(AtomicUsize::new(0)),
                ),
                Arc::new(DurableFileArchive::new()),
            )
            .expect("Loop non-array session"),
    )
    .await;
    assert_eq!(non_array_http_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        terminal_failure(&non_array_events).map(|failure| failure.code),
        Some(RuntimeFailureCode::InputTypeMismatch)
    );
    let failed_http_calls = Arc::new(AtomicUsize::new(0));
    let failed_extract_calls = Arc::new(AtomicUsize::new(0));
    let failed_events = collect_events(
        runtime
            .execute(
                request(
                    typed_control_plan(4, "control_alpha", MergeStrategy::ConcatArrays),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Live,
                ),
                control_handlers(
                    FixtureHttp::failure(failed_http_calls.clone()),
                    Arc::new(AtomicUsize::new(0)),
                    failed_extract_calls.clone(),
                ),
                Arc::new(DurableFileArchive::new()),
            )
            .expect("Loop body failure session"),
    )
    .await;
    assert_eq!(failed_http_calls.load(Ordering::SeqCst), 1);
    assert_eq!(failed_extract_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        terminal_failure(&failed_events).map(|failure| failure.code),
        Some(RuntimeFailureCode::EffectFailed)
    );
}

#[tokio::test]
async fn loop_global_hard_max_is_inclusive_and_overflow_starts_no_body_effect() {
    let runtime = runtime(64);
    let hard_max = u16::try_from(MAX_LOOP_ITERATIONS).expect("hard max fits Definition wire");
    let boundary_http_calls = Arc::new(AtomicUsize::new(0));
    let boundary_extract_calls = Arc::new(AtomicUsize::new(0));
    let boundary_events = collect_events(
        runtime
            .execute(
                request(
                    typed_control_plan(
                        hard_max,
                        "control_alpha_hard_max",
                        MergeStrategy::ConcatArrays,
                    ),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Live,
                ),
                control_handlers(
                    FixtureHttp::success(boundary_http_calls.clone()),
                    Arc::new(AtomicUsize::new(0)),
                    boundary_extract_calls.clone(),
                ),
                Arc::new(DurableFileArchive::new()),
            )
            .expect("Loop hard max boundary session"),
    )
    .await;
    assert!(matches!(
        boundary_events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Completed)
    ));
    assert_eq!(
        boundary_http_calls.load(Ordering::SeqCst),
        usize::try_from(MAX_LOOP_ITERATIONS).expect("hard max fits usize")
    );
    assert_eq!(
        boundary_extract_calls.load(Ordering::SeqCst),
        usize::try_from(MAX_LOOP_ITERATIONS).expect("hard max fits usize")
    );

    let overflow_http_calls = Arc::new(AtomicUsize::new(0));
    let overflow_events = collect_events(
        runtime
            .execute(
                request(
                    typed_control_plan(
                        hard_max,
                        "control_alpha_hard_max_plus_one",
                        MergeStrategy::ConcatArrays,
                    ),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Live,
                ),
                control_handlers(
                    FixtureHttp::success(overflow_http_calls.clone()),
                    Arc::new(AtomicUsize::new(0)),
                    Arc::new(AtomicUsize::new(0)),
                ),
                Arc::new(DurableFileArchive::new()),
            )
            .expect("Loop hard max overflow session"),
    )
    .await;
    assert_eq!(overflow_http_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        terminal_failure(&overflow_events).map(|failure| failure.code),
        Some(RuntimeFailureCode::InputTypeMismatch)
    );
}

#[tokio::test]
async fn activated_required_merge_input_missing_is_a_typed_failure() {
    let plan = rewrite_plan(
        &typed_control_plan(4, "control_alpha", MergeStrategy::ConcatArrays),
        |value| {
            let alpha = Uuid::from_u128(1_003).to_string();
            let merge = Uuid::from_u128(1_005).to_string();
            value["edges"]
                .as_array_mut()
                .expect("Plan edges fixture")
                .retain(|edge| {
                    !(edge["from"]["node_id"].as_str() == Some(alpha.as_str())
                        && edge["to"]["node_id"].as_str() == Some(merge.as_str())
                        && edge["to"]["handle"].as_str() == Some("alpha"))
                });
        },
        true,
    );
    let http_calls = Arc::new(AtomicUsize::new(0));
    let events = collect_events(
        runtime(32)
            .execute(
                request(plan, Uuid::new_v4(), lj_runtime::ExecutionMode::Live),
                control_handlers(
                    FixtureHttp::success(http_calls.clone()),
                    Arc::new(AtomicUsize::new(0)),
                    Arc::new(AtomicUsize::new(0)),
                ),
                Arc::new(DurableFileArchive::new()),
            )
            .expect("missing required input session"),
    )
    .await;
    assert_eq!(http_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        terminal_failure(&events).map(|failure| failure.code),
        Some(RuntimeFailureCode::InputTypeMismatch)
    );
}

#[tokio::test]
async fn cancellation_inside_first_iteration_stops_all_later_body_effects() {
    let runtime = runtime(32);
    let http_calls = Arc::new(AtomicUsize::new(0));
    let extract_calls = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Notify::new());
    let started_wait = started.notified();
    let session = runtime
        .execute(
            request(
                typed_control_plan(4, "control_alpha", MergeStrategy::ConcatArrays),
                Uuid::new_v4(),
                lj_runtime::ExecutionMode::Live,
            ),
            control_handlers(
                FixtureHttp::wait_for_cancellation(http_calls.clone(), started.clone()),
                Arc::new(AtomicUsize::new(0)),
                extract_calls.clone(),
            ),
            Arc::new(DurableFileArchive::new()),
        )
        .expect("Loop cancellation session");
    tokio::time::timeout(Duration::from_secs(1), started_wait)
        .await
        .expect("first Loop body effect must start");
    assert!(session.cancellation_handle().cancel());
    let events = collect_events(session).await;
    assert_eq!(http_calls.load(Ordering::SeqCst), 1);
    assert_eq!(extract_calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Cancelled)
    ));
    assert_eq!(terminal_count(&events), 1);
}
