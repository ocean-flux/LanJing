//! 核心外 Rule Package 的端到端验收（ticket #64 的 P2）。
//!
//! fixture 不是编译期的内置测试数据: 它是磁盘上的 Rule Package 文件, 只走
//! `RuleInput::Package` 通用导入路径, 不经 Legado/Maccms 来源专有捷径。测试把这份文件从
//! import 推到候选、授权、Source Revision、编译、执行与 archive-only replay, 并覆盖图里
//! 两类规则能力: 来源规则 (Http 端点 + Extract 提取规则) 与受控 JS 节点。
//!
//! 不 mock install 事务、执行边界或 archive: 用真实 SQLite/artifact、真实 wiremock 与
//! concrete `RuleSystem`。

use std::fs;
use std::path::Path;
use std::time::Duration;

use futures::StreamExt;
use lj_capability::{IntentInput, StandardIntent};
use lj_media::{MediaGraphDelta, MediaResourceId};
use lj_rule_model::DiagnosticSeverity;
use lj_rule_system::test_support::{TempRuleSystem, init_mock_keyring};
use lj_rule_system::{
    CapabilityGrant, ExecuteRequest, ExecutionEventKind, ExecutionMode, RuleInput, RuleSystem,
    SourceId,
};
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SOURCE_IDENTITY: &str = "source:core-out-rule-package";
const BASE_URL_PLACEHOLDER: &str = "http://rule-package.invalid";
/// 来源规则 (Http 端点 + Extract 提取规则) 的搜索关键词。
const QUERY: &str = "零号档案";
const ITEM_TITLE: &str = "零号档案";
const ITEM_SOURCE_KEY: &str = "zero-001";
const ITEM_COVER: &str = "/cover/zero.jpg";
/// 受控 JS 节点通过 `{{key}}` 模板读取的标准意图输入。
const JS_QUERY: &str = "zero";
/// Search 路径上的 effect 节点: Http 与 Extract。
const SOURCE_RULE_EFFECTS: usize = 2;
/// Discover 路径上的 effect 节点: 受控 JS。
const CONTROLLED_JS_EFFECTS: usize = 1;

fn fixture_bytes() -> Vec<u8> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("core_out_rule_package.json");
    fs::read(&fixture).unwrap_or_else(|error| panic!("read fixture {}: {error}", fixture.display()))
}

/// 把 fixture 里声明的部署占位 base URL 换成真实 mock 端点。
///
/// fixture 只承诺「来源规则引用 `base_url`」, 不承诺某个固定地址; 注入后导入、编译与安装
/// 全部重新基于注入值派生。
fn package_input(base_url: &str) -> RuleInput {
    let mut package: Value = serde_json::from_slice(&fixture_bytes()).expect("parse fixture JSON");
    let definition = package
        .get_mut("definition")
        .expect("Rule Package 必须含嵌套 Definition");
    assert_eq!(
        definition["base_url"],
        json!(BASE_URL_PLACEHOLDER),
        "fixture 只声明占位 base_url, 由调用方注入真实部署地址"
    );
    definition["base_url"] = json!(base_url);
    RuleInput::Package {
        source_json: serde_json::to_string(&package).expect("serialize Rule Package input"),
    }
}

async fn mount_search_route(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(query_param("key", QUERY))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "list": [
                {
                    "name": ITEM_TITLE,
                    "detail_id": ITEM_SOURCE_KEY,
                    "cover": ITEM_COVER,
                    "unused": "fixture 里未进入来源规则的字段不应出现在标准媒体模型"
                }
            ]
        })))
        .mount(server)
        .await;
}

struct LiveRun {
    execution_id: lj_rule_system::ExecutionId,
    events: Vec<lj_rule_system::ExecutionEvent>,
}

async fn run_live(
    system: &RuleSystem,
    source_id: &SourceId,
    intent: StandardIntent,
    input: IntentInput,
) -> LiveRun {
    let session = system
        .execute(ExecuteRequest {
            source_id: source_id.clone(),
            intent,
            input,
            mode: ExecutionMode::Live,
        })
        .await
        .expect("已安装来源的标准意图必须可执行");
    let execution_id = session.id;
    let events = session.into_events().collect::<Vec<_>>().await;
    assert_contiguous(&events);
    assert_eq!(terminal_count(&events), 1, "session 只能有一个终态");
    assert!(
        matches!(
            events.last().map(|event| &event.kind),
            Some(ExecutionEventKind::Completed)
        ),
        "live execution 必须以唯一 Completed 终态结束: {events:?}"
    );
    LiveRun {
        execution_id,
        events,
    }
}

async fn run_replay(
    system: &RuleSystem,
    source_id: &SourceId,
    intent: StandardIntent,
    input: IntentInput,
    execution_id: lj_rule_system::ExecutionId,
) -> Vec<lj_rule_system::ExecutionEvent> {
    let session = system
        .execute(ExecuteRequest {
            source_id: source_id.clone(),
            intent,
            input,
            mode: ExecutionMode::Replay { execution_id },
        })
        .await
        .expect("断网后 replay 必须仍能从 execution pin 的 archive 启动");
    let events = session.into_events().collect::<Vec<_>>().await;
    assert_contiguous(&events);
    assert_eq!(terminal_count(&events), 1, "session 只能有一个终态");
    assert!(
        matches!(
            events.last().map(|event| &event.kind),
            Some(ExecutionEventKind::Completed)
        ),
        "replay 必须以唯一 Completed 终态结束: {events:?}"
    );
    assert_eq!(
        captured_effects(&events),
        0,
        "replay 不得产生新的 live capture, 只允许读取历史 archive: {events:?}"
    );
    events
}

fn captured_effects(events: &[lj_rule_system::ExecutionEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event.kind, ExecutionEventKind::EffectCaptured { .. }))
        .count()
}

fn assert_contiguous(events: &[lj_rule_system::ExecutionEvent]) {
    for (index, event) in events.iter().enumerate() {
        assert_eq!(
            event.sequence,
            u64::try_from(index + 1).expect("测试序号可转换为 u64"),
            "catch-up 不得产生 sequence 洞"
        );
    }
}

fn terminal_count(events: &[lj_rule_system::ExecutionEvent]) -> usize {
    events
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                ExecutionEventKind::Completed
                    | ExecutionEventKind::Failed { .. }
                    | ExecutionEventKind::Cancelled
            )
        })
        .count()
}

fn committed_delta(events: &[lj_rule_system::ExecutionEvent]) -> MediaGraphDelta {
    events
        .iter()
        .find_map(|event| match &event.kind {
            ExecutionEventKind::DeltaCommitted { delta, .. } => Some(delta.clone()),
            _ => None,
        })
        .expect("execution 必须提交标准媒体增量")
}

/// 该路径上的每个 effect 节点都必须进入 durable capture, 且必须发生在 Delta 提交之前。
fn assert_live_capture(events: &[lj_rule_system::ExecutionEvent], expected: usize) {
    let commit_index = events
        .iter()
        .position(|event| matches!(event.kind, ExecutionEventKind::DeltaCommitted { .. }))
        .expect("live execution 必须提交 Delta");
    let captured: Vec<&lj_rule_system::ExecutionEvent> = events[..commit_index]
        .iter()
        .filter(|event| matches!(event.kind, ExecutionEventKind::EffectCaptured { .. }))
        .collect();
    assert_eq!(
        captured.len(),
        expected,
        "本 intent 路径上的全部 effect 都必须在 Delta 提交前留下 durable capture: {events:?}"
    );
    for event in captured {
        let ExecutionEventKind::EffectCaptured {
            artifact_refs,
            output_hash,
            ..
        } = &event.kind
        else {
            unreachable!("已按 EffectCaptured 过滤");
        };
        assert!(
            !artifact_refs.is_empty(),
            "live effect capture 必须有 durable artifact 引用"
        );
        assert_eq!(output_hash.len(), 64, "live effect 输出必须带 BLAKE3 hex");
    }
}

fn assert_source_profile(delta: &MediaGraphDelta, source_id: &MediaResourceId) {
    assert!(
        delta.sources.iter().any(|profile| &profile.id == source_id),
        "增量必须携带已安装来源的 profile"
    );
}

/// 来源规则路径: Http 端点取回 mock 响应, Extract 的字段规则产标准媒体主体。
fn assert_source_rule_item(delta: &MediaGraphDelta, source_id: &MediaResourceId) {
    assert_source_profile(delta, source_id);
    let item = delta
        .items
        .iter()
        .find(|item| item.title == ITEM_TITLE)
        .expect("来源规则必须产出标准媒体主体");
    assert_eq!(&item.source_id, source_id, "item 必须属于已安装来源");
    assert_eq!(
        item.metadata.get("source_item_id").and_then(Value::as_str),
        Some(ITEM_SOURCE_KEY),
        "Extract 的来源 id 规则必须落到标准模型 metadata"
    );
    assert_eq!(
        item.metadata.get("cover_url").and_then(Value::as_str),
        Some(ITEM_COVER)
    );
    assert!(
        !item.metadata.contains_key("unused"),
        "来源规则没有声明的字段不得泄漏进标准媒体模型"
    );
}

/// 受控 JS 路径: 脚本在沙箱里读标准意图输入并产出发现动作。
fn assert_controlled_js_discovery(delta: &MediaGraphDelta, source_id: &MediaResourceId) {
    assert_source_profile(delta, source_id);
    let action = delta
        .actions
        .iter()
        .find(|action| action.label == JS_QUERY)
        .expect("受控 JS 节点产出的发现动作必须被提交");
    assert_eq!(&action.source_id, source_id, "action 必须属于已安装来源");
    assert_eq!(action.intent, StandardIntent::ContinueAction);
    assert_eq!(
        action.payload.get("url").and_then(Value::as_str),
        Some("/browse/zero"),
        "受控 JS 必须读到标准意图输入, 而不是空输入"
    );
}

#[tokio::test]
async fn core_out_rule_package_fixture_is_self_consistent_and_installs_as_a_candidate() {
    init_mock_keyring();
    let temp = TempRuleSystem::new("core-out-fixture");
    let system = temp.open(Duration::from_mins(1)).await;

    let inspection = system
        .inspect_rule_package(&fixture_bytes())
        .expect("核心外 Rule Package 必须通过 facade 校验");
    assert_eq!(json!(inspection.source_id), json!(SOURCE_IDENTITY));
    assert_eq!(
        inspection.version, inspection.definition_hash,
        "fixture 自己声明的 version 必须就是 canonical Definition hash"
    );
    assert!(inspection.unavailable_nodes.is_empty());
    assert!(
        !inspection
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error),
        "fixture 不得带 error 级诊断: {:?}",
        inspection.diagnostics
    );

    // 通用导入路径: 不经任何来源专有 importer, 也不触碰网络。
    let candidate = system
        .prepare_install(package_input(BASE_URL_PLACEHOLDER))
        .await
        .expect("Rule Package 必须能生成 durable 候选");
    assert_eq!(
        candidate.definition_hash, inspection.definition_hash,
        "候选必须与已校验的 Rule Package 绑定同一个 Definition"
    );
    drop(system);
}

#[tokio::test]
async fn core_out_rule_package_covers_source_rules_and_controlled_js_live_and_replay() {
    init_mock_keyring();
    let temp = TempRuleSystem::new("core-out-e2e");
    let server = MockServer::start().await;
    mount_search_route(&server).await;
    let system = temp.open(Duration::from_mins(1)).await;

    let candidate = system
        .prepare_install(package_input(&server.uri()))
        .await
        .expect("核心外 Rule Package 候选");
    let source = system
        .install(candidate.id, CapabilityGrant::network_only())
        .await
        .expect("用户批准 network grant 后必须完成安装与 Source Revision");
    assert_eq!(source.revision, 1);

    let search = run_live(
        &system,
        &source.source_id,
        StandardIntent::Search,
        IntentInput::Query(QUERY.to_string()),
    )
    .await;
    assert_live_capture(&search.events, SOURCE_RULE_EFFECTS);
    let search_delta = committed_delta(&search.events);
    assert_source_rule_item(&search_delta, &source.profile.id);

    let discover = run_live(
        &system,
        &source.source_id,
        StandardIntent::Discover,
        IntentInput::Query(JS_QUERY.to_string()),
    )
    .await;
    assert_live_capture(&discover.events, CONTROLLED_JS_EFFECTS);
    let discover_delta = committed_delta(&discover.events);
    assert_controlled_js_discovery(&discover_delta, &source.profile.id);

    // 关掉 mock 端点: 两次 replay 若能完成, 只可能来自 durable archive。
    drop(server);
    let replayed_search = run_replay(
        &system,
        &source.source_id,
        StandardIntent::Search,
        IntentInput::Query(QUERY.to_string()),
        search.execution_id,
    )
    .await;
    assert_eq!(
        committed_delta(&replayed_search),
        search_delta,
        "replay 必须复现历史输入输出, 而不是重新观测"
    );
    let replayed_discover = run_replay(
        &system,
        &source.source_id,
        StandardIntent::Discover,
        IntentInput::Query(JS_QUERY.to_string()),
        discover.execution_id,
    )
    .await;
    assert_eq!(
        committed_delta(&replayed_discover),
        discover_delta,
        "受控 JS effect 的 replay 必须复现历史输入输出"
    );
    drop(system);
}
