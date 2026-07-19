//! Maccms JSON 通过 concrete `RuleSystem` 的生命周期集成合同。
//!
//! 本测试只构造真实 SQLite/artifact、wiremock 与 concrete façade；不组装内部执行编排、
//! handler registry 或 storage transaction。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::Duration;

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Text};
use futures::StreamExt;
use keyring::{mock, set_default_credential_builder};
use lj_capability::{IntentInput, StandardIntent};
use lj_media::{MediaAssetKind, MediaAssetLocator, MediaGraphDelta, MediaKind};
use lj_rule_system::{
    CapabilityGrant, ExecuteRequest, ExecutionEventKind, ExecutionMode, InstallCandidate,
    LibraryEntryUpdate, LibraryProgress, RuleErrorStage, RuleInput, RuleSystem, RuleSystemConfig,
};
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct TempRuleSystem {
    root: PathBuf,
    keyring_service: String,
}

impl TempRuleSystem {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("lj-rule-system-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("创建 RuleSystem 测试根目录");
        let keyring_service = format!("lanjing.rule-system.test.{}", Uuid::new_v4());
        Self {
            root,
            keyring_service,
        }
    }

    fn database_path(&self) -> PathBuf {
        self.root.join("event-store.db")
    }

    async fn open(&self, candidate_ttl: Duration) -> RuleSystem {
        self.open_result(candidate_ttl)
            .await
            .expect("打开 concrete RuleSystem")
    }

    async fn reopen_after_drop(&self, candidate_ttl: Duration) -> RuleSystem {
        for attempt in 0..20 {
            match self.open_result(candidate_ttl).await {
                Ok(system) => return system,
                Err(error) if error.stage == RuleErrorStage::Persistence && attempt < 19 => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => panic!("同进程 storage writer 退出后无法重开 RuleSystem: {error:?}"),
            }
        }
        unreachable!("有界重开循环应在成功或最终错误时退出")
    }

    async fn open_result(
        &self,
        candidate_ttl: Duration,
    ) -> Result<RuleSystem, lj_rule_system::RuleError> {
        RuleSystem::open(
            RuleSystemConfig::local_fixture(
                self.root.join("event-store.db"),
                self.root.join("artifacts"),
            )
            .with_keyring_service(self.keyring_service.clone())
            .with_candidate_ttl(candidate_ttl),
        )
        .await
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

#[derive(Clone, Copy)]
enum CandidateTamper {
    Profile,
    RequiredGrant,
    Expiry,
    DefinitionHash,
    Diagnostics,
    ArtifactAlgorithm,
}

fn tamper_candidate_row(path: &Path, candidate: &InstallCandidate, tamper: CandidateTamper) {
    let mut connection = diesel::sqlite::SqliteConnection::establish(
        path.to_str().expect("测试 SQLite 路径必须是 UTF-8"),
    )
    .expect("打开真实 candidate SQLite 以注入篡改");
    let candidate_id = serde_json::to_string(&candidate.id)
        .expect("candidate ID 可序列化")
        .trim_matches('"')
        .to_string();
    let changed = match tamper {
        CandidateTamper::Profile => {
            let mut profile = candidate.profile.clone();
            profile.id = lj_media::MediaResourceId("source:tampered".to_string());
            let profile_json = serde_json::to_string(&profile).expect("profile 可序列化");
            sql_query("UPDATE candidates SET profile_json = ? WHERE candidate_id = ?")
                .bind::<Text, _>(&profile_json)
                .bind::<Text, _>(&candidate_id)
                .execute(&mut connection)
        }
        CandidateTamper::RequiredGrant => {
            let grant_json = serde_json::to_string(&CapabilityGrant::none())
                .expect("grant 可序列化");
            sql_query("UPDATE candidates SET required_grant_json = ? WHERE candidate_id = ?")
                .bind::<Text, _>(&grant_json)
                .bind::<Text, _>(&candidate_id)
                .execute(&mut connection)
        }
        CandidateTamper::Diagnostics => {
            let diagnostics_json = serde_json::to_string(&serde_json::json!([
                {
                    "code": "tampered",
                    "severity": "warning",
                    "message": "tampered"
                }
            ]))
            .expect("diagnostics 可序列化");
            sql_query("UPDATE candidates SET diagnostics_json = ? WHERE candidate_id = ?")
                .bind::<Text, _>(&diagnostics_json)
                .bind::<Text, _>(&candidate_id)
                .execute(&mut connection)
        }
        CandidateTamper::ArtifactAlgorithm => sql_query(
            "UPDATE events SET artifact_refs_json = json_set(artifact_refs_json, '$[0].codec', ?) WHERE stream_id = ?",
        )
        .bind::<Text, _>("sha256")
        .bind::<Text, _>(&format!("candidate/{candidate_id}"))
        .execute(&mut connection),
        CandidateTamper::Expiry => sql_query(
            "UPDATE candidates SET expires_at_ms = ? WHERE candidate_id = ?",
        )
        .bind::<BigInt, _>(candidate.expires_at_ms.saturating_add(60_000))
        .bind::<Text, _>(&candidate_id)
        .execute(&mut connection),
        CandidateTamper::DefinitionHash => sql_query(
            "UPDATE candidates SET definition_hash = ? WHERE candidate_id = ?",
        )
        .bind::<Text, _>(&"0".repeat(64))
        .bind::<Text, _>(&candidate_id)
        .execute(&mut connection),
    }
    .expect("注入 candidate SQLite 篡改");
    assert_eq!(changed, 1, "必须只篡改一个 candidate row");
}

fn maccms_json_input(base_url: &str) -> RuleInput {
    RuleInput::MaccmsJson {
        url: format!("{base_url}/api.php/provide/vod/"),
    }
}

async fn mount_maccms_json_routes(server: &MockServer, list_delay: Option<Duration>) {
    let mut list_response = ResponseTemplate::new(200).set_body_json(json!({
        "code": 1,
        "page": 1,
        "pagecount": 1,
        "limit": 20,
        "total": 1,
        "list": [
            {
                "vod_id": 140_789,
                "vod_name": "爱情没有神话",
                "vod_pic": "/covers/140789.jpg",
                "type_name": "国产剧",
                "vod_remarks": "第02集"
            }
        ]
    }));
    if let Some(delay) = list_delay {
        list_response = list_response.set_delay(delay);
    }
    Mock::given(method("GET"))
        .and(path("/api.php/provide/vod/"))
        .and(query_param("ac", "list"))
        .respond_with(list_response)
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api.php/provide/vod/"))
        .and(query_param("ac", "detail"))
        .and(query_param("ids", "140789"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": 1,
            "list": [
                {
                    "vod_id": 140_789,
                    "vod_name": "爱情没有神话",
                    "vod_pic": "/covers/140789.jpg",
                    "type_name": "国产剧",
                    "vod_remarks": "第02集",
                    "vod_content": "一段本地 mock 的剧情简介。",
                    "vod_play_from": "hnyun,hnm3u8",
                    "vod_play_url": "第1集$mock-stream://json/1.m3u8#第2集$mock-stream://json/2.m3u8###正片$mock-stream://json/main.mp4"
                }
            ]
        })))
        .mount(server)
        .await;
}

fn committed_delta(events: &[lj_rule_system::ExecutionEvent]) -> &MediaGraphDelta {
    events
        .iter()
        .find_map(|event| match &event.kind {
            ExecutionEventKind::DeltaCommitted { delta, .. } => Some(delta),
            ExecutionEventKind::Started
            | ExecutionEventKind::Diagnostic { .. }
            | ExecutionEventKind::EffectCaptured { .. }
            | ExecutionEventKind::Completed
            | ExecutionEventKind::Failed { .. }
            | ExecutionEventKind::Cancelled => None,
        })
        .expect("execution 应在终态前 delivery 已提交的 MediaGraphDelta")
}

fn assert_completed(events: &[lj_rule_system::ExecutionEvent], require_effect_capture: bool) {
    assert_contiguous(events);
    assert!(
        matches!(
            events.last().map(|event| &event.kind),
            Some(ExecutionEventKind::Completed)
        ),
        "session 必须以唯一 Completed 终态结束: {events:?}"
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| is_terminal(&event.kind))
            .count(),
        1,
        "session 必须只有一个终态: {events:?}"
    );
    let commit_index = events
        .iter()
        .position(|event| matches!(event.kind, ExecutionEventKind::DeltaCommitted { .. }))
        .expect("必须存在 commit 后的 Delta delivery");
    assert!(
        commit_index + 1 < events.len(),
        "DeltaCommitted 必须发生在终态前"
    );
    if require_effect_capture {
        let captured_before_commit = events[..commit_index]
            .iter()
            .filter(|event| matches!(&event.kind, ExecutionEventKind::EffectCaptured { .. }))
            .count();
        assert!(
            captured_before_commit > 0,
            "live Delta 前必须先持久化至少一个 effect archive"
        );
        for event in &events[..commit_index] {
            if let ExecutionEventKind::EffectCaptured {
                artifact_refs,
                output_hash,
                ..
            } = &event.kind
            {
                assert!(
                    !artifact_refs.is_empty(),
                    "live effect archive 必须有 durable artifact 引用"
                );
                assert_eq!(output_hash.len(), 64, "live effect 输出必须带 BLAKE3 hex");
            }
        }
        assert!(
            events[commit_index..]
                .iter()
                .all(|event| !matches!(&event.kind, ExecutionEventKind::EffectCaptured { .. })),
            "effect archive 必须在 DeltaCommitted 前持久化"
        );
    }
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

fn assert_delta_source_ownership(delta: &MediaGraphDelta, source_id: &lj_media::MediaResourceId) {
    assert!(
        delta.sources.iter().any(|profile| &profile.id == source_id),
        "Delta 必须包含已安装来源的 profile"
    );
    assert!(
        delta.sources.iter().all(|profile| &profile.id == source_id),
        "Delta 中的 source profile 必须属于已安装来源"
    );
    assert!(
        delta.items.iter().all(|item| &item.source_id == source_id),
        "Delta 中的 item 必须属于已安装来源"
    );
    assert!(
        delta
            .collections
            .iter()
            .all(|collection| &collection.source_id == source_id),
        "Delta 中的 collection 必须属于已安装来源"
    );
    assert!(
        delta.units.iter().all(|unit| &unit.source_id == source_id),
        "Delta 中的 unit 必须属于已安装来源"
    );
    assert!(
        delta
            .assets
            .iter()
            .all(|asset| &asset.source_id == source_id),
        "Delta 中的 asset 必须属于已安装来源"
    );
    assert!(
        delta
            .relations
            .iter()
            .all(|relation| &relation.source_id == source_id),
        "Delta 中的 relation 必须属于已安装来源"
    );
    assert!(
        delta
            .actions
            .iter()
            .all(|action| &action.source_id == source_id),
        "Delta 中的 action 必须属于已安装来源"
    );
}

fn is_terminal(kind: &ExecutionEventKind) -> bool {
    matches!(
        kind,
        ExecutionEventKind::Completed
            | ExecutionEventKind::Failed { .. }
            | ExecutionEventKind::Cancelled
    )
}

struct ExecutionDelta {
    execution_id: lj_rule_system::ExecutionId,
    delta: MediaGraphDelta,
}

async fn execute_live(
    system: &RuleSystem,
    source_id: &lj_rule_system::SourceId,
    intent: StandardIntent,
    input: IntentInput,
) -> ExecutionDelta {
    let session = system
        .execute(ExecuteRequest {
            source_id: source_id.clone(),
            intent,
            input,
            mode: ExecutionMode::Live,
        })
        .await
        .expect("已安装 source ID 应足以启动 live execution");
    let execution_id = session.id;
    let events = session.into_events().collect::<Vec<_>>().await;
    assert_completed(&events, true);
    ExecutionDelta {
        execution_id,
        delta: committed_delta(&events).clone(),
    }
}

async fn execute_replay(
    system: &RuleSystem,
    source_id: &lj_rule_system::SourceId,
    intent: StandardIntent,
    input: IntentInput,
    execution_id: lj_rule_system::ExecutionId,
) -> MediaGraphDelta {
    let events = system
        .execute(ExecuteRequest {
            source_id: source_id.clone(),
            intent,
            input,
            mode: ExecutionMode::Replay { execution_id },
        })
        .await
        .expect("断网 replay 必须从 execution pin 的 archive 启动")
        .into_events()
        .collect::<Vec<_>>()
        .await;
    assert_completed(&events, false);
    committed_delta(&events).clone()
}

fn assert_discover_delta(delta: &MediaGraphDelta, source_id: &lj_media::MediaResourceId) -> String {
    assert_delta_source_ownership(delta, source_id);
    let item = delta
        .items
        .iter()
        .find(|item| item.title == "爱情没有神话")
        .expect("Discover 应提交视频媒体主体");
    assert_eq!(item.media_kind, MediaKind::Video);
    assert_eq!(item.subtitle.as_deref(), Some("第02集"));
    assert_eq!(item.metadata["source_item_id"], "140789");
    assert_eq!(
        &item.source_id, source_id,
        "Discover item 必须属于已安装来源"
    );
    let collection = delta
        .collections
        .iter()
        .find(|collection| collection.item_ids.contains(&item.id))
        .expect("Discover 应提交包含媒体主体的集合");
    assert_eq!(
        &collection.source_id, source_id,
        "Discover collection 必须属于已安装来源"
    );
    item.id.0.clone()
}

fn assert_resolve_item_delta(delta: &MediaGraphDelta, source_id: &lj_media::MediaResourceId) {
    assert_delta_source_ownership(delta, source_id);
    let item = delta
        .items
        .iter()
        .find(|item| {
            item.title == "爱情没有神话"
                && item.description.as_deref() == Some("一段本地 mock 的剧情简介。")
        })
        .expect("ResolveItem 应提交完整媒体主体");
    assert_eq!(
        &item.source_id, source_id,
        "ResolveItem item 必须属于已安装来源"
    );
    let asset = delta
        .assets
        .iter()
        .find(|asset| {
            asset.asset_kind == MediaAssetKind::VideoStream
                && matches!(&asset.locator, MediaAssetLocator::Url(url) if url.ends_with("/json/1.m3u8"))
        })
        .expect("ResolveItem 应提交播放资产");
    assert_eq!(
        &asset.source_id, source_id,
        "ResolveItem asset 必须属于已安装来源"
    );
}

fn assert_list_units_delta(
    delta: &MediaGraphDelta,
    source_id: &lj_media::MediaResourceId,
) -> String {
    assert_delta_source_ownership(delta, source_id);
    let first_unit = delta
        .units
        .iter()
        .find(|unit| unit.title == "第1集")
        .expect("ListUnits 应提交播放单元");
    assert_eq!(first_unit.metadata["line"], "hnyun");
    assert_eq!(
        &first_unit.source_id, source_id,
        "ListUnits unit 必须属于已安装来源"
    );
    assert!(delta.assets.is_empty(), "ListUnits 不得混入资产");
    first_unit.id.0.clone()
}

fn assert_resolve_asset_delta(
    delta: &MediaGraphDelta,
    source_id: &lj_media::MediaResourceId,
    unit_id: &str,
) {
    assert_delta_source_ownership(delta, source_id);
    let asset = delta
        .assets
        .iter()
        .find(|asset| {
            asset.asset_kind == MediaAssetKind::VideoStream
                && asset.unit_id.as_ref().map(|id| id.0.as_str()) == Some(unit_id)
                && matches!(&asset.locator, MediaAssetLocator::Url(url) if url.ends_with("/json/1.m3u8"))
        })
        .expect("ResolveAsset 应只提交绑定给请求 unit 的视频流");
    assert_eq!(
        &asset.source_id, source_id,
        "ResolveAsset asset 必须属于已安装来源"
    );
}

async fn catch_up_until_terminal(
    session: &lj_rule_system::ExecutionSession,
) -> Vec<lj_rule_system::ExecutionEvent> {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let events = session
                .catch_up(0)
                .await
                .expect("cancelled execution 可从 C2 stream catch-up");
            if events.last().is_some_and(|event| is_terminal(&event.kind)) {
                return events;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("取消后的持久终态必须在超时内可 catch-up")
}

#[tokio::test]
async fn candidate_boundary_is_opaque_and_rejects_tampering_expiry_and_insufficient_grant() {
    init_mock_keyring();
    let temp = TempRuleSystem::new("candidate-boundary");
    let system = temp.open(Duration::from_mins(1)).await;
    let input = maccms_json_input("https://fixture.example");

    let candidate = system
        .prepare_install(input.clone())
        .await
        .expect("Maccms JSON 应生成 durable candidate");
    let wire = serde_json::to_value(&candidate).expect("candidate 可作为外部安全 DTO 序列化");
    for forbidden in ["definition", "package", "plan", "graph"] {
        assert!(
            wire.get(forbidden).is_none(),
            "prepare_install 不得向调用方泄露 {forbidden}"
        );
    }
    assert!(
        candidate
            .profile
            .supported_intents
            .contains(&StandardIntent::Discover)
    );
    assert!(candidate.required_grant.requires_network());
    assert_eq!(candidate.definition_hash.len(), 64);
    assert_eq!(candidate.plan_hash.len(), 64);
    assert_eq!(
        candidate.profile.version.as_deref(),
        Some(candidate.definition_hash.as_str()),
        "candidate profile version 必须绑定 Definition hash",
    );
    let expected_source_version = candidate.definition_hash.clone();
    drop(system);
    let system = temp.reopen_after_drop(Duration::from_mins(1)).await;

    let tampered_id = serde_json::from_value(json!(Uuid::new_v4().to_string()))
        .expect("opaque candidate ID 的 wire 值可被外部反序列化");
    let tampered = system
        .install(tampered_id, CapabilityGrant::network_only())
        .await
        .expect_err("篡改 candidate token 必须被拒绝");
    assert_eq!(tampered.stage, RuleErrorStage::Candidate);

    let insufficient = system
        .install(candidate.id.clone(), CapabilityGrant::none())
        .await
        .expect_err("缺少 network grant 的 Maccms candidate 不得安装");
    assert_eq!(insufficient.stage, RuleErrorStage::Capability);

    let consumed_candidate_id = candidate.id.clone();
    let first = system
        .install(candidate.id, CapabilityGrant::network_only())
        .await
        .expect("有效 candidate 应安装");
    assert_eq!(first.version, expected_source_version);
    let consumed = system
        .install(consumed_candidate_id, CapabilityGrant::network_only())
        .await
        .expect_err("已消费 candidate 不得再次安装");
    assert_eq!(consumed.stage, RuleErrorStage::Candidate);
    let replacement = system
        .prepare_install(input)
        .await
        .expect("同 identity 可准备新版本");
    let second = system
        .install(replacement.id, CapabilityGrant::network_only())
        .await
        .expect("同 identity 安装应追加 source revision");
    assert_eq!(second.version, first.version);
    assert_eq!(first.source_id, second.source_id, "来源 identity 必须稳定");
    assert!(
        second.revision > first.revision,
        "同一 stable identity 的更新必须形成更高 source revision"
    );
    let expiry_temp = TempRuleSystem::new("candidate-expiry");
    let expiry_system = expiry_temp.open(Duration::from_millis(30)).await;
    let expired = expiry_system
        .prepare_install(maccms_json_input("https://expiry.example"))
        .await
        .expect("可创建待过期 candidate");
    tokio::time::sleep(Duration::from_millis(40)).await;
    let expiry_error = expiry_system
        .install(expired.id, CapabilityGrant::network_only())
        .await
        .expect_err("过期 candidate 不得安装");
    assert_eq!(expiry_error.stage, RuleErrorStage::Candidate);
}

#[tokio::test]
async fn candidate_install_revalidates_event_metadata_after_restart() {
    init_mock_keyring();
    for (name, tamper) in [
        ("profile", CandidateTamper::Profile),
        ("grant", CandidateTamper::RequiredGrant),
        ("expiry", CandidateTamper::Expiry),
        ("definition-hash", CandidateTamper::DefinitionHash),
        ("diagnostics", CandidateTamper::Diagnostics),
        ("artifact-algorithm", CandidateTamper::ArtifactAlgorithm),
    ] {
        let temp = TempRuleSystem::new(&format!("candidate-tamper-{name}"));
        let system = temp.open(Duration::from_mins(1)).await;
        let candidate = system
            .prepare_install(maccms_json_input("https://fixture.example"))
            .await
            .expect("Maccms candidate 应先 durable staging");
        system
            .shutdown_for_test()
            .await
            .expect("篡改前应关闭 SQLite writer");
        drop(system);
        tamper_candidate_row(&temp.database_path(), &candidate, tamper);

        let reopened = temp.reopen_after_drop(Duration::from_mins(1)).await;
        let error = reopened
            .install(candidate.id, CapabilityGrant::network_only())
            .await
            .expect_err("重启后 candidate durable metadata 篡改必须被拒绝");
        assert_eq!(error.stage, RuleErrorStage::Candidate);
        assert_eq!(error.code, "candidate_tampered");
    }
}

#[tokio::test]
async fn maccms_json_four_intents_live_and_replay_use_only_rule_system() {
    init_mock_keyring();
    let temp = TempRuleSystem::new("maccms-live-replay");
    let server = MockServer::start().await;
    mount_maccms_json_routes(&server, None).await;
    let system = temp.open(Duration::from_mins(1)).await;
    let candidate = system
        .prepare_install(maccms_json_input(&server.uri()))
        .await
        .expect("Maccms JSON candidate");
    let source = system
        .install(candidate.id, CapabilityGrant::network_only())
        .await
        .expect("Maccms JSON source install");
    let source_profile_id = source.profile.id.clone();

    let discover = execute_live(
        &system,
        &source.source_id,
        StandardIntent::Discover,
        IntentInput::None,
    )
    .await;
    let item_id = assert_discover_delta(&discover.delta, &source_profile_id);

    let detail = execute_live(
        &system,
        &source.source_id,
        StandardIntent::ResolveItem,
        IntentInput::ItemId(item_id.clone()),
    )
    .await;
    assert_resolve_item_delta(&detail.delta, &source_profile_id);

    let units = execute_live(
        &system,
        &source.source_id,
        StandardIntent::ListUnits,
        IntentInput::ItemId(item_id.clone()),
    )
    .await;
    let unit_id = assert_list_units_delta(&units.delta, &source_profile_id);

    let asset = execute_live(
        &system,
        &source.source_id,
        StandardIntent::ResolveAsset,
        IntentInput::UnitId(unit_id.clone()),
    )
    .await;
    assert_resolve_asset_delta(&asset.delta, &source_profile_id, &unit_id);

    drop(server);
    for (intent, input, execution_id, expected_delta) in [
        (
            StandardIntent::Discover,
            IntentInput::None,
            discover.execution_id,
            &discover.delta,
        ),
        (
            StandardIntent::ResolveItem,
            IntentInput::ItemId(item_id.clone()),
            detail.execution_id,
            &detail.delta,
        ),
        (
            StandardIntent::ListUnits,
            IntentInput::ItemId(item_id),
            units.execution_id,
            &units.delta,
        ),
        (
            StandardIntent::ResolveAsset,
            IntentInput::UnitId(unit_id),
            asset.execution_id,
            &asset.delta,
        ),
    ] {
        let replay_delta =
            execute_replay(&system, &source.source_id, intent, input, execution_id).await;
        assert_delta_source_ownership(&replay_delta, &source_profile_id);
        assert_eq!(
            &replay_delta, expected_delta,
            "{intent:?} replay 必须产生与 live 等价的 Delta"
        );
    }
}

#[tokio::test]
async fn stream_drop_does_not_cancel_and_cancel_and_catch_up_are_idempotent_and_contiguous() {
    init_mock_keyring();
    let temp = TempRuleSystem::new("session-lifecycle");
    let server = MockServer::start().await;
    mount_maccms_json_routes(&server, Some(Duration::from_millis(300))).await;
    let system = temp.open(Duration::from_mins(1)).await;
    let candidate = system
        .prepare_install(maccms_json_input(&server.uri()))
        .await
        .expect("Maccms JSON candidate");
    let source = system
        .install(candidate.id, CapabilityGrant::network_only())
        .await
        .expect("Maccms JSON install");

    let dropped_session = system
        .execute(ExecuteRequest {
            source_id: source.source_id.clone(),
            intent: StandardIntent::Discover,
            input: IntentInput::None,
            mode: ExecutionMode::Live,
        })
        .await
        .expect("dropped-stream live execution");
    let dropped_execution = dropped_session.id;
    drop(dropped_session);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let cancelled = system
        .execute(ExecuteRequest {
            source_id: source.source_id.clone(),
            intent: StandardIntent::Discover,
            input: IntentInput::None,
            mode: ExecutionMode::Live,
        })
        .await
        .expect("cancellable execution");
    let cancelled_execution = cancelled.id;
    let running_replay_error = system
        .execute(ExecuteRequest {
            source_id: source.source_id.clone(),
            intent: StandardIntent::Discover,
            input: IntentInput::None,
            mode: ExecutionMode::Replay {
                execution_id: cancelled_execution,
            },
        })
        .await
        .err()
        .expect("running execution archive 不得 replay");
    assert_eq!(running_replay_error.stage, RuleErrorStage::Replay);
    assert_eq!(running_replay_error.code, "replay_execution_not_completed");
    assert!(cancelled.cancel(), "首次 cancel 必须改变状态");
    assert!(!cancelled.cancel(), "重复 cancel 必须幂等");
    let catch_up = catch_up_until_terminal(&cancelled).await;
    assert_contiguous(&catch_up);
    assert_eq!(
        catch_up
            .iter()
            .filter(|event| is_terminal(&event.kind))
            .count(),
        1,
        "cancelled session 必须只有一个持久终态: {catch_up:?}"
    );
    assert!(
        matches!(
            catch_up.last().map(|event| &event.kind),
            Some(ExecutionEventKind::Cancelled)
        ),
        "cancelled session 必须只有 Cancelled 终态: {catch_up:?}"
    );
    let cancelled_replay_error = system
        .execute(ExecuteRequest {
            source_id: source.source_id.clone(),
            intent: StandardIntent::Discover,
            input: IntentInput::None,
            mode: ExecutionMode::Replay {
                execution_id: cancelled_execution,
            },
        })
        .await
        .err()
        .expect("cancelled execution archive 不得 replay");
    assert_eq!(cancelled_replay_error.stage, RuleErrorStage::Replay);
    assert_eq!(
        cancelled_replay_error.code,
        "replay_execution_not_completed"
    );

    let moved_session = system
        .execute(ExecuteRequest {
            source_id: source.source_id.clone(),
            intent: StandardIntent::Discover,
            input: IntentInput::None,
            mode: ExecutionMode::Live,
        })
        .await
        .expect("moved-stream cancellable execution");
    let moved_cancellation = moved_session.cancellation_handle();
    let moved_delivery = moved_session.into_events();
    assert!(
        moved_cancellation.cancel(),
        "delivery stream 移动后 opaque handle 仍必须可取消"
    );
    assert!(
        !moved_cancellation.cancel(),
        "移动后 opaque handle 的重复取消必须幂等"
    );
    let moved_events = moved_delivery.collect::<Vec<_>>().await;
    assert_contiguous(&moved_events);
    assert_eq!(
        moved_events
            .iter()
            .filter(|event| is_terminal(&event.kind))
            .count(),
        1,
        "moved-stream execution 必须只有一个持久终态: {moved_events:?}"
    );
    assert!(
        matches!(
            moved_events.last().map(|event| &event.kind),
            Some(ExecutionEventKind::Cancelled)
        ),
        "opaque handle 必须在 stream 移动后 delivery Cancelled: {moved_events:?}"
    );

    drop(server);
    let dropped_replay = system
        .execute(ExecuteRequest {
            source_id: source.source_id,
            intent: StandardIntent::Discover,
            input: IntentInput::None,
            mode: ExecutionMode::Replay {
                execution_id: dropped_execution,
            },
        })
        .await
        .expect("丢弃 stream 不得隐式取消已启动 execution")
        .into_events()
        .collect::<Vec<_>>()
        .await;
    assert_completed(&dropped_replay, false);
}

fn library_update(resource_id: String, expected_version: u64, pinned: bool) -> LibraryEntryUpdate {
    LibraryEntryUpdate {
        resource_id,
        favorite: true,
        pinned,
        last_opened_at: Some("2026-07-18T00:00:00Z".to_string()),
        progress: Some(LibraryProgress {
            unit_id: Some("unit:query:1".to_string()),
            position: 42,
            total: Some(100),
        }),
        expected_version,
    }
}

#[tokio::test]
async fn safe_query_facade_lists_sources_projects_library_and_catches_up() {
    init_mock_keyring();
    let temp = TempRuleSystem::new("safe-query-facade");
    let server = MockServer::start().await;
    mount_maccms_json_routes(&server, None).await;
    let system = temp.open(Duration::from_mins(1)).await;
    let candidate = system
        .prepare_install(maccms_json_input(&server.uri()))
        .await
        .expect("Maccms JSON candidate");
    let installed = system
        .install(candidate.id, CapabilityGrant::network_only())
        .await
        .expect("Maccms JSON install");

    let sources = system
        .list_installed_sources()
        .await
        .expect("safe facade 应列出持久已安装来源");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0], installed);
    let source_wire = serde_json::to_value(&sources[0]).expect("来源摘要可安全序列化");
    assert!(source_wire.get("definition").is_none());
    assert!(source_wire.get("plan").is_none());
    assert!(source_wire.get("secret").is_none());

    let empty_projection = system
        .get_library_projection()
        .await
        .expect("safe facade 应读取空资料库投影");
    assert!(empty_projection.entries.is_empty());

    let session = system
        .execute(ExecuteRequest {
            source_id: installed.source_id.clone(),
            intent: StandardIntent::Discover,
            input: IntentInput::None,
            mode: ExecutionMode::Live,
        })
        .await
        .expect("safe facade execution");
    let execution_id = session.id;
    let delivered = session.into_events().collect::<Vec<_>>().await;
    assert_completed(&delivered, true);
    let caught_up = system
        .catch_up_execution(execution_id, 0)
        .await
        .expect("RuleSystem 应补读指定 execution 的持久事件");
    assert_eq!(caught_up, delivered);
    let resource_id = committed_delta(&delivered).items[0].id.0.clone();

    let created = system
        .update_library_entry(library_update(resource_id.clone(), 0, false))
        .await
        .expect("safe facade 应创建资料库条目");
    let stale = system
        .update_library_entry(library_update(resource_id.clone(), 0, true))
        .await
        .expect_err("过期 library revision 必须被拒绝");
    assert_eq!(stale.code, "stream_version_conflict");
    let updated = system
        .update_library_entry(library_update(resource_id.clone(), created.revision, true))
        .await
        .expect("当前 library revision 应可更新");

    let projection = system
        .get_library_projection()
        .await
        .expect("safe facade 应读取更新后的资料库投影");
    assert!(projection.global_seq >= updated.global_seq);
    assert_eq!(projection.entries.len(), 1);
    let entry = &projection.entries[0];
    assert_eq!(entry.resource_id, resource_id);
    assert!(entry.favorite);
    assert!(entry.pinned);
    assert_eq!(entry.revision, updated.revision);
    assert_eq!(entry.updated_global_seq, updated.global_seq);
    assert_eq!(
        entry.progress.as_ref().map(|progress| progress.position),
        Some(42)
    );
    let projection_wire = serde_json::to_value(&projection).expect("资料库投影可安全序列化");
    assert!(projection_wire.get("plan").is_none());
    assert!(projection_wire.get("secret").is_none());
}
