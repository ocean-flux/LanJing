use std::collections::BTreeMap;
use std::fs;
use std::sync::Once;

use futures::StreamExt as _;
use keyring_core::{mock, set_default_store};
use lj_capability::{IntentExport, IntentInput, StandardIntent};
use lj_rule_model::definition::MapperOutputKind;
use lj_rule_model::{
    CapabilityManifest, ConditionConfig, ControlExpression, ControlledMapper, FlowEdge, FlowGraph,
    FlowNode, FlowNodeConfig, FlowPortRef, JsConfig, JsOutputKind, LINEAR_INPUT_HANDLE,
    LINEAR_OUTPUT_HANDLE, MERGE_OUTPUT_HANDLE, MergeConfig, MergeInput, MergeInputActivation,
    MergeStrategy, PolicyCapabilities, RuleDefinition, SourceIdentity, SystemCapabilities,
};
use uuid::Uuid;

use super::super::RuleSystem;
use super::prepare_install::PreparedRuleInput;
use crate::{CapabilityGrant, ExecuteRequest, ExecutionEventKind, ExecutionMode, RuleSystemConfig};

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
