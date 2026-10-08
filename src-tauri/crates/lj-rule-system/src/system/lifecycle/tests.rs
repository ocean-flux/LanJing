use std::collections::BTreeMap;
use std::fs;
use std::sync::Once;
use std::time::Duration;

use futures::StreamExt as _;
use keyring_core::{mock, set_default_store};
use lj_capability::{IntentExport, IntentInput, StandardIntent};
use lj_rule_model::definition::MapperOutputKind;
use lj_rule_model::{
    CapabilityManifest, ConditionConfig, ControlExpression, ControlledMapper, DiagnosticSeverity,
    FlowEdge, FlowGraph, FlowNode, FlowNodeConfig, FlowPortRef, JsBudget, JsConfig, JsOutputKind,
    LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE, MERGE_OUTPUT_HANDLE, MergeConfig, MergeInput,
    MergeInputActivation, MergeStrategy, RuleDefinition, RulePackage, SourceIdentity,
    SystemCapabilities, UnavailableNodeConfig, definition_hash, read_rule_package,
};
use uuid::Uuid;

use super::super::RuleSystem;
use super::prepare_install::PreparedRuleInput;
use crate::{
    CreateMode, CreateNativeRuleDocumentRequest, ExecuteRequest, ExecutionEventKind, ExecutionMode,
    ExpectedDataType, GetNativeRuleDocumentRequest, RuleErrorStage, RuleInput, RuleSystemConfig,
    SaveNativeRuleDocumentRequest, SemanticSave, SourceId, SourceOperation,
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
                    FlowNodeConfig::Js(JsConfig::new("JSON.stringify([{ enabled: true, title: '入口', url: 'https://example.invalid/entry' }])".to_string(), JsOutputKind::Json)),
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
                    FlowNodeConfig::Js(JsConfig::new("JSON.stringify([{ title: '命中', url: 'https://example.invalid/alpha' }])".to_string(), JsOutputKind::Json)),
                ),
                FlowNode::new(
                    beta,
                    FlowNodeConfig::Js(JsConfig::new("JSON.stringify([{ title: '未命中', url: 'https://example.invalid/beta' }])".to_string(), JsOutputKind::Json)),
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
            required: SystemCapabilities::default(),
        },
        vec!["url".to_string()],
    )
}

#[tokio::test]
async fn install_refuses_candidate_declaring_system_capabilities() {
    init_mock_keyring();
    let root = std::env::temp_dir().join(format!("lj-rule-system-system-grant-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("create fixture root");
    let system = RuleSystem::open(
        RuleSystemConfig::desktop(root.join("event-store.db"), root.join("artifacts"))
            .with_keyring_service(format!("lanjing.rule-system.test.{}", Uuid::new_v4())),
    )
    .await
    .expect("open RuleSystem");
    let mut definition = current_control_definition();
    definition.capability_manifest_mut().required.fs = true;
    let candidate = system
        .stage_prepared_candidate(
            PreparedRuleInput {
                definition,
                runtime_credentials: None,
                display_title: None,
                display_group: None,
                diagnostics: Vec::new(),
            },
            0,
            "trace-system-grant-candidate",
        )
        .await
        .expect("stage candidate requiring a system capability");
    let error = system
        .install(candidate.id)
        .await
        .expect_err("声明 fs 能力的 candidate 不得安装：应用不授予任何系统能力");
    assert_eq!(error.stage, RuleErrorStage::Capability);
    assert_eq!(error.code, "grant_insufficient");
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
        .install(candidate.id)
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
async fn export_rule_package_round_trips_installed_package_through_import() {
    let system = open_package_test_system().await;
    let definition = current_control_definition();
    let candidate = system
        .prepare_install(package_input(&definition, "v1"))
        .await
        .expect("合法 package 必须可导入");
    let installed_definition_hash = candidate.definition_hash.clone();
    let installed = system
        .install(candidate.id)
        .await
        .expect("candidate 必须可安装");

    let exported = system
        .export_rule_package(installed.source_id)
        .await
        .expect("已安装来源必须可导出");
    let package =
        read_rule_package(exported.as_bytes()).expect("导出内容必须通过 package 合同校验");
    assert_eq!(
        package.source_identity().id,
        "source:rule-system-control-test"
    );
    // 安装把 package version 定为 content-addressed 的 canonical Definition hash。
    assert_eq!(package.version(), installed_definition_hash);
    assert_eq!(
        definition_hash(package.definition()).expect("canonical Definition hash"),
        installed_definition_hash
    );

    // 导出文件必须能重新进入导入链；同一来源已安装，因此声明为 update。
    let reimported = system
        .prepare_install(RuleInput::Package {
            source_json: exported,
        })
        .await
        .expect("导出的 package 必须可重新导入");
    assert_eq!(reimported.operation, SourceOperation::Update);
    drop(system);
}

#[tokio::test]
async fn export_rule_package_reports_source_not_installed() {
    let system = open_package_test_system().await;

    let error = system
        .export_rule_package(SourceId::from_identity("source:unknown".to_string()))
        .await
        .expect_err("未安装来源不得导出");

    assert_eq!(error.code, "source_not_installed");
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
    let installed = system.install(first.id).await.expect("首次安装必须提交");

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
    system.install(first.id).await.expect("首次安装必须提交");
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
        .install(first.id)
        .await
        .expect("首个 candidate 必须提交");

    let error = system
        .install(second.id)
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
/// 一次 JS 执行的预期终态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsOutcomeTerminal {
    /// 正常完成。
    Completed,
    /// effect 失败（异常 / 超时 / 资源超限）。
    Failed,
    /// 外部取消。
    Cancelled,
}

/// 同一段脚本经真实 adapter 执行时必须得到的稳定结局。
///
/// 这是防误报的关键：只有先证明这段脚本真的产出目标结局，后续「状态未被破坏」的断言
/// 才是在证明本用例的结局，而不是某个无关失败的副作用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsAdapterOutcome {
    /// 正常返回。
    Ok,
    /// 运行时异常。
    Evaluation,
    /// watchdog 超时中断。
    Timeout,
    /// 外部取消中断。
    Cancelled,
    /// 堆预算超限。
    MemoryLimit,
    /// 输出预算超限。
    OutputBudget,
}

impl JsAdapterOutcome {
    fn matches(self, result: &Result<String, lj_node_js::error::JsError>) -> bool {
        use lj_node_js::error::JsError;
        matches!(
            (self, result),
            (Self::Ok, Ok(_))
                | (Self::Evaluation, Err(JsError::EvalError(_)))
                | (Self::Timeout, Err(JsError::Timeout(_)))
                | (Self::Cancelled, Err(JsError::Cancelled))
                | (Self::MemoryLimit, Err(JsError::MemoryLimit))
                | (Self::OutputBudget, Err(JsError::OutputBudget(_)))
        )
    }
}

/// 失败 / 取消用例中应归档的 effect 数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureExpectation {
    /// 至少一次 capture（正常完成时下游 effect 也要有）。
    Any,
    /// 恰好这么多次 capture。
    Exactly(usize),
}

/// 一个 JS 结局用例：脚本、预期 adapter 结局、预期 execution 终态。
struct JsOutcomeCase {
    label: &'static str,
    entry_code: &'static str,
    entry_budgets: JsBudget,
    terminal: JsOutcomeTerminal,
    adapter_outcome: JsAdapterOutcome,
    captures: CaptureExpectation,
    cancel_after: Option<Duration>,
}

/// ticket 点名的五种结局，资源超限按内存与输出各取一例。
fn js_outcome_cases() -> Vec<JsOutcomeCase> {
    const NORMAL_ENTRY: &str =
        "JSON.stringify([{ enabled: true, title: '入口', url: 'https://example.invalid/entry' }])";
    vec![
        JsOutcomeCase {
            label: "正常完成",
            entry_code: NORMAL_ENTRY,
            entry_budgets: JsBudget::default(),
            terminal: JsOutcomeTerminal::Completed,
            adapter_outcome: JsAdapterOutcome::Ok,
            captures: CaptureExpectation::Any,
            cancel_after: None,
        },
        JsOutcomeCase {
            label: "异常",
            entry_code: "throw new Error('boom')",
            entry_budgets: JsBudget::default(),
            terminal: JsOutcomeTerminal::Failed,
            adapter_outcome: JsAdapterOutcome::Evaluation,
            captures: CaptureExpectation::Exactly(1),
            cancel_after: None,
        },
        JsOutcomeCase {
            label: "超时",
            entry_code: "while (true) {}",
            entry_budgets: JsBudget {
                timeout_ms: 200,
                ..JsBudget::default()
            },
            terminal: JsOutcomeTerminal::Failed,
            adapter_outcome: JsAdapterOutcome::Timeout,
            captures: CaptureExpectation::Exactly(1),
            cancel_after: None,
        },
        JsOutcomeCase {
            label: "取消",
            entry_code: "while (true) {}",
            entry_budgets: JsBudget::default(),
            terminal: JsOutcomeTerminal::Cancelled,
            adapter_outcome: JsAdapterOutcome::Cancelled,
            captures: CaptureExpectation::Exactly(0),
            cancel_after: Some(Duration::from_millis(50)),
        },
        JsOutcomeCase {
            label: "内存超限",
            entry_code: "new Array(100000000).fill(1)",
            entry_budgets: JsBudget::default(),
            terminal: JsOutcomeTerminal::Failed,
            adapter_outcome: JsAdapterOutcome::MemoryLimit,
            captures: CaptureExpectation::Exactly(1),
            cancel_after: None,
        },
        JsOutcomeCase {
            label: "输出超限",
            entry_code: "'x'.repeat(4096)",
            entry_budgets: JsBudget {
                output_bytes: 64,
                ..JsBudget::default()
            },
            terminal: JsOutcomeTerminal::Failed,
            adapter_outcome: JsAdapterOutcome::OutputBudget,
            captures: CaptureExpectation::Exactly(1),
            cancel_after: None,
        },
    ]
}

/// 用真实 `QuickJS` adapter 证明脚本确实产出目标结局。
fn assert_adapter_outcome(case: &JsOutcomeCase) {
    let cancellation = lj_runtime::CancellationHandle::new();
    let token = cancellation.token();
    if case.adapter_outcome == JsAdapterOutcome::Cancelled {
        let _ = cancellation.cancel();
    }
    let result = lj_node_js::execute_js_blocking_cancellable(
        case.entry_code,
        None,
        None,
        case.entry_budgets,
        &token,
    );
    assert!(
        case.adapter_outcome.matches(&result),
        "JS 结局 {} 的脚本未在真实 adapter 上产出预期结局: {result:?}",
        case.label
    );
}

/// 文档语义域的完整可观测状态：Draft Revision 与 Effective Rule Revision。
///
/// 断言整体相等，而不是逐字段挑选：漏掉任一被改写/推进/丢弃的字段都会让测试失败。
#[derive(Debug, Clone, PartialEq, Eq)]
struct DocumentRevisionState {
    semantic_revision: i64,
    effective_revision: Option<i64>,
    effective_definition_hash: Option<String>,
    draft_definition: Option<RuleDefinition>,
}

async fn document_revision_state(system: &RuleSystem, document_id: &str) -> DocumentRevisionState {
    let detail = system
        .get_native_rule_document(GetNativeRuleDocumentRequest {
            document_id: document_id.to_string(),
        })
        .await
        .expect("JS 结局不得使文档读取失败")
        .expect("文档必须存在");
    DocumentRevisionState {
        semantic_revision: detail.semantic_revision,
        effective_revision: detail.effective_semantic_revision,
        effective_definition_hash: detail
            .effective_summary
            .map(|summary| summary.definition_hash),
        draft_definition: detail.definition,
    }
}

/// 在 control 定义上替换入口 JS 节点的源码与预算，其余拓扑保持不变。
fn definition_with_entry_js(
    identity: &str,
    entry_code: &str,
    entry_budgets: JsBudget,
) -> RuleDefinition {
    let mut definition = current_control_definition();
    *definition.source_identity_mut() = SourceIdentity {
        id: identity.to_string(),
    };
    definition.flow_mut().nodes[0].config = FlowNodeConfig::Js(JsConfig {
        code: entry_code.to_string(),
        output: JsOutputKind::Json,
        budgets: entry_budgets,
    });
    definition
}

/// control 定义里 `alpha` 分支的 JS 节点 ID；它的输出才是 mapper 消费的 items。
const CONTROL_ALPHA_NODE: u128 = 2_003;

/// 替换 `alpha` 分支的 JS 源码：只有这个节点的输出会进入 mapper 与媒体 delta。
fn definition_with_alpha_js(identity: &str, alpha_code: &str) -> RuleDefinition {
    let mut definition = definition_with_entry_js(
        identity,
        "JSON.stringify([{ enabled: true, title: '入口', url: 'https://example.invalid/entry' }])",
        JsBudget::default(),
    );
    let node = definition
        .flow_mut()
        .nodes
        .iter_mut()
        .find(|node| node.id == Uuid::from_u128(CONTROL_ALPHA_NODE))
        .expect("control 定义必须含 alpha 分支节点");
    node.config = FlowNodeConfig::Js(JsConfig {
        code: alpha_code.to_string(),
        output: JsOutputKind::Json,
        budgets: JsBudget::default(),
    });
    definition
}

/// 安装一版定义并返回更新后的 installed source revision。
async fn install_definition(
    system: &RuleSystem,
    definition: RuleDefinition,
    expected_installed_revision: u64,
) -> u64 {
    let candidate = system
        .stage_prepared_candidate(
            PreparedRuleInput {
                definition,
                runtime_credentials: None,
                display_title: Some("JS 结局 fixture".to_string()),
                display_group: None,
                diagnostics: Vec::new(),
            },
            expected_installed_revision,
            "trace-js-outcome-fixture",
        )
        .await
        .expect("JS 结局 fixture 必须可通过 gate");
    system
        .install(candidate.id)
        .await
        .expect("JS 结局 fixture 必须可安装")
        .revision
}

/// 安装并执行一个 JS 结局用例，断言其 execution 终态与 capture 位置。
async fn run_js_outcome_case(
    system: &RuleSystem,
    identity: &str,
    case: &JsOutcomeCase,
    installed_revision: u64,
) -> u64 {
    let installed_revision = install_definition(
        system,
        definition_with_entry_js(identity, case.entry_code, case.entry_budgets),
        installed_revision,
    )
    .await;
    let session = system
        .execute(ExecuteRequest {
            source_id: SourceId::from_identity(identity.to_string()),
            intent: StandardIntent::Search,
            input: IntentInput::Query(case.label.to_string()),
            mode: ExecutionMode::Live,
        })
        .await
        .unwrap_or_else(|error| panic!("JS 结局 {} 必须能启动 execution: {error:?}", case.label));
    let cancellation = session.cancellation_handle();
    let completion = session.into_events();
    if let Some(delay) = case.cancel_after {
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            let _ = cancellation.cancel();
        });
    }
    let events = completion.collect::<Vec<_>>().await;
    let captures = events
        .iter()
        .filter(|event| matches!(event.kind, ExecutionEventKind::EffectCaptured { .. }))
        .count();
    match case.captures {
        CaptureExpectation::Any => assert!(
            captures >= 1,
            "JS 结局 {} 应至少归档一个 effect capture, 实际 {captures}",
            case.label
        ),
        CaptureExpectation::Exactly(expected) => assert_eq!(
            captures, expected,
            "JS 结局 {} 的 capture 数说明 Plan 没有停在预期位置",
            case.label
        ),
    }

    let last = events.last().map(|event| &event.kind);
    match case.terminal {
        JsOutcomeTerminal::Completed => assert!(
            matches!(last, Some(ExecutionEventKind::Completed)),
            "JS 结局 {} 应正常完成, 实际: {last:?}",
            case.label
        ),
        JsOutcomeTerminal::Cancelled => assert!(
            matches!(last, Some(ExecutionEventKind::Cancelled)),
            "JS 结局 {} 应被取消, 实际: {last:?}",
            case.label
        ),
        JsOutcomeTerminal::Failed => {
            let Some(ExecutionEventKind::Failed { error }) = last else {
                panic!("JS 结局 {} 应失败, 实际: {last:?}", case.label);
            };
            assert_eq!(
                error.stage,
                RuleErrorStage::Execution,
                "JS 结局 {} 必须给出稳定 execution 失败终态",
                case.label
            );
            assert_eq!(error.code, "execution_failed");
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event.kind, ExecutionEventKind::DeltaCommitted { .. })),
                "JS 结局 {} 不得提交媒体 delta",
                case.label
            );
        }
    }
    installed_revision
}

/// 文档的 Draft/Effective Rule Revision 不因 JS 节点的任何一种结局而改变。
///
/// 覆盖 ticket 点名的五种结局：正常完成、异常、超时、取消、资源超限（内存与输出各一例）。
/// 每个用例先在真实 adapter 上证明脚本确实产出该结局，再经完整 lifecycle 执行，断言完整
/// 语义域快照（draft revision、effective revision、effective definition hash、draft
/// definition）整体不变；最后证明 revision 链仍可继续推进。
#[tokio::test]
async fn js_effect_outcomes_leave_draft_and_effective_rule_revisions_intact() {
    // execution 跑的是独立 installed source：文档是作者态 artifact, execution 消费已安装的
    // pinned source；两者内容相同但身份不同。
    const EXECUTED_IDENTITY: &str = "source:js-outcome-lifecycle";
    const NORMAL_ENTRY: &str =
        "JSON.stringify([{ enabled: true, title: '入口', url: 'https://example.invalid/entry' }])";
    let system = open_package_test_system().await;
    let created = system
        .create_native_rule_document(CreateNativeRuleDocumentRequest {
            mode: CreateMode::Template {
                title: "JS 结局文档".to_string(),
                intent: StandardIntent::Search,
                data_type: ExpectedDataType::Json,
                base_url: "https://example.invalid".to_string(),
            },
        })
        .await
        .expect("创建模板文档");
    let identity = created.source_identity.clone();
    let document_id = created.document_id.clone();

    // revision 2 = 含 JS 节点的有效定义, 成为 Effective Rule Revision。
    let saved = system
        .save_native_rule_document(SaveNativeRuleDocumentRequest {
            document_id: document_id.clone(),
            semantic: Some(SemanticSave {
                expected_revision: 1,
                definition: definition_with_entry_js(&identity, NORMAL_ENTRY, JsBudget::default()),
                credential_mutations: Vec::new(),
            }),
            layout: None,
        })
        .await
        .expect("保存有效 revision");
    assert_eq!(saved.semantic.expect("语义域结果").revision, 2);

    // revision 3 = 仍含 JS 节点但缺意图导出的草稿, 不得替换 Effective Rule Revision。
    let mut draft_definition = definition_with_entry_js(
        &identity,
        "JSON.stringify([{ title: '草稿', url: 'https://example.invalid/draft' }])",
        JsBudget::default(),
    );
    draft_definition.intent_exports_mut().clear();
    let saved = system
        .save_native_rule_document(SaveNativeRuleDocumentRequest {
            document_id: document_id.clone(),
            semantic: Some(SemanticSave {
                expected_revision: 2,
                definition: draft_definition.clone(),
                credential_mutations: Vec::new(),
            }),
            layout: None,
        })
        .await
        .expect("保存无效草稿");
    assert_eq!(
        saved.semantic.expect("语义域结果").activation,
        Some(crate::SemanticActivation::Draft),
        "无效草稿只能成为 Draft"
    );

    let before = document_revision_state(&system, &document_id).await;
    assert_eq!(before.semantic_revision, 3);
    assert_eq!(before.effective_revision, Some(2));
    assert_eq!(before.draft_definition.as_ref(), Some(&draft_definition));

    let mut installed_revision = 0;
    for case in js_outcome_cases() {
        assert_adapter_outcome(&case);
        installed_revision =
            run_js_outcome_case(&system, EXECUTED_IDENTITY, &case, installed_revision).await;
        assert_eq!(
            document_revision_state(&system, &document_id).await,
            before,
            "JS 结局 {} 破坏了 Rule Draft/Effective Revision 状态",
            case.label
        );
    }

    // revision 链未被扰动: 仍能以 draft revision 3 为基线推进到下一版有效 revision。
    let saved = system
        .save_native_rule_document(SaveNativeRuleDocumentRequest {
            document_id: document_id.clone(),
            semantic: Some(SemanticSave {
                expected_revision: 3,
                definition: definition_with_entry_js(&identity, NORMAL_ENTRY, JsBudget::default()),
                credential_mutations: Vec::new(),
            }),
            layout: None,
        })
        .await
        .expect("JS 结局之后 revision 链必须仍可推进");
    let outcome = saved.semantic.expect("语义域结果");
    assert_eq!(outcome.revision, 4);
    assert_eq!(
        outcome.activation,
        Some(crate::SemanticActivation::Effective),
        "有效 revision 必须替换 Effective Rule Revision"
    );
    let after = document_revision_state(&system, &document_id).await;
    assert_eq!(after.semantic_revision, 4);
    assert_eq!(after.effective_revision, Some(4));
    drop(system);
}

/// JS 变换节点够不到明文 source credential。
///
/// 真实 live execution 会把已解密的 source secret 带进 runtime；`alpha` 分支 JS 把所有可达
/// 全局与 `input` 原样回显到 item title，于是「脚本能看到什么」会一路进入媒体 delta 与交付
/// 事件。因此下面的正反断言都不是空转：正例证明回显真的到达交付面，反例证明明文不在其中。
#[tokio::test]
async fn js_effect_cannot_reach_plaintext_source_credentials() {
    const SECRET: &str = "plaintext-credential-must-not-be-reachable";
    const IDENTITY: &str = "source:js-credential-boundary";
    let system = open_package_test_system().await;
    let secret_bytes = serde_json::to_vec(&serde_json::json!({
        "authorization": format!("Bearer {SECRET}"),
    }))
    .expect("credential map 必须可序列化");
    let definition = definition_with_alpha_js(
        IDENTITY,
        r"JSON.stringify([{ title: JSON.stringify(Object.getOwnPropertyNames(globalThis).sort()) + '::'
                + typeof globalThis.input + '::' + JSON.stringify(globalThis.input) + '::'
                + typeof globalThis.credential + typeof globalThis.credentials
                + typeof globalThis.secret + typeof globalThis.token,
            url: 'https://example.invalid/alpha' }])",
    );
    let candidate = system
        .stage_prepared_candidate(
            PreparedRuleInput {
                definition,
                runtime_credentials: Some(secret_bytes),
                display_title: Some("凭据边界".to_string()),
                display_group: None,
                diagnostics: Vec::new(),
            },
            0,
            "trace-js-credential-boundary",
        )
        .await
        .expect("带 runtime credential 的 candidate 必须可暂存");
    let installed = system
        .install(candidate.id)
        .await
        .expect("candidate 必须可安装");
    let session = system
        .execute(ExecuteRequest {
            source_id: installed.source_id,
            intent: StandardIntent::Search,
            input: IntentInput::Query("credential-boundary".to_string()),
            mode: ExecutionMode::Live,
        })
        .await
        .expect("live execution 必须能启动");
    let events = session.into_events().collect::<Vec<_>>().await;
    assert_eq!(
        events.last().map(|event| &event.kind),
        Some(&ExecutionEventKind::Completed),
        "凭据边界用例必须先正常完成"
    );
    let delivered = serde_json::to_string(&events).expect("交付事件必须可序列化");
    assert!(
        delivered.contains("AggregateError"),
        "JS 回显的全局名单必须真的进入交付事件, 否则本断言是空转"
    );
    assert!(
        delivered.contains("undefinedundefinedundefinedundefined"),
        "JS 对 credential 类全局的探测必须真的执行过"
    );
    assert!(
        !delivered.contains(SECRET),
        "明文 credential 不得进入 JS 可达表面或任何交付事件"
    );
    assert!(
        !delivered.contains("Bearer "),
        "明文 credential header 不得出现在交付事件里"
    );
    drop(system);
}
