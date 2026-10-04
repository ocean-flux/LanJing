use std::collections::BTreeMap;
use std::fs;
use std::sync::Once;

use futures::StreamExt as _;
use keyring_core::{mock, set_default_store};
use lj_capability::{IntentExport, IntentInput, StandardIntent};
use lj_rule_model::definition::MapperOutputKind;
use lj_rule_model::{
    CapabilityManifest, ConditionConfig, ControlExpression, ControlledMapper, DiagnosticSeverity,
    FlowEdge, FlowGraph, FlowNode, FlowNodeConfig, FlowPortRef, JsConfig, JsOutputKind,
    LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE, MERGE_OUTPUT_HANDLE, MergeConfig, MergeInput,
    MergeInputActivation, MergeStrategy, PolicyCapabilities, RuleDefinition, RulePackage,
    SourceIdentity, SystemCapabilities, UnavailableNodeConfig, definition_hash,
};
use uuid::Uuid;

use super::super::RuleSystem;
use super::prepare_install::PreparedRuleInput;
use crate::{
    CapabilityGrant, ExecuteRequest, ExecutionEventKind, ExecutionMode, RuleErrorStage, RuleInput,
    RuleSystemConfig, SourceId, SourceOperation,
};

fn init_mock_keyring() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        set_default_store(mock::Store::new().expect("keyring-core mock store"));
    });
}

fn current_control_definition() -> RuleDefinition {
    let entry = Uuid::from_u128(2_001);
    let condition = Uuid::from_u128(2_002);
    let alpha = Uuid::from_u128(2_003);
    let beta = Uuid::from_u128(2_004);
    let merge = Uuid::from_u128(2_005);
    let mapper = Uuid::from_u128(2_006);
    RuleDefinition::new(
        SourceIdentity {
            id: "source:rule-system-control-test".to_string(),
        },
        "https://example.invalid",
        BTreeMap::from([(
            StandardIntent::Search,
            IntentExport::new(entry, mapper),
        )]),
        FlowGraph {
            nodes: vec![
                FlowNode::new(
                    entry,
                    FlowNodeConfig::Js(JsConfig {
                        code: "JSON.stringify([{ enabled: true, title: '入口', url: 'https://example.invalid/entry' }])".to_string(),
                        output: JsOutputKind::Json,
                    }),
                ),
                FlowNode::new(
                    condition,
                    FlowNodeConfig::Condition(ConditionConfig {
                        branches: vec!["alpha".to_string(), "beta".to_string()],
                        expression: ControlExpression::Js {
                            code: "'alpha'".to_string(),
                        },
                    }),
                ),
                FlowNode::new(
                    alpha,
                    FlowNodeConfig::Js(JsConfig {
                        code: "JSON.stringify([{ title: '命中', url: 'https://example.invalid/alpha' }])".to_string(),
                        output: JsOutputKind::Json,
                    }),
                ),
                FlowNode::new(
                    beta,
                    FlowNodeConfig::Js(JsConfig {
                        code: "JSON.stringify([{ title: '未命中', url: 'https://example.invalid/beta' }])".to_string(),
                        output: JsOutputKind::Json,
                    }),
                ),
                FlowNode::new(
                    merge,
                    FlowNodeConfig::Merge(MergeConfig {
                        inputs: vec![
                            MergeInput {
                                input_id: "alpha".to_string(),
                                handle: "alpha".to_string(),
                                order: 0,
                                activation: MergeInputActivation::Required,
                            },
                            MergeInput {
                                input_id: "beta".to_string(),
                                handle: "beta".to_string(),
                                order: 1,
                                activation: MergeInputActivation::Optional,
                            },
                        ],
                        strategy: MergeStrategy::SingleActive,
                    }),
                ),
                FlowNode::new(
                    mapper,
                    FlowNodeConfig::Mapper(ControlledMapper {
                        output: MapperOutputKind::Items,
                        identity_fields: vec!["url".to_string()],
                    }),
                ),
            ],
            edges: vec![
                FlowEdge::new(
                    FlowPortRef::new(entry, LINEAR_OUTPUT_HANDLE),
                    FlowPortRef::new(condition, lj_rule_model::CONDITION_INPUT_HANDLE),
                ),
                FlowEdge::new(
                    FlowPortRef::new(condition, "alpha"),
                    FlowPortRef::new(alpha, LINEAR_INPUT_HANDLE),
                ),
                FlowEdge::new(
                    FlowPortRef::new(condition, "beta"),
                    FlowPortRef::new(beta, LINEAR_INPUT_HANDLE),
                ),
                FlowEdge::new(
                    FlowPortRef::new(alpha, LINEAR_OUTPUT_HANDLE),
                    FlowPortRef::new(merge, "alpha"),
                ),
                FlowEdge::new(
                    FlowPortRef::new(beta, LINEAR_OUTPUT_HANDLE),
                    FlowPortRef::new(merge, "beta"),
                ),
                FlowEdge::new(
                    FlowPortRef::new(merge, MERGE_OUTPUT_HANDLE),
                    FlowPortRef::new(mapper, LINEAR_INPUT_HANDLE),
                ),
            ],
        },
        CapabilityManifest {
            required: PolicyCapabilities {
                network: true,
                system: SystemCapabilities::default(),
            },
        },
        vec!["url".to_string()],
    )
}

#[tokio::test]
async fn current_control_candidate_passes_gate_and_installs_executes_and_replays() {
    init_mock_keyring();
    let root = std::env::temp_dir().join(format!("lj-rule-system-control-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("create fixture root");
    let system = RuleSystem::open(
        RuleSystemConfig::desktop(root.join("event-store.db"), root.join("artifacts"))
            .with_keyring_service(format!("lanjing.rule-system.test.{}", Uuid::new_v4())),
    )
    .await
    .expect("open RuleSystem");
    let candidate = system
        .stage_prepared_candidate(
            PreparedRuleInput {
                definition: current_control_definition(),
                runtime_credentials: None,
                display_title: Some("控制流测试".to_string()),
                display_group: None,
                diagnostics: Vec::new(),
            },
            0,
            "trace-control-candidate",
        )
        .await
        .expect("stage current candidate");
    let installed = system
        .install(candidate.id, CapabilityGrant::network_only())
        .await
        .expect("install candidate");

    let live = system
        .execute(ExecuteRequest {
            source_id: installed.source_id.clone(),
            intent: StandardIntent::Search,
            input: IntentInput::Query("control".to_string()),
            mode: ExecutionMode::Live,
        })
        .await
        .expect("start live");
    let live_execution_id = live.id;
    let live_events = live.into_events().collect::<Vec<_>>().await;
    assert!(matches!(
        live_events.last().map(|event| &event.kind),
        Some(ExecutionEventKind::Completed)
    ));
    assert!(
        live_events
            .iter()
            .any(|event| matches!(event.kind, ExecutionEventKind::EffectCaptured { .. }))
    );

    let replay = system
        .execute(ExecuteRequest {
            source_id: installed.source_id,
            intent: StandardIntent::Search,
            input: IntentInput::Query("control".to_string()),
            mode: ExecutionMode::Replay {
                execution_id: live_execution_id,
            },
        })
        .await
        .expect("start replay");
    let replay_events = replay.into_events().collect::<Vec<_>>().await;
    assert!(matches!(
        replay_events.last().map(|event| &event.kind),
        Some(ExecutionEventKind::Completed)
    ));
    assert!(
        replay_events
            .iter()
            .all(|event| !matches!(event.kind, ExecutionEventKind::EffectCaptured { .. }))
    );
    drop(system);
    let _ = fs::remove_dir_all(root);
}

async fn open_package_test_system() -> RuleSystem {
    init_mock_keyring();
    let root = std::env::temp_dir().join(format!("lj-rule-system-package-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("create fixture root");
    RuleSystem::open(
        RuleSystemConfig::desktop(root.join("event-store.db"), root.join("artifacts"))
            .with_keyring_service(format!("lanjing.rule-system.test.{}", Uuid::new_v4())),
    )
    .await
    .expect("open RuleSystem")
}

fn package_bytes(definition: &RuleDefinition, version: &str) -> Vec<u8> {
    serde_json::to_vec(&RulePackage::new(
        definition.source_identity().clone(),
        version,
        definition.clone(),
    ))
    .expect("serialize Rule Package")
}

fn definition_with_unavailable_node() -> RuleDefinition {
    let mut definition = current_control_definition();
    definition.flow_mut().nodes[0].config = FlowNodeConfig::Unavailable(
        UnavailableNodeConfig::new("custom_reader", serde_json::json!({ "selector": ".entry" })),
    );
    definition
}

fn package_input(definition: &RuleDefinition, version: &str) -> RuleInput {
    RuleInput::Package {
        source_json: String::from_utf8(package_bytes(definition, version)).expect("utf-8 package"),
    }
}

fn has_error_capability_diagnostic(error: &crate::RuleError) -> bool {
    error
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "NODE_CAPABILITY_UNAVAILABLE")
}

#[tokio::test]
async fn inspect_rule_package_reports_identity_version_and_canonical_definition_hash() {
    let system = open_package_test_system().await;
    let definition = current_control_definition();

    let inspection = system
        .inspect_rule_package(&package_bytes(&definition, "v1"))
        .expect("合法 package 必须通过 facade 校验");

    assert_eq!(
        inspection.source_id,
        SourceId::from_identity("source:rule-system-control-test".to_string())
    );
    assert_eq!(inspection.version, "v1");
    assert_eq!(
        inspection.definition_hash,
        definition_hash(&definition).expect("canonical Definition hash")
    );
    assert!(inspection.unavailable_nodes.is_empty());
    assert!(
        !inspection
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    );
    drop(system);
}

#[tokio::test]
async fn inspect_rule_package_flags_unavailable_capability_nodes_without_refusing_the_file() {
    let system = open_package_test_system().await;
    let definition = definition_with_unavailable_node();

    let inspection = system
        .inspect_rule_package(&package_bytes(&definition, "v2"))
        .expect("含未安装能力的 package 仍须可读入、展示与保存");

    assert_eq!(inspection.unavailable_nodes.len(), 1);
    assert_eq!(
        inspection.unavailable_nodes[0].node_id,
        Uuid::from_u128(2_001)
    );
    assert_eq!(inspection.unavailable_nodes[0].kind, "custom_reader");
    assert!(
        inspection
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NODE_CAPABILITY_UNAVAILABLE")
    );
    drop(system);
}

#[tokio::test]
async fn inspect_rule_package_reports_stable_error_for_incompatible_schema() {
    let system = open_package_test_system().await;
    let definition = current_control_definition();
    let mut value = serde_json::to_value(RulePackage::new(
        definition.source_identity().clone(),
        "v1",
        definition,
    ))
    .expect("serialize Rule Package");
    value["schema_version"] = serde_json::json!(2);

    let error = system
        .inspect_rule_package(&serde_json::to_vec(&value).expect("serialize fixture"))
        .expect_err("不兼容 schema 必须被拒绝");

    assert_eq!(error.stage, RuleErrorStage::Import);
    assert_eq!(error.code, "RULE_CONTRACT_SCHEMA_UNSUPPORTED");
    drop(system);
}

#[tokio::test]
async fn inspect_rule_package_reports_stable_error_for_invalid_data() {
    let system = open_package_test_system().await;
    let definition = current_control_definition();
    let mut value = serde_json::to_value(RulePackage::new(
        definition.source_identity().clone(),
        "v1",
        definition,
    ))
    .expect("serialize Rule Package");
    value["definition"]["unknown"] = serde_json::json!(true);

    let error = system
        .inspect_rule_package(&serde_json::to_vec(&value).expect("serialize fixture"))
        .expect_err("未知字段必须仍是非法数据");

    assert_eq!(error.stage, RuleErrorStage::Import);
    assert_eq!(error.code, "RULE_CONTRACT_INVALID_DATA");
    drop(system);
}

#[tokio::test]
async fn prepare_install_imports_valid_package_as_candidate() {
    let system = open_package_test_system().await;
    let definition = current_control_definition();

    let candidate = system
        .prepare_install(package_input(&definition, "v1"))
        .await
        .expect("合法 package 必须可导入");

    assert_eq!(
        candidate.definition_hash,
        definition_hash(&definition).expect("canonical Definition hash")
    );
    assert_eq!(candidate.profile.id.0, "source:rule-system-control-test");
    assert!(
        !candidate
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    );
    drop(system);
}

#[tokio::test]
async fn prepare_install_rejects_package_with_unavailable_capability() {
    let system = open_package_test_system().await;
    let definition = definition_with_unavailable_node();

    let error = system
        .prepare_install(package_input(&definition, "v2"))
        .await
        .expect_err("含未安装能力的 package 不得进入 candidate 或 Plan");

    assert_eq!(error.stage, RuleErrorStage::Validation);
    assert!(has_error_capability_diagnostic(&error));
    // 拒绝不得留残余: 没有 installed source 就没有可 execute 的来源。
    assert!(
        system
            .list_installed_sources()
            .await
            .expect("list installed sources")
            .is_empty()
    );
    drop(system);
}

#[tokio::test]
async fn candidate_declares_install_for_unknown_source_and_update_for_installed_one() {
    let system = open_package_test_system().await;
    let definition = current_control_definition();

    let first = system
        .prepare_install(package_input(&definition, "v1"))
        .await
        .expect("首次安装必须可准备");
    assert_eq!(first.operation, SourceOperation::Install);
    assert_eq!(first.expected_installed_revision, 0);
    let installed = system
        .install(first.id, CapabilityGrant::network_only())
        .await
        .expect("首次安装必须提交");

    let update = system
        .prepare_install(package_input(&definition, "v2"))
        .await
        .expect("已安装来源必须可准备更新");
    assert_eq!(update.operation, SourceOperation::Update);
    assert_eq!(update.expected_installed_revision, installed.revision);
    drop(system);
}

#[tokio::test]
async fn rollback_candidate_is_declared_as_update_of_installed_revision() {
    let system = open_package_test_system().await;
    let definition = current_control_definition();
    let first = system
        .prepare_install(package_input(&definition, "v1"))
        .await
        .expect("首次安装必须可准备");
    system
        .install(first.id, CapabilityGrant::network_only())
        .await
        .expect("首次安装必须提交");
    let source_id = SourceId::from_identity(first.profile.id.0.clone());

    let rollback = system
        .prepare_source_rollback(source_id.clone(), 1)
        .await
        .expect("已安装来源必须可回滚到历史 revision");

    assert_eq!(rollback.operation, SourceOperation::Update);
    assert_eq!(rollback.expected_installed_revision, 1);
    drop(system);
}

#[tokio::test]
async fn stale_candidate_is_rejected_after_source_revision_moves_on() {
    let system = open_package_test_system().await;
    let definition = current_control_definition();
    // 两个 candidate 都以"尚未安装"为基线, 第一个提交后第二个基线就过期了。
    let first = system
        .prepare_install(package_input(&definition, "v1"))
        .await
        .expect("首个 candidate 必须可准备");
    let second = system
        .prepare_install(package_input(&definition, "v2"))
        .await
        .expect("第二个 candidate 必须可准备");

    system
        .install(first.id, CapabilityGrant::network_only())
        .await
        .expect("首个 candidate 必须提交");

    let error = system
        .install(second.id, CapabilityGrant::network_only())
        .await
        .expect_err("过期 candidate 不得提交");

    assert_eq!(error.stage, RuleErrorStage::Candidate);
    assert_eq!(error.code, "candidate_stale");
    assert_eq!(
        system
            .list_source_revisions(SourceId::from_identity(second.profile.id.0.clone()))
            .await
            .expect("已安装来源必须仍可读历史")
            .len(),
        1,
        "过期 candidate 不得追加 Source Revision"
    );
    drop(system);
}
