//! 取消、全局/来源 semaphore 与事件通道背压合同。

use super::*;

#[tokio::test]
async fn cancellation_stops_new_effects_and_emits_only_cancelled() {
    let runtime = runtime(4);
    let archive = Arc::new(DurableFileArchive::new());
    let http_calls = Arc::new(AtomicUsize::new(0));
    let extract_calls = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Notify::new());
    let started_wait = started.notified();
    let session = runtime
        .execute(
            request(
                sample_plan(),
                Uuid::new_v4(),
                lj_runtime::ExecutionMode::Live,
            ),
            handlers(
                FixtureHttp::wait_for_cancellation(http_calls.clone(), started.clone()),
                extract_calls.clone(),
            ),
            archive,
        )
        .expect("cancel session");
    tokio::time::timeout(Duration::from_secs(1), started_wait)
        .await
        .expect("HTTP effect must start");
    let cancellation = session.cancellation_handle();
    assert!(cancellation.cancel(), "第一次取消必须改变状态");
    assert!(!cancellation.cancel(), "重复取消必须幂等");
    assert!(session.is_cancelled());
    let events = collect_events(session).await;

    assert_eq!(http_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        extract_calls.load(Ordering::SeqCst),
        0,
        "取消后不得调度 Extract"
    );
    assert_eq!(terminal_count(&events), 1);
    assert!(matches!(
        events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Cancelled)
    ));
}

#[tokio::test]
async fn source_effect_semaphore_blocks_second_execution_until_first_releases() {
    let runtime = runtime(8);
    let archive = Arc::new(DurableFileArchive::new());
    let http_calls = Arc::new(AtomicUsize::new(0));
    let extract_calls = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let started_wait = started.notified();

    let first = runtime
        .execute(
            request(
                sample_plan(),
                Uuid::new_v4(),
                lj_runtime::ExecutionMode::Live,
            ),
            handlers(
                FixtureHttp::wait_for_release(http_calls.clone(), started.clone(), release.clone()),
                extract_calls.clone(),
            ),
            archive.clone(),
        )
        .expect("first session");
    tokio::time::timeout(Duration::from_secs(1), started_wait)
        .await
        .expect("first HTTP effect must start");

    let second = runtime
        .execute(
            request(
                sample_plan(),
                Uuid::new_v4(),
                lj_runtime::ExecutionMode::Live,
            ),
            handlers(
                FixtureHttp::wait_for_release(http_calls.clone(), started, release.clone()),
                extract_calls.clone(),
            ),
            archive,
        )
        .expect("second session");
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        http_calls.load(Ordering::SeqCst),
        1,
        "同一来源的第二个 effect 必须等待 source semaphore"
    );

    release.notify_one();
    release.notify_one();
    let first_events = collect_events(first).await;
    let second_events = collect_events(second).await;
    assert_eq!(terminal_count(&first_events), 1);
    assert_eq!(terminal_count(&second_events), 1);
    assert!(matches!(
        first_events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Completed)
    ));
    assert!(matches!(
        second_events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Completed)
    ));
}

#[tokio::test]
async fn global_effect_semaphore_blocks_distinct_sources() {
    let runtime = PlanRuntime::new(PlanRuntimeConfig {
        compiler_version: "runtime-test-compiler@1".to_string(),
        event_channel_capacity: 8,
        max_concurrent_executions: 2,
        max_concurrent_effects: 1,
        max_concurrent_effects_per_source: 1,
    })
    .expect("runtime config");
    let archive = Arc::new(DurableFileArchive::new());
    let http_calls = Arc::new(AtomicUsize::new(0));
    let extract_calls = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Notify::new());
    let started_wait = started.notified();
    let release = Arc::new(Notify::new());

    let first = runtime
        .execute(
            request(
                sample_plan(),
                Uuid::new_v4(),
                lj_runtime::ExecutionMode::Live,
            ),
            handlers(
                FixtureHttp::wait_for_release(http_calls.clone(), started.clone(), release.clone()),
                extract_calls.clone(),
            ),
            archive.clone(),
        )
        .expect("first session");
    tokio::time::timeout(Duration::from_secs(1), started_wait)
        .await
        .expect("first HTTP effect must start");

    let mut other_source_request = request(
        sample_plan(),
        Uuid::new_v4(),
        lj_runtime::ExecutionMode::Live,
    );
    other_source_request.source_id = "other-runtime-test-source".to_string();
    let second = runtime
        .execute(
            other_source_request,
            handlers(
                FixtureHttp::success(http_calls.clone()),
                extract_calls.clone(),
            ),
            archive,
        )
        .expect("second session");
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        http_calls.load(Ordering::SeqCst),
        1,
        "不同来源仍必须共同受全局 effect semaphore 限制"
    );

    release.notify_one();
    let first_events = collect_events(first).await;
    let second_events = collect_events(second).await;
    assert_eq!(http_calls.load(Ordering::SeqCst), 2);
    assert!(matches!(
        first_events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Completed)
    ));
    assert!(matches!(
        second_events.last().map(|event| &event.kind),
        Some(lj_runtime::ExecutionEventKind::Completed)
    ));
}

#[tokio::test]
async fn effect_error_becomes_one_failed_terminal_with_attribution() {
    let runtime = runtime(4);
    let events = collect_events(
        runtime
            .execute(
                request(
                    sample_plan(),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Live,
                ),
                handlers(
                    FixtureHttp::failure(Arc::new(AtomicUsize::new(0))),
                    Arc::new(AtomicUsize::new(0)),
                ),
                Arc::new(DurableFileArchive::new()),
            )
            .expect("error session"),
    )
    .await;

    assert_eq!(terminal_count(&events), 1);
    let Some(lj_runtime::ExecutionEventKind::Failed { failure }) =
        events.last().map(|event| &event.kind)
    else {
        panic!("effect error 必须进入 Failed 终态");
    };
    assert_eq!(failure.code, RuntimeFailureCode::EffectFailed);
    assert_eq!(failure.node_id, Some(Uuid::from_u128(101)));
    assert!(failure.effect_id.is_some());
    assert_eq!(failure.execution_id, events[0].execution_id);
}

#[tokio::test]
async fn missing_network_capability_fails_before_any_effect_with_stable_code() {
    let runtime = runtime(4);
    let archive = Arc::new(DurableFileArchive::new());
    let http_calls = Arc::new(AtomicUsize::new(0));
    let extract_calls = Arc::new(AtomicUsize::new(0));
    let mut denied = request(
        sample_plan(),
        Uuid::new_v4(),
        lj_runtime::ExecutionMode::Live,
    );
    denied.capabilities = PolicyCapabilities {
        network: false,
        system: SystemCapabilities::default(),
    };
    let events = collect_events(
        runtime
            .execute(
                denied,
                handlers(
                    FixtureHttp::success(http_calls.clone()),
                    extract_calls.clone(),
                ),
                archive,
            )
            .expect("denied session"),
    )
    .await;

    assert_eq!(
        http_calls.load(Ordering::SeqCst),
        0,
        "capability 被拒绝时不得触碰外部 handler"
    );
    assert_eq!(extract_calls.load(Ordering::SeqCst), 0);
    assert!(
        !events.iter().any(|event| matches!(
            event.kind,
            lj_runtime::ExecutionEventKind::EffectCaptured { .. }
        )),
        "被拒绝的 effect 不得留下 live capture: {events:?}"
    );
    assert_eq!(terminal_count(&events), 1);
    let Some(lj_runtime::ExecutionEventKind::Failed { failure }) =
        events.last().map(|event| &event.kind)
    else {
        panic!("capability 拒绝必须进入 Failed 终态");
    };
    assert_eq!(failure.code, RuntimeFailureCode::CapabilityDenied);
    assert_eq!(failure.node_id, Some(Uuid::from_u128(101)));
    assert_eq!(failure.execution_id, events[0].execution_id);
}

#[tokio::test]
async fn script_timeout_is_a_captured_failed_terminal_and_not_a_cancellation() {
    let runtime = runtime(4);
    let archive = Arc::new(DurableFileArchive::new());
    let quickjs_calls = Arc::new(AtomicUsize::new(0));
    let live_execution = Uuid::new_v4();
    let live_events = collect_events(
        runtime
            .execute(
                request(
                    quickjs_plan(),
                    live_execution,
                    lj_runtime::ExecutionMode::Live,
                ),
                timeout_handlers(
                    FixtureHttp::success(Arc::new(AtomicUsize::new(0))),
                    quickjs_calls.clone(),
                ),
                archive.clone(),
            )
            .expect("timeout live session"),
    )
    .await;

    assert_eq!(quickjs_calls.load(Ordering::SeqCst), 1);
    assert!(
        !live_events
            .iter()
            .any(|event| matches!(event.kind, lj_runtime::ExecutionEventKind::Cancelled)),
        "脚本超时是 effect 失败, 不是 cancellation: {live_events:?}"
    );
    assert_eq!(terminal_count(&live_events), 1);
    let Some(lj_runtime::ExecutionEventKind::Failed { failure }) =
        live_events.last().map(|event| &event.kind)
    else {
        panic!("脚本超时必须进入 Failed 终态");
    };
    assert_eq!(failure.code, RuntimeFailureCode::EffectFailed);
    assert_eq!(failure.node_id, Some(Uuid::from_u128(1)));
    assert!(failure.effect_id.is_some());
    assert_eq!(failure.execution_id, live_events[0].execution_id);

    // 失败类别本身 (而不只是一个笼统的失败) 必须随输出 durable 归档, 否则 replay 无法复现。
    let captures = archive.captures();
    assert_eq!(captures.len(), 1, "超时 effect 必须先 durable capture");
    assert!(
        matches!(
            captures[0].output.as_ref(),
            EffectOutput::QuickJs(QuickJsOutput::Error(QuickJsErrorKind::Timeout))
        ),
        "capture 必须保存稳定的 timeout 类别: {:?}",
        captures[0].output
    );
    let capture_position = live_events
        .iter()
        .position(|event| {
            matches!(
                event.kind,
                lj_runtime::ExecutionEventKind::EffectCaptured { .. }
            )
        })
        .expect("超时 effect 必须发出 EffectCaptured");
    assert!(
        capture_position < live_events.len() - 1,
        "durable capture 必须先于失败终态"
    );

    let replay_events = collect_events(
        runtime
            .execute(
                request(
                    quickjs_plan(),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Replay {
                        archived_execution_id: live_execution,
                    },
                ),
                timeout_handlers(
                    FixtureHttp::success(Arc::new(AtomicUsize::new(0))),
                    quickjs_calls.clone(),
                ),
                archive,
            )
            .expect("timeout replay session"),
    )
    .await;

    assert_eq!(
        quickjs_calls.load(Ordering::SeqCst),
        1,
        "replay 不得重跑 live handler"
    );
    assert_eq!(terminal_count(&replay_events), 1);
    assert_eq!(
        replay_events
            .iter()
            .filter(|event| matches!(
                event.kind,
                lj_runtime::ExecutionEventKind::EffectReplayed { .. }
            ))
            .count(),
        1,
        "超时 capture 必须可被 replay 读取"
    );
    let Some(lj_runtime::ExecutionEventKind::Failed { failure }) =
        replay_events.last().map(|event| &event.kind)
    else {
        panic!("超时 capture 的 replay 必须复现同一 Failed 终态");
    };
    assert_eq!(failure.code, RuntimeFailureCode::EffectFailed);
    assert_eq!(failure.node_id, Some(Uuid::from_u128(1)));
}

#[tokio::test]
async fn missing_operation_in_frozen_registry_fails_with_stable_code() {
    let runtime = runtime(4);
    let events = collect_events(
        runtime
            .execute(
                request(
                    quickjs_plan(),
                    Uuid::new_v4(),
                    lj_runtime::ExecutionMode::Live,
                ),
                registry_without_quickjs(
                    FixtureHttp::success(Arc::new(AtomicUsize::new(0))),
                    Arc::new(AtomicUsize::new(0)),
                ),
                Arc::new(DurableFileArchive::new()),
            )
            .expect("missing operation session"),
    )
    .await;

    assert_eq!(terminal_count(&events), 1);
    let Some(lj_runtime::ExecutionEventKind::Failed { failure }) =
        events.last().map(|event| &event.kind)
    else {
        panic!("registry 缺少 operation 必须进入 Failed 终态");
    };
    assert_eq!(failure.code, RuntimeFailureCode::OperationUnavailable);
    assert!(
        failure.message.contains("frozen registry"),
        "失败必须指明是 registry 缺少 handler，而不是 handler 自身失败：{}",
        failure.message
    );
}

#[tokio::test]
async fn bounded_event_channel_backpressures_before_downstream_effect() {
    let runtime = runtime(1);
    let archive = Arc::new(DurableFileArchive::new());
    let http_calls = Arc::new(AtomicUsize::new(0));
    let extract_calls = Arc::new(AtomicUsize::new(0));
    let session = runtime
        .execute(
            request(
                sample_plan(),
                Uuid::new_v4(),
                lj_runtime::ExecutionMode::Live,
            ),
            handlers(FixtureHttp::success(http_calls), extract_calls.clone()),
            archive.clone(),
        )
        .expect("backpressure session");

    tokio::time::timeout(Duration::from_secs(1), archive.wait_until_persisted())
        .await
        .expect("HTTP capture must be durable before event delivery");
    tokio::task::yield_now().await;
    assert_eq!(
        extract_calls.load(Ordering::SeqCst),
        0,
        "capacity=1 时 EffectCaptured 被 Started 背压，不能提前推进下游",
    );

    let mut stream = session.into_events();
    assert!(matches!(
        stream.next().await.map(|event| event.kind),
        Some(lj_runtime::ExecutionEventKind::Started)
    ));
    assert!(matches!(
        stream.next().await.map(|event| event.kind),
        Some(lj_runtime::ExecutionEventKind::EffectCaptured { .. })
    ));
    let remaining: Vec<_> = stream.collect().await;
    assert!(remaining.iter().any(|event| {
        matches!(
            event.kind,
            lj_runtime::ExecutionEventKind::Completed
                | lj_runtime::ExecutionEventKind::Failed { .. }
        )
    }));
    assert_eq!(extract_calls.load(Ordering::SeqCst), 1);
}
