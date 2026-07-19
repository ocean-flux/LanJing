//! Legado 六个标准 intent 的 `RuleSystem` live/replay 黄金合同。
//!
//! 本测试只通过 concrete `RuleSystem`、真实 C2 SQLite/artifact/keyring 和本地 wiremock
//! 驱动来源；不会组装旧 Graph、执行器、handler registry 或 mock archive success。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

use futures::StreamExt;
use keyring::{mock, set_default_credential_builder};
use lj_capability::{IntentInput, StandardIntent};
use lj_importer::legado::{CONTINUE_ACTION_TTL_MS, LegadoImporter};
use lj_media::{MediaAssetLocator, MediaGraphDelta, MediaKind};
use lj_rule_system::{
    CapabilityGrant, EffectWitnessCaptureForTest, EffectWitnessForTest, ExecuteRequest,
    ExecutionEvent, ExecutionEventKind, ExecutionId, ExecutionMode, HttpDnsTargetKindForTest,
    HttpMethodForTest, QuickJsHostCallForTest, RuleErrorStage, RuleInput, RuleSystem,
    RuleSystemConfig, SourceId,
};
use serde_json::{Value, json};
use uuid::Uuid;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SOURCE_STATIC_SECRET: &str = "source-static-secret";
const SEARCH_QUERY_FRAGMENT: &str = "?q=修罗";

struct TempRuleSystem {
    root: PathBuf,
    keyring_service: String,
}

impl TempRuleSystem {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("lj-legado-rule-system-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create RuleSystem test root");
        Self {
            root,
            keyring_service: format!("lanjing.legado.rule-system.test.{}", Uuid::new_v4()),
        }
    }

    async fn open(&self) -> RuleSystem {
        RuleSystem::open(
            RuleSystemConfig::local_fixture(
                self.root.join("event-store.db"),
                self.root.join("artifacts"),
            )
            .with_keyring_service(self.keyring_service.clone()),
        )
        .await
        .expect("open concrete RuleSystem")
    }
}

impl Drop for TempRuleSystem {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn init_mock_keyring() {
    static INIT: Once = Once::new();
    INIT.call_once(|| set_default_credential_builder(mock::default_credential_builder()));
}

fn legado_input(base_url: &str) -> RuleInput {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("legado_star_free_novel.json");
    let raw = fs::read_to_string(&fixture)
        .unwrap_or_else(|error| panic!("read fixture {}: {error}", fixture.display()));
    let mut source = serde_json::from_str::<Value>(&raw).expect("parse fixture JSON");
    source["bookSourceUrl"] = Value::String(base_url.to_string());
    let instrumented_explore = source["exploreUrl"]
        .as_str()
        .expect("fixture should contain Explore QuickJS")
        .replacen("@js:", "@js:\nDate.now();\nMath.random();\n", 1);
    source["exploreUrl"] = Value::String(instrumented_explore);
    source["header"] = Value::String(
        json!({
            "User-Agent": "legado-golden",
            "Authorization": format!("Bearer {SOURCE_STATIC_SECRET}"),
            "Cookie": format!("sid={SOURCE_STATIC_SECRET}"),
        })
        .to_string(),
    );
    RuleInput::Legado {
        source_json: serde_json::to_string(&source).expect("serialize local source input"),
    }
}

fn legado_input_without_source_credentials(base_url: &str) -> RuleInput {
    let RuleInput::Legado { source_json } = legado_input(base_url) else {
        unreachable!("fixture must construct a Legado input");
    };
    let mut source = serde_json::from_str::<Value>(&source_json)
        .expect("parse local Legado source without credentials");
    source["header"] = Value::String(json!({ "User-Agent": "legado-golden" }).to_string());
    RuleInput::Legado {
        source_json: serde_json::to_string(&source)
            .expect("serialize local Legado source without credentials"),
    }
}
async fn mount_legado_routes(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("q", "修罗"))
        .and(header("authorization", "Bearer source-static-secret"))
        .and(header("cookie", "sid=source-static-secret"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/search-result"))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/search-result"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("set-cookie", "session=response-capture-secret")
                .set_body_string(
                    r"
                    <ul>
                      <li itemprop='mainEntity'>
                        <h2 itemprop='name'>修罗武神</h2>
                        <p itemprop='author'>善良的蜜蜂</p>
                        <a itemprop='url' href='/book/1'>详情</a>
                        <img itemprop='image' src='/cover/1.jpg' />
                        <span itemprop='genre'>玄幻</span>
                      </li>
                    </ul>
                    ",
                ),
        )
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/haomenzongcai"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r"
            <section>
              <ul>
                <li itemprop='mainEntity'>
                  <h2 itemprop='name'>豪门测试书</h2>
                  <p itemprop='author'>作者甲</p>
                  <a itemprop='url' href='/book/2'>详情</a>
                  <img itemprop='image' src='/cover/2.jpg' />
                </li>
              </ul>
            </section>
            ",
        ))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/book/1"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r"
            <main>
              <h1>修罗武神</h1>
              <figure><img src='/cover/1.jpg' /></figure>
              <a href='/author/bee'>善良的蜜蜂</a>
              <div itemprop='description'>少年成长为强者。</div>
              <ol><li>占位</li><li>占位</li><li><a>玄幻</a></li></ol>
              <span>10万字</span>
              <div id='full-catalog'>
                <a href='/read/1.html'>第一章 起始</a>
                <a href='/read/2.html'>第二章 风起</a>
              </div>
            </main>
            ",
        ))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/read/1.html"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r"<article id='article-content'><p>第一章 起始</p><p>正文内容</p></article>",
        ))
        .mount(server)
        .await;
}

async fn mount_credential_free_search_redirect(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("q", "修罗"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/search-result"))
        .mount(server)
        .await;
}

#[derive(Clone)]
struct LiveExecution {
    id: ExecutionId,
    events: Vec<ExecutionEvent>,
    delta: MediaGraphDelta,
}

async fn execute_live(
    system: &RuleSystem,
    source_id: &SourceId,
    intent: StandardIntent,
    input: IntentInput,
) -> LiveExecution {
    let session = system
        .execute(ExecuteRequest {
            source_id: source_id.clone(),
            intent,
            input,
            mode: ExecutionMode::Live,
        })
        .await
        .expect("installed Legado source should start live execution");
    let id = session.id;
    let events = session.into_events().collect::<Vec<_>>().await;
    assert_completed(&events, true);
    LiveExecution {
        id,
        delta: committed_delta(&events).clone(),
        events,
    }
}

async fn execute_replay(
    system: &RuleSystem,
    source_id: &SourceId,
    intent: StandardIntent,
    input: IntentInput,
    archived_execution_id: ExecutionId,
) -> Vec<ExecutionEvent> {
    system
        .execute(ExecuteRequest {
            source_id: source_id.clone(),
            intent,
            input,
            mode: ExecutionMode::Replay {
                execution_id: archived_execution_id,
            },
        })
        .await
        .expect("historical pin should start replay")
        .into_events()
        .collect::<Vec<_>>()
        .await
}

fn committed_delta(events: &[ExecutionEvent]) -> &MediaGraphDelta {
    events
        .iter()
        .find_map(|event| match &event.kind {
            ExecutionEventKind::DeltaCommitted { delta, .. } => Some(delta),
            _ => None,
        })
        .expect("execution should commit a Delta before terminal state")
}

fn assert_completed(events: &[ExecutionEvent], require_effect_capture: bool) {
    assert_contiguous(events);
    assert!(matches!(
        events.last().map(|event| &event.kind),
        Some(ExecutionEventKind::Completed)
    ));
    assert_eq!(
        events
            .iter()
            .filter(|event| is_terminal(&event.kind))
            .count(),
        1,
        "execution must have exactly one terminal event: {events:?}"
    );
    let commit_index = events
        .iter()
        .position(|event| matches!(event.kind, ExecutionEventKind::DeltaCommitted { .. }))
        .expect("execution must commit a Delta");
    assert!(commit_index + 1 < events.len());
    if require_effect_capture {
        assert!(
            events[..commit_index]
                .iter()
                .any(|event| matches!(event.kind, ExecutionEventKind::EffectCaptured { .. })),
            "live path must durable-capture effects before Delta commit"
        );
    } else {
        assert!(
            events
                .iter()
                .all(|event| !matches!(event.kind, ExecutionEventKind::EffectCaptured { .. })),
            "replay must not invoke or capture a live effect"
        );
    }
}

fn assert_contiguous(events: &[ExecutionEvent]) {
    for (index, event) in events.iter().enumerate() {
        assert_eq!(
            event.sequence,
            u64::try_from(index + 1).expect("sequence fits u64")
        );
    }
}

fn is_terminal(kind: &ExecutionEventKind) -> bool {
    matches!(
        kind,
        ExecutionEventKind::Completed
            | ExecutionEventKind::Failed { .. }
            | ExecutionEventKind::Cancelled
    )
}

fn find_item(delta: &MediaGraphDelta, title: &str) -> String {
    let item = delta
        .items
        .iter()
        .find(|item| item.title == title)
        .expect("expected text item");
    assert_eq!(item.media_kind, MediaKind::Text);
    item.id.0.clone()
}

fn find_unit(delta: &MediaGraphDelta) -> String {
    delta
        .units
        .iter()
        .find(|unit| unit.title.contains("第一章"))
        .unwrap_or_else(|| panic!("expected text unit, received delta: {delta:#?}"))
        .id
        .0
        .clone()
}

fn assert_text_asset(delta: &MediaGraphDelta) {
    assert!(delta.assets.iter().any(|asset| matches!(
        &asset.locator,
        MediaAssetLocator::Text(text) if text.contains("正文内容")
    )));
}

/// 断言黄金路径只通过标准媒体资源的来源与父子归属暴露结果。
fn assert_standard_source_ownership(
    delta: &MediaGraphDelta,
    expected_source_id: &str,
    expected_item_id: Option<&str>,
    expected_unit_id: Option<&str>,
) {
    for item in &delta.items {
        assert_eq!(item.source_id.0, expected_source_id);
    }
    for unit in &delta.units {
        assert_eq!(unit.source_id.0, expected_source_id);
        if let Some(expected_item_id) = expected_item_id {
            assert_eq!(unit.item_id.0, expected_item_id);
        }
    }
    for asset in &delta.assets {
        assert_eq!(asset.source_id.0, expected_source_id);
        if let Some(expected_unit_id) = expected_unit_id {
            assert_eq!(
                asset.unit_id.as_ref().map(|unit_id| unit_id.0.as_str()),
                Some(expected_unit_id)
            );
        }
    }
    for action in &delta.actions {
        assert_eq!(action.source_id.0, expected_source_id);
        if action.intent == StandardIntent::ContinueAction {
            assert_eq!(
                action.payload["source_identity"].as_str(),
                Some(expected_source_id)
            );
        }
    }
    for source in &delta.sources {
        assert_eq!(source.id.0, expected_source_id);
    }
}

fn find_continue_action(delta: &MediaGraphDelta) -> Value {
    let action = delta
        .actions
        .iter()
        .find(|action| action.label == "豪门")
        .expect("Discover should expose the class continuation action");
    assert_eq!(action.intent, StandardIntent::ContinueAction);
    assert_eq!(action.payload["schema_version"], 1);
    assert!(action.payload["source_identity"].as_str().is_some());
    assert!(action.payload["action_identity"].as_str().is_some());
    assert!(action.payload["integrity"].as_str().is_some());
    action.payload.clone()
}

fn assert_no_plaintext_secret(root: &Path, secret: &[u8]) {
    let files = collect_files(root);
    assert!(
        files.iter().all(|file| {
            fs::read(file).map_or(true, |bytes| {
                !bytes.windows(secret.len()).any(|window| window == secret)
            })
        }),
        "secret must not appear in SQLite, artifact body/metadata, or encrypted artifact files"
    );
}

fn collect_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_files(&path));
        } else {
            files.push(path);
        }
    }
    files
}

fn body_artifact_path(root: &Path, hash: &str) -> PathBuf {
    root.join("artifacts")
        .join("body")
        .join(&hash[..2])
        .join(&hash[2..4])
        .join(format!("{hash}.zst"))
}

fn captured_body_artifact_path(root: &Path, events: &[ExecutionEvent]) -> PathBuf {
    events
        .iter()
        .find_map(|event| match &event.kind {
            ExecutionEventKind::EffectCaptured { artifact_refs, .. } => artifact_refs
                .iter()
                .map(|artifact| body_artifact_path(root, &artifact.hash))
                .find(|path| path.is_file()),
            _ => None,
        })
        .expect("live effect must expose a durable body artifact reference")
}

async fn captured_witnesses(
    system: &RuleSystem,
    execution_id: ExecutionId,
    events: &[ExecutionEvent],
) -> Vec<EffectWitnessCaptureForTest> {
    let mut captures = Vec::new();
    for event in events {
        let ExecutionEventKind::EffectCaptured { effect_id, .. } = &event.kind else {
            continue;
        };
        captures.push(
            system
                .read_effect_witness_for_test(execution_id, *effect_id)
                .await
                .expect("live archive should expose a verified redacted witness"),
        );
    }
    captures
}

fn assert_blake3_hex(value: &str) {
    assert_eq!(value.len(), 64, "witness hash must be BLAKE3 hex");
    assert!(
        value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "witness hash must be BLAKE3 hex"
    );
}

fn assert_safe_witness_url(value: &str) {
    assert!(
        value.starts_with("http://") || value.starts_with("https://"),
        "witness URL must preserve an HTTP(S) origin"
    );
    assert!(
        !value.contains(['?', '#', '@']),
        "witness URL must not expose query, fragment, or userinfo: {value}"
    );
}

async fn assert_live_http_and_quickjs_witnesses(
    system: &RuleSystem,
    search: &LiveExecution,
    discover: &LiveExecution,
) {
    let search_captures = captured_witnesses(system, search.id, &search.events).await;
    for capture in &search_captures {
        assert_blake3_hex(&capture.witness_hash);
    }
    let http = search_captures
        .into_iter()
        .find_map(|capture| match capture.witness {
            EffectWitnessForTest::Http(witness) => Some(witness),
            EffectWitnessForTest::QuickJs(_) | EffectWitnessForTest::Extract(_) => None,
        })
        .expect("Search must archive an HTTP witness");
    assert_eq!(http.error, None);
    assert_eq!(http.request.method, HttpMethodForTest::Get);
    assert_safe_witness_url(&http.request.safe_url);
    assert!(http.request.safe_url.ends_with("/search"));
    assert_eq!(http.request.body, None, "GET fixture has no request body");
    assert_eq!(http.request.headers.len(), 1);
    let header = &http.request.headers[0];
    assert_eq!(header.name, "user-agent");
    assert_blake3_hex(&header.value_hash);
    assert!(
        http.request.headers.iter().all(|header| {
            !matches!(
                header.name.as_str(),
                "authorization" | "cookie" | "set-cookie" | "proxy-authorization"
            ) && !header.name.contains("token")
        }),
        "credential headers must not enter the safe witness"
    );
    assert_eq!(http.redirects.len(), 1);
    let redirect = &http.redirects[0];
    assert_eq!(redirect.status, 302);
    assert_safe_witness_url(&redirect.from_url);
    assert_safe_witness_url(&redirect.to_url);
    assert!(redirect.from_url.ends_with("/search"));
    assert!(redirect.to_url.ends_with("/search-result"));
    assert_eq!(http.dns_targets.len(), 2);
    assert!(http.dns_targets.iter().all(|target| {
        target.kind == HttpDnsTargetKindForTest::IpLiteral
            && target.addresses.len() == 1
            && !target.host.is_empty()
    }));
    let safe_http = serde_json::to_string(&http).expect("serialize safe HTTP witness");
    for forbidden in [
        SOURCE_STATIC_SECRET,
        "response-capture-secret",
        SEARCH_QUERY_FRAGMENT,
    ] {
        assert!(
            !safe_http.contains(forbidden),
            "safe HTTP witness leaked {forbidden}"
        );
    }

    let discover_captures = captured_witnesses(system, discover.id, &discover.events).await;
    for capture in &discover_captures {
        assert_blake3_hex(&capture.witness_hash);
    }
    let quickjs = discover_captures
        .into_iter()
        .find_map(|capture| match capture.witness {
            EffectWitnessForTest::QuickJs(witness) => Some(witness),
            EffectWitnessForTest::Http(_) | EffectWitnessForTest::Extract(_) => None,
        })
        .expect("Discover must archive a QuickJS witness");
    assert_eq!(quickjs.error, None);
    for hash in [
        &quickjs.script_hash,
        &quickjs.input_hash,
        &quickjs.output_hash,
    ] {
        assert_blake3_hex(hash);
    }
    assert_eq!(quickjs.host_calls.len(), 2);
    assert_eq!(quickjs.host_calls[0].sequence, 1);
    assert_eq!(quickjs.host_calls[1].sequence, 2);
    assert!(matches!(
        quickjs.host_calls[0].call,
        QuickJsHostCallForTest::Time { .. }
    ));
    assert!(matches!(
        quickjs.host_calls[1].call,
        QuickJsHostCallForTest::Random { .. }
    ));
    let safe_quickjs = serde_json::to_string(&quickjs).expect("serialize safe QuickJS witness");
    for forbidden in ["Date.now()", "Math.random()", "豪门测试书"] {
        assert!(
            !safe_quickjs.contains(forbidden),
            "safe QuickJS witness leaked {forbidden}"
        );
    }
}

#[tokio::test]
async fn legado_six_intents_live_and_offline_replay_are_equivalent_and_secure() {
    init_mock_keyring();
    let temp = TempRuleSystem::new("six-intents");
    let server = MockServer::start().await;
    mount_legado_routes(&server).await;
    let system = temp.open().await;
    let source = system
        .prepare_install(legado_input(&server.uri()))
        .await
        .expect("Legado candidate should be durable and opaque");
    let wire = serde_json::to_value(&source).expect("candidate should serialize safely");
    for forbidden in ["definition", "package", "plan", "graph", "source_json"] {
        assert!(
            wire.get(forbidden).is_none(),
            "candidate leaked {forbidden}"
        );
    }
    let installed = system
        .install(source.id, CapabilityGrant::network_only())
        .await
        .expect("network grant should install Legado source");

    let search = execute_live(
        &system,
        &installed.source_id,
        StandardIntent::Search,
        IntentInput::Query("修罗".to_string()),
    )
    .await;
    let item_id = find_item(&search.delta, "修罗武神");
    assert_standard_source_ownership(&search.delta, &installed.profile.id.0, None, None);

    let discover = execute_live(
        &system,
        &installed.source_id,
        StandardIntent::Discover,
        IntentInput::None,
    )
    .await;
    let continue_action = find_continue_action(&discover.delta);
    assert_standard_source_ownership(&discover.delta, &installed.profile.id.0, None, None);

    let resolve_item = execute_live(
        &system,
        &installed.source_id,
        StandardIntent::ResolveItem,
        IntentInput::ItemId(item_id.clone()),
    )
    .await;
    let resolved_item_id = find_item(&resolve_item.delta, "修罗武神");
    assert_standard_source_ownership(&resolve_item.delta, &installed.profile.id.0, None, None);

    let units = execute_live(
        &system,
        &installed.source_id,
        StandardIntent::ListUnits,
        IntentInput::ItemId(resolved_item_id.clone()),
    )
    .await;
    let unit_id = find_unit(&units.delta);
    assert_standard_source_ownership(
        &units.delta,
        &installed.profile.id.0,
        Some(&resolved_item_id),
        None,
    );

    let asset = execute_live(
        &system,
        &installed.source_id,
        StandardIntent::ResolveAsset,
        IntentInput::UnitId(unit_id.clone()),
    )
    .await;
    assert_text_asset(&asset.delta);
    assert_standard_source_ownership(&asset.delta, &installed.profile.id.0, None, Some(&unit_id));

    let continued = execute_live(
        &system,
        &installed.source_id,
        StandardIntent::ContinueAction,
        IntentInput::Opaque(continue_action.clone()),
    )
    .await;
    find_item(&continued.delta, "豪门测试书");
    assert_standard_source_ownership(&continued.delta, &installed.profile.id.0, None, None);

    assert_live_http_and_quickjs_witnesses(&system, &search, &discover).await;

    let safe_events =
        serde_json::to_string(&search.events).expect("serialize safe delivery events");
    assert!(!safe_events.contains("response-capture-secret"));
    assert!(!safe_events.contains(SOURCE_STATIC_SECRET));
    assert!(!safe_events.contains(SEARCH_QUERY_FRAGMENT));
    assert_no_plaintext_secret(&temp.root, b"response-capture-secret");
    assert_no_plaintext_secret(&temp.root, SOURCE_STATIC_SECRET.as_bytes());
    assert_no_plaintext_secret(&temp.root, SEARCH_QUERY_FRAGMENT.as_bytes());

    drop(server);
    for (intent, input, live) in [
        (
            StandardIntent::Search,
            IntentInput::Query("修罗".to_string()),
            &search,
        ),
        (StandardIntent::Discover, IntentInput::None, &discover),
        (
            StandardIntent::ResolveItem,
            IntentInput::ItemId(item_id.clone()),
            &resolve_item,
        ),
        (
            StandardIntent::ListUnits,
            IntentInput::ItemId(item_id.clone()),
            &units,
        ),
        (
            StandardIntent::ResolveAsset,
            IntentInput::UnitId(find_unit(&units.delta)),
            &asset,
        ),
        (
            StandardIntent::ContinueAction,
            IntentInput::Opaque(continue_action.clone()),
            &continued,
        ),
    ] {
        let replay = execute_replay(&system, &installed.source_id, intent, input, live.id).await;
        assert_completed(&replay, false);
        assert_eq!(
            committed_delta(&replay),
            &live.delta,
            "{intent:?} replay Delta must equal live Delta without network fallback"
        );
    }

    let mismatched = execute_replay(
        &system,
        &installed.source_id,
        StandardIntent::Search,
        IntentInput::Query("different input".to_string()),
        search.id,
    )
    .await;
    assert_contiguous(&mismatched);
    assert!(
        mismatched.iter().any(|event| {
            matches!(
                &event.kind,
                ExecutionEventKind::Diagnostic { code, .. } if code == "replay_fingerprint_mismatch"
            )
        }),
        "different replay input must emit an explicit fingerprint diagnostic: {mismatched:#?}"
    );
    assert!(matches!(
        mismatched.last().map(|event| &event.kind),
        Some(ExecutionEventKind::Failed { .. })
    ));
}

#[tokio::test]
async fn legado_replay_refuses_lost_master_key_before_effects() {
    init_mock_keyring();
    let temp = TempRuleSystem::new("replay-key-loss");
    let server = MockServer::start().await;
    mount_legado_routes(&server).await;
    let system = temp.open().await;
    let candidate = system
        .prepare_install(legado_input(&server.uri()))
        .await
        .expect("credential-bearing Legado candidate");
    let installed = system
        .install(candidate.id, CapabilityGrant::network_only())
        .await
        .expect("credential-bearing Legado source");
    let live = execute_live(
        &system,
        &installed.source_id,
        StandardIntent::Search,
        IntentInput::Query("修罗".to_string()),
    )
    .await;

    drop(server);
    system
        .shutdown_for_test()
        .await
        .expect("close C2 writer before key-loss restart");
    drop(system);
    let restarted = temp.open().await;

    let Err(error) = restarted
        .execute(ExecuteRequest {
            source_id: installed.source_id,
            intent: StandardIntent::Search,
            input: IntentInput::Query("修罗".to_string()),
            mode: ExecutionMode::Replay {
                execution_id: live.id,
            },
        })
        .await
    else {
        panic!("replay must reject a lost C2 master key before effects");
    };
    assert_eq!(error.stage, RuleErrorStage::Replay);
    assert_eq!(error.code, "replay_pin_unavailable");
}

#[tokio::test]
async fn legado_replay_refuses_tampered_effect_body_without_live_fallback() {
    init_mock_keyring();
    let temp = TempRuleSystem::new("tampered-effect-body");
    let server = MockServer::start().await;
    mount_legado_routes(&server).await;
    let system = temp.open().await;
    let candidate = system
        .prepare_install(legado_input(&server.uri()))
        .await
        .expect("credential-bearing Legado candidate");
    let installed = system
        .install(candidate.id, CapabilityGrant::network_only())
        .await
        .expect("credential-bearing Legado source");
    let live = execute_live(
        &system,
        &installed.source_id,
        StandardIntent::Search,
        IntentInput::Query("修罗".to_string()),
    )
    .await;
    let artifact = captured_body_artifact_path(&temp.root, &live.events);
    fs::write(&artifact, b"tampered capture body").expect("corrupt captured body artifact");

    drop(server);
    let replay = execute_replay(
        &system,
        &installed.source_id,
        StandardIntent::Search,
        IntentInput::Query("修罗".to_string()),
        live.id,
    )
    .await;
    assert_contiguous(&replay);
    assert!(
        replay.iter().any(|event| {
            matches!(
                &event.kind,
                ExecutionEventKind::Diagnostic { code, .. } if code == "replay_capture_missing"
            )
        }),
        "tampered body must produce an explicit replay failure: {replay:#?}"
    );
    assert!(matches!(
        replay.last().map(|event| &event.kind),
        Some(ExecutionEventKind::Failed { .. })
    ));
    assert!(
        replay
            .iter()
            .all(|event| !matches!(event.kind, ExecutionEventKind::EffectCaptured { .. })),
        "tampered replay must never fall back to a live effect"
    );
}

#[tokio::test]
async fn legado_replay_refuses_missing_effect_secret_without_live_fallback() {
    init_mock_keyring();
    let temp = TempRuleSystem::new("missing-effect-secret");
    let server = MockServer::start().await;
    mount_legado_routes(&server).await;
    mount_credential_free_search_redirect(&server).await;
    let system = temp.open().await;
    let candidate = system
        .prepare_install(legado_input_without_source_credentials(&server.uri()))
        .await
        .expect("credential-free Legado candidate");
    let installed = system
        .install(candidate.id, CapabilityGrant::network_only())
        .await
        .expect("credential-free Legado source");
    let live = execute_live(
        &system,
        &installed.source_id,
        StandardIntent::Search,
        IntentInput::Query("修罗".to_string()),
    )
    .await;
    let secret_files = collect_files(&temp.root.join("artifacts").join("secret"));
    assert_eq!(
        secret_files.len(),
        1,
        "only the response secret should exist without source credentials"
    );
    fs::remove_file(&secret_files[0]).expect("remove captured response secret artifact");

    drop(server);
    let replay = execute_replay(
        &system,
        &installed.source_id,
        StandardIntent::Search,
        IntentInput::Query("修罗".to_string()),
        live.id,
    )
    .await;
    assert_contiguous(&replay);
    assert!(
        replay.iter().any(|event| {
            matches!(
                &event.kind,
                ExecutionEventKind::Diagnostic { code, .. } if code == "replay_capture_missing"
            )
        }),
        "missing secret must produce an explicit replay failure: {replay:#?}"
    );
    assert!(matches!(
        replay.last().map(|event| &event.kind),
        Some(ExecutionEventKind::Failed { .. })
    ));
    assert!(
        replay
            .iter()
            .all(|event| !matches!(event.kind, ExecutionEventKind::EffectCaptured { .. })),
        "missing secret replay must never fall back to a live effect"
    );
}
#[tokio::test]
async fn legado_continue_action_rejects_cross_source_schema_and_expiry_before_effects() {
    init_mock_keyring();
    let temp = TempRuleSystem::new("continue-action");
    let server = MockServer::start().await;
    mount_legado_routes(&server).await;

    let system = temp.open().await;
    let first = system
        .prepare_install(legado_input(&server.uri()))
        .await
        .expect("first Legado candidate");
    let first = system
        .install(first.id, CapabilityGrant::network_only())
        .await
        .expect("first Legado source");
    let discover = execute_live(
        &system,
        &first.source_id,
        StandardIntent::Discover,
        IntentInput::None,
    )
    .await;
    let action = find_continue_action(&discover.delta);

    let mut invalid_integrity = action.clone();
    invalid_integrity["integrity"] = json!("0".repeat(64));
    let Err(invalid_integrity) = system
        .execute(ExecuteRequest {
            source_id: first.source_id.clone(),
            intent: StandardIntent::ContinueAction,
            input: IntentInput::Opaque(invalid_integrity),
            mode: ExecutionMode::Live,
        })
        .await
    else {
        panic!("tampered action integrity must fail before effects");
    };
    assert_eq!(invalid_integrity.code, "continue_action_integrity_invalid");

    let second_candidate = system
        .prepare_install(legado_input(&format!("{}/second", server.uri())))
        .await
        .expect("second distinct Legado candidate");
    let second = system
        .install(second_candidate.id, CapabilityGrant::network_only())
        .await
        .expect("second Legado source");
    let Err(cross_source) = system
        .execute(ExecuteRequest {
            source_id: second.source_id,
            intent: StandardIntent::ContinueAction,
            input: IntentInput::Opaque(action.clone()),
            mode: ExecutionMode::Live,
        })
        .await
    else {
        panic!("action must not cross source ownership");
    };
    assert_eq!(cross_source.stage, RuleErrorStage::Execution);
    assert_eq!(cross_source.code, "continue_action_source_mismatch");

    let mut wrong_schema = action.clone();
    wrong_schema["schema_version"] = json!(2);
    let Err(wrong_schema) = system
        .execute(ExecuteRequest {
            source_id: first.source_id.clone(),
            intent: StandardIntent::ContinueAction,
            input: IntentInput::Opaque(wrong_schema),
            mode: ExecutionMode::Live,
        })
        .await
    else {
        panic!("unsupported action schema must fail before effects");
    };
    assert_eq!(wrong_schema.code, "continue_action_schema_unsupported");

    let source_identity = first.profile.id.0.clone();
    let expired = LegadoImporter::seal_continue_action_payload(
        &json!({"title": "过期", "url": "/haomenzongcai?page=1"}),
        &source_identity,
        1_750_000_000_000_i64 - CONTINUE_ACTION_TTL_MS - 1,
    )
    .expect("construct expired source-owned action");
    let Err(expired) = system
        .execute(ExecuteRequest {
            source_id: first.source_id,
            intent: StandardIntent::ContinueAction,
            input: IntentInput::Opaque(expired),
            mode: ExecutionMode::Live,
        })
        .await
    else {
        panic!("expired action must fail before effects");
    };
    assert_eq!(expired.code, "continue_action_expired");
}
