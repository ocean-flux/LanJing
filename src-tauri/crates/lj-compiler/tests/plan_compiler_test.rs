//! Compiler current-contract tests：typed Plan、deterministic hash 与 stable diagnostics。

use std::collections::{BTreeMap, HashMap};

use lj_capability::{IntentExport, StandardIntent};
use lj_compiler::Compiler;
use lj_rule_model::{
    CONDITION_INPUT_HANDLE, CanonicalNumber, CapabilityManifest, CollectionSelector,
    ConditionConfig, ConditionPredicate, ControlExpression, ControlRegion, ControlledMapper,
    DiagnosticSeverity, EffectKind, ExpectedDataType, ExtractSpec, FlowEdge, FlowGraph, FlowNode,
    FlowNodeConfig, FlowPortRef, ForEachConfig, HttpMethod, HttpSpec, JsConfig, JsOutputKind,
    LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE, LOOP_BODY_HANDLE, LOOP_COLLECTION_HANDLE,
    LOOP_DONE_HANDLE, LOOP_YIELD_HANDLE, LoopIterationLimit, MAX_LOOP_ITERATIONS,
    MERGE_OUTPUT_HANDLE, MapperOutputKind, MergeConfig, MergeInput, MergeInputActivation,
    MergeStrategy, OutputTarget, PlanNode, PlanNodeConfig, PolicyCapabilities, PortValueKind,
    PortValueType, RULE_CONTRACT_SCHEMA_VERSION, RuleDefinition, SourceIdentity, SourceSpan,
    SystemCapabilities, TypedLiteral, UnavailableNodeConfig,
};
use serde_json::json;
use uuid::Uuid;

const JS: u128 = 1;
const CONDITION: u128 = 2;
const MERGE: u128 = 3;
const LOOP: u128 = 4;
const HTTP: u128 = 5;
const EXTRACT: u128 = 6;
const MAPPER: u128 = 7;
const INNER_LOOP: u128 = 8;

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn port(node: u128, handle: &str) -> FlowPortRef {
    FlowPortRef::new(id(node), handle)
}

fn edge(from_node: u128, from_handle: &str, to_node: u128, to_handle: &str) -> FlowEdge {
    FlowEdge::new(port(from_node, from_handle), port(to_node, to_handle))
}

fn http_config() -> HttpSpec {
    HttpSpec {
        method: HttpMethod::Get,
        url: "https://example.com/search?q={{key}}".to_string(),
        headers: HashMap::new(),
        body: None,
        charset: None,
        expected_type: ExpectedDataType::Html,
    }
}

fn extract_config() -> ExtractSpec {
    ExtractSpec {
        rules: Vec::new(),
        field_rules: HashMap::new(),
        expected_type: ExpectedDataType::Html,
        output_target: OutputTarget::default(),
    }
}

fn mapper_config() -> ControlledMapper {
    ControlledMapper {
        output: MapperOutputKind::Items,
        identity_fields: vec!["book_url".to_string()],
    }
}

fn typed_condition_expression() -> ControlExpression {
    ControlExpression::Typed {
        predicate: ConditionPredicate::Eq {
            pointer: "/enabled".to_string(),
            value: TypedLiteral::Bool(true),
        },
        true_branch: "alpha".to_string(),
        false_branch: "beta".to_string(),
    }
}

fn loop_config() -> ForEachConfig {
    ForEachConfig {
        collection: CollectionSelector::Typed {
            pointer: String::new(),
        },
        item_binding: "item".to_string(),
        index_binding: "index".to_string(),
        max_iterations: 8,
    }
}

fn manifest() -> CapabilityManifest {
    CapabilityManifest {
        required: PolicyCapabilities {
            network: true,
            system: SystemCapabilities::default(),
        },
    }
}

fn merge_inputs() -> Vec<MergeInput> {
    // 物理声明顺序故意与 order 相反，证明 order 才承载语义。
    vec![
        MergeInput {
            input_id: "in_beta".to_string(),
            handle: "beta".to_string(),
            order: 1,
            activation: MergeInputActivation::Optional,
        },
        MergeInput {
            input_id: "in_alpha".to_string(),
            handle: "alpha".to_string(),
            order: 0,
            activation: MergeInputActivation::Required,
        },
    ]
}

fn definition(nodes: Vec<FlowNode>, edges: Vec<FlowEdge>, entry: u128) -> RuleDefinition {
    let mut intent_exports = BTreeMap::new();
    intent_exports.insert(
        StandardIntent::Search,
        IntentExport::new(id(entry), id(MAPPER)),
    );
    RuleDefinition::new(
        SourceIdentity {
            id: "source:compiler-current".to_string(),
        },
        "https://example.com",
        intent_exports,
        FlowGraph { nodes, edges },
        manifest(),
        vec!["book_url".to_string()],
    )
}

fn linear_definition() -> RuleDefinition {
    definition(
        vec![
            FlowNode::new(id(HTTP), FlowNodeConfig::Http(http_config())),
            FlowNode::new(id(EXTRACT), FlowNodeConfig::Extract(extract_config())),
            FlowNode::new(id(MAPPER), FlowNodeConfig::Mapper(mapper_config())),
        ],
        vec![
            edge(HTTP, LINEAR_OUTPUT_HANDLE, EXTRACT, LINEAR_INPUT_HANDLE),
            edge(EXTRACT, LINEAR_OUTPUT_HANDLE, MAPPER, LINEAR_INPUT_HANDLE),
        ],
        HTTP,
    )
}

fn seven_node_definition(expression: ControlExpression) -> RuleDefinition {
    definition(
        vec![
            FlowNode::new(
                id(JS),
                FlowNodeConfig::Js(JsConfig::new(
                    "JSON.stringify([{ enabled: true }])".to_string(),
                    JsOutputKind::Json,
                )),
            ),
            FlowNode::new(
                id(CONDITION),
                FlowNodeConfig::Condition(ConditionConfig {
                    branches: vec!["beta".to_string(), "alpha".to_string()],
                    expression,
                }),
            ),
            FlowNode::new(
                id(MERGE),
                FlowNodeConfig::Merge(MergeConfig {
                    inputs: merge_inputs(),
                    strategy: MergeStrategy::CollectArray,
                }),
            ),
            FlowNode::new(id(LOOP), FlowNodeConfig::Loop(loop_config())),
            FlowNode::new(id(HTTP), FlowNodeConfig::Http(http_config())),
            FlowNode::new(id(EXTRACT), FlowNodeConfig::Extract(extract_config())),
            FlowNode::new(id(MAPPER), FlowNodeConfig::Mapper(mapper_config())),
        ],
        vec![
            edge(JS, LINEAR_OUTPUT_HANDLE, CONDITION, CONDITION_INPUT_HANDLE),
            edge(CONDITION, "alpha", MERGE, "alpha"),
            edge(CONDITION, "beta", MERGE, "beta"),
            edge(MERGE, MERGE_OUTPUT_HANDLE, LOOP, LOOP_COLLECTION_HANDLE),
            edge(LOOP, LOOP_BODY_HANDLE, HTTP, LINEAR_INPUT_HANDLE),
            edge(HTTP, LINEAR_OUTPUT_HANDLE, EXTRACT, LINEAR_INPUT_HANDLE),
            edge(EXTRACT, LINEAR_OUTPUT_HANDLE, LOOP, LOOP_YIELD_HANDLE),
            edge(LOOP, LOOP_DONE_HANDLE, MAPPER, LINEAR_INPUT_HANDLE),
        ],
        JS,
    )
}

fn node_mut(definition: &mut RuleDefinition, node_id: u128) -> &mut FlowNode {
    definition
        .flow_mut()
        .nodes
        .iter_mut()
        .find(|node| node.id == id(node_id))
        .expect("fixture node exists")
}

fn plan_node(plan_nodes: &[PlanNode], node_id: u128) -> &PlanNode {
    plan_nodes
        .iter()
        .find(|node| node.id == id(node_id))
        .expect("compiled Plan node exists")
}

fn assert_rejects(definition: &RuleDefinition, expected_code: &str) {
    let error = Compiler::default()
        .compile(definition)
        .expect_err("invalid current Definition must not compile");
    let diagnostic = error
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == expected_code)
        .unwrap_or_else(|| {
            panic!(
                "expected diagnostic {expected_code}, got {:?}",
                error.diagnostics()
            )
        });
    let path = diagnostic
        .span
        .as_ref()
        .and_then(|span| span.path.as_deref())
        .expect("blocking compiler diagnostic has a SourceSpan path");
    assert!(path.starts_with('/'), "diagnostic path was {path}");
}

fn plan_hash(definition: &RuleDefinition) -> String {
    Compiler::with_version("test-compiler@1".to_string())
        .compile(definition)
        .expect("valid fixture compiles")
        .plan_hash()
        .to_string()
}

fn assert_hash_change(
    base: &RuleDefinition,
    label: &str,
    mutate: impl FnOnce(&mut RuleDefinition),
) {
    let baseline = plan_hash(base);
    let mut changed = base.clone();
    mutate(&mut changed);
    let changed_hash = plan_hash(&changed);
    assert_ne!(baseline, changed_hash, "{label} must change Plan hash");
}

#[test]
fn base_linear_graph_writes_only_current_typed_plan() {
    let plan = Compiler::with_version("test-compiler@1".to_string())
        .compile(&linear_definition())
        .expect("valid linear current Definition compiles");

    assert!(!plan.has_control_flow());
    assert_eq!(plan.nodes().len(), 3);
    assert_eq!(plan.edges().len(), 2);
    assert_eq!(plan.plan_hash().len(), 64);
    assert_eq!(plan.definition_hash().len(), 64);

    let serialized = serde_json::to_string(&plan).expect("current Plan serializes");
    let json: serde_json::Value = serde_json::from_str(&serialized).expect("Plan JSON parses");
    assert_eq!(
        json.get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(u64::from(RULE_CONTRACT_SCHEMA_VERSION))
    );
    for forbidden in ["type_tag", "layout", "viewport", "position"] {
        assert!(
            !serialized.contains(forbidden),
            "Plan leaked non-contract field {forbidden}"
        );
    }
    assert!(matches!(
        &plan_node(plan.nodes(), HTTP).outputs[0].value_type,
        PortValueType::Kind {
            kind: PortValueKind::HttpResponse
        }
    ));
    assert!(matches!(
        &plan_node(plan.nodes(), EXTRACT).outputs[0].value_type,
        PortValueType::Kind {
            kind: PortValueKind::Json
        }
    ));
    assert!(matches!(
        &plan_node(plan.nodes(), MAPPER).outputs[0].value_type,
        PortValueType::Kind {
            kind: PortValueKind::Delta
        }
    ));
}

#[test]
fn good_seven_node_graph_compiles_typed_configs_edges_ports_and_loop_region() {
    let plan = Compiler::with_version("test-compiler@1".to_string())
        .compile(&seven_node_definition(typed_condition_expression()))
        .expect("all seven typed node contracts compile");

    assert_eq!(plan.nodes().len(), 7);
    assert_eq!(plan.edges().len(), 8);
    assert!(plan.has_control_flow());
    assert!(matches!(
        &plan_node(plan.nodes(), HTTP).config,
        PlanNodeConfig::Http(_)
    ));
    assert!(matches!(
        &plan_node(plan.nodes(), JS).config,
        PlanNodeConfig::Js(_)
    ));
    assert!(matches!(
        &plan_node(plan.nodes(), EXTRACT).config,
        PlanNodeConfig::Extract(_)
    ));
    assert!(matches!(
        &plan_node(plan.nodes(), MAPPER).config,
        PlanNodeConfig::Mapper(_)
    ));
    assert!(matches!(
        &plan_node(plan.nodes(), MERGE).config,
        PlanNodeConfig::Merge(_)
    ));
    assert!(matches!(
        &plan_node(plan.nodes(), CONDITION).config,
        PlanNodeConfig::Condition(_)
    ));
    assert!(matches!(
        &plan_node(plan.nodes(), LOOP).config,
        PlanNodeConfig::Loop(_)
    ));

    let merge = plan_node(plan.nodes(), MERGE);
    // ports 按显式 order 排列，而非 handle 字典序/物理声明序。
    assert_eq!(
        merge
            .inputs
            .iter()
            .map(|port| port.handle.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "beta"]
    );
    let condition = plan_node(plan.nodes(), CONDITION);
    assert_eq!(condition.inputs[0].handle, CONDITION_INPUT_HANDLE);
    assert_eq!(
        condition
            .outputs
            .iter()
            .map(|port| port.handle.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "beta"]
    );
    let loop_node = plan_node(plan.nodes(), LOOP);
    assert_eq!(
        loop_node
            .inputs
            .iter()
            .map(|port| port.handle.as_str())
            .collect::<Vec<_>>(),
        [LOOP_COLLECTION_HANDLE, LOOP_YIELD_HANDLE]
    );
    assert_eq!(
        loop_node
            .outputs
            .iter()
            .map(|port| port.handle.as_str())
            .collect::<Vec<_>>(),
        [LOOP_BODY_HANDLE, LOOP_DONE_HANDLE]
    );

    let [ControlRegion::Loop(region)] = plan.control_regions() else {
        panic!("bounded Loop compiles to one typed control region");
    };
    assert_eq!(region.loop_node, id(LOOP));
    assert_eq!(
        region.body_entry,
        FlowPortRef::new(id(HTTP), LINEAR_INPUT_HANDLE)
    );
    assert_eq!(
        region.yield_source,
        FlowPortRef::new(id(EXTRACT), LINEAR_OUTPUT_HANDLE)
    );
    assert_eq!(region.body_nodes, [id(HTTP), id(EXTRACT)]);

    assert!(
        plan.effects()
            .iter()
            .any(|effect| { effect.node_id == id(JS) && effect.kind == EffectKind::QuickJs })
    );
    assert!(
        plan.effects()
            .iter()
            .any(|effect| { effect.node_id == id(HTTP) && effect.kind == EffectKind::Http })
    );
    assert!(
        plan.effects()
            .iter()
            .any(|effect| { effect.node_id == id(EXTRACT) && effect.kind == EffectKind::Extract })
    );
    assert_eq!(plan.capability_requirements(), ["network"]);
}

#[test]
fn typed_and_js_condition_and_typed_and_js_loop_emit_explicit_quickjs_effects() {
    let typed = Compiler::default()
        .compile(&seven_node_definition(typed_condition_expression()))
        .expect("typed controls compile");
    assert!(
        !typed
            .effects()
            .iter()
            .any(|effect| effect.node_id == id(CONDITION) || effect.node_id == id(LOOP))
    );

    let js_condition = Compiler::default()
        .compile(&seven_node_definition(ControlExpression::Js {
            code: "'alpha'".to_string(),
        }))
        .expect("JS Condition compiles");
    assert!(
        js_condition.effects().iter().any(|effect| {
            effect.node_id == id(CONDITION) && effect.kind == EffectKind::QuickJs
        })
    );

    let mut js_loop_definition = seven_node_definition(typed_condition_expression());
    let FlowNodeConfig::Loop(config) = &mut node_mut(&mut js_loop_definition, LOOP).config else {
        panic!("fixture Loop exists");
    };
    config.collection = CollectionSelector::Js {
        code: "input".to_string(),
    };
    let js_loop = Compiler::default()
        .compile(&js_loop_definition)
        .expect("JS Loop selector compiles");
    assert!(
        js_loop
            .effects()
            .iter()
            .any(|effect| { effect.node_id == id(LOOP) && effect.kind == EffectKind::QuickJs })
    );
}

#[test]
fn control_script_source_is_not_echoed_in_diagnostics() {
    let secret_source = "control-secret-must-not-leak";
    let mut definition = seven_node_definition(ControlExpression::Js {
        code: secret_source.to_string(),
    });
    definition.capability_manifest_mut().required.network = false;

    let error = Compiler::default()
        .compile(&definition)
        .expect_err("missing control capability must be rejected");
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "CAPABILITY_MISMATCH")
    );
    assert!(error.diagnostics().iter().all(|diagnostic| {
        !diagnostic.message.contains(secret_source)
            && diagnostic
                .span
                .as_ref()
                .and_then(|span| span.path.as_deref())
                .is_none_or(|path| !path.contains(secret_source))
    }));
}

#[test]
fn unavailable_node_capability_is_rejected_without_producing_a_plan() {
    let mut definition = seven_node_definition(typed_condition_expression());
    node_mut(&mut definition, JS).config = FlowNodeConfig::Unavailable(UnavailableNodeConfig::new(
        "custom_reader",
        json!({ "selector": ".entry" }),
    ));

    let diagnostics = lj_compiler::validate(&definition);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "NODE_CAPABILITY_UNAVAILABLE"
            && diagnostic.severity == DiagnosticSeverity::Error
    }));

    let error = Compiler::default()
        .compile(&definition)
        .expect_err("未安装能力节点不得产出 immutable Plan");
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "NODE_CAPABILITY_UNAVAILABLE")
    );
}

#[test]
fn declaration_reordering_and_source_spans_do_not_change_definition_or_plan_hash() {
    let compiler = Compiler::with_version("test-compiler@1".to_string());
    let base = seven_node_definition(typed_condition_expression());
    let mut reordered = base.clone();
    let flow = reordered.flow_mut();
    flow.nodes.reverse();
    flow.edges.reverse();
    for node in &mut flow.nodes {
        match &mut node.config {
            FlowNodeConfig::Merge(config) => config.inputs.reverse(),
            FlowNodeConfig::Condition(config) => config.branches.reverse(),
            FlowNodeConfig::Http(_)
            | FlowNodeConfig::Js(_)
            | FlowNodeConfig::Extract(_)
            | FlowNodeConfig::Mapper(_)
            | FlowNodeConfig::Loop(_)
            | FlowNodeConfig::Unavailable(_) => {}
        }
    }

    let base_plan = compiler.compile(&base).expect("base compiles");
    let reordered_plan = compiler.compile(&reordered).expect("reordered compiles");
    assert_eq!(
        base_plan.definition_hash(),
        reordered_plan.definition_hash()
    );
    assert_eq!(base_plan.plan_hash(), reordered_plan.plan_hash());

    let mut relocated = base.clone();
    node_mut(&mut relocated, HTTP).span = Some(SourceSpan {
        start: 41,
        end: 99,
        path: Some("/editor/nodes/4".to_string()),
    });
    let relocated_plan = compiler.compile(&relocated).expect("relocated compiles");
    assert_eq!(
        base_plan.definition_hash(),
        relocated_plan.definition_hash()
    );
    assert_eq!(base_plan.plan_hash(), relocated_plan.plan_hash());

    let another_compiler = Compiler::with_version("alternate-test-compiler@1".to_string())
        .compile(&base)
        .expect("same Definition compiles with another compiler identity");
    assert_eq!(
        base_plan.definition_hash(),
        another_compiler.definition_hash()
    );
    assert_ne!(base_plan.plan_hash(), another_compiler.plan_hash());
}

#[test]
fn every_node_config_and_semantic_edge_change_changes_plan_hash() {
    let base = seven_node_definition(typed_condition_expression());

    assert_hash_change(&base, "Http config", |definition| {
        let FlowNodeConfig::Http(config) = &mut node_mut(definition, HTTP).config else {
            panic!("fixture Http exists");
        };
        config.url.push_str("&page=2");
    });
    assert_hash_change(&base, "Js config", |definition| {
        let FlowNodeConfig::Js(config) = &mut node_mut(definition, JS).config else {
            panic!("fixture Js exists");
        };
        config.code.push_str(";void 0");
    });
    assert_hash_change(&base, "Extract config", |definition| {
        let FlowNodeConfig::Extract(config) = &mut node_mut(definition, EXTRACT).config else {
            panic!("fixture Extract exists");
        };
        config.expected_type = ExpectedDataType::Json;
    });
    assert_hash_change(&base, "Mapper config", |definition| {
        let FlowNodeConfig::Mapper(config) = &mut node_mut(definition, MAPPER).config else {
            panic!("fixture Mapper exists");
        };
        config.output = MapperOutputKind::Units;
    });
    assert_hash_change(&base, "Merge strategy", |definition| {
        let FlowNodeConfig::Merge(config) = &mut node_mut(definition, MERGE).config else {
            panic!("fixture Merge exists");
        };
        config.strategy = MergeStrategy::ConcatArrays;
    });
    assert_hash_change(&base, "Merge input identity", |definition| {
        let FlowNodeConfig::Merge(config) = &mut node_mut(definition, MERGE).config else {
            panic!("fixture Merge exists");
        };
        config.inputs[0].input_id.push_str("-changed");
    });
    assert_hash_change(&base, "Merge activation", |definition| {
        let FlowNodeConfig::Merge(config) = &mut node_mut(definition, MERGE).config else {
            panic!("fixture Merge exists");
        };
        config.inputs[0].activation = MergeInputActivation::Required;
    });
    assert_hash_change(&base, "Merge explicit order", |definition| {
        let FlowNodeConfig::Merge(config) = &mut node_mut(definition, MERGE).config else {
            panic!("fixture Merge exists");
        };
        for input in &mut config.inputs {
            input.order = 1 - input.order;
        }
    });
    assert_hash_change(&base, "Condition predicate", |definition| {
        let FlowNodeConfig::Condition(config) = &mut node_mut(definition, CONDITION).config else {
            panic!("fixture Condition exists");
        };
        config.expression = ControlExpression::Typed {
            predicate: ConditionPredicate::Eq {
                pointer: "/enabled".to_string(),
                value: TypedLiteral::Bool(false),
            },
            true_branch: "alpha".to_string(),
            false_branch: "beta".to_string(),
        };
    });
    assert_hash_change(&base, "Loop max_iterations", |definition| {
        let FlowNodeConfig::Loop(config) = &mut node_mut(definition, LOOP).config else {
            panic!("fixture Loop exists");
        };
        config.max_iterations = 9;
    });
    assert_hash_change(&base, "semantic edge handles", |definition| {
        let flow = definition.flow_mut();
        for edge in &mut flow.edges {
            if edge.from.node_id != id(CONDITION) {
                continue;
            }
            edge.to.handle = match edge.to.handle.as_str() {
                "alpha" => "beta".to_string(),
                "beta" => "alpha".to_string(),
                other => other.to_string(),
            };
        }
    });
}

#[test]
fn bad_config_handle_port_intent_and_capability_contracts_have_stable_paths() {
    let mut duplicate_node = linear_definition();
    let duplicate = duplicate_node.flow().nodes[0].clone();
    duplicate_node.flow_mut().nodes.push(duplicate);
    assert_rejects(&duplicate_node, "DUPLICATE_NODE_ID");

    let mut empty_js = seven_node_definition(typed_condition_expression());
    let FlowNodeConfig::Js(config) = &mut node_mut(&mut empty_js, JS).config else {
        panic!("fixture Js exists");
    };
    config.code.clear();
    assert_rejects(&empty_js, "NODE_CONFIG_INVALID");

    let mut duplicate_edge = linear_definition();
    let duplicate = duplicate_edge.flow().edges[0].clone();
    duplicate_edge.flow_mut().edges.push(duplicate);
    assert_rejects(&duplicate_edge, "DUPLICATE_EDGE");

    let mut missing_handle = linear_definition();
    missing_handle.flow_mut().edges[0].to.handle = "missing".to_string();
    assert_rejects(&missing_handle, "TARGET_HANDLE_MISSING");

    let mut incompatible = linear_definition();
    incompatible.flow_mut().edges = vec![edge(
        HTTP,
        LINEAR_OUTPUT_HANDLE,
        MAPPER,
        LINEAR_INPUT_HANDLE,
    )];
    assert_rejects(&incompatible, "PORT_TYPE_MISMATCH");

    let mut unreachable = linear_definition();
    unreachable.flow_mut().edges.clear();
    assert_rejects(&unreachable, "MAPPER_UNREACHABLE");

    let mut invalid_entry = seven_node_definition(typed_condition_expression());
    invalid_entry
        .intent_exports_mut()
        .get_mut(&StandardIntent::Search)
        .expect("Search export exists")
        .flow_entry = id(CONDITION);
    assert_rejects(&invalid_entry, "INTENT_ENTRY_PORT_MISMATCH");

    let mut capability = linear_definition();
    capability.capability_manifest_mut().required.network = false;
    assert_rejects(&capability, "CAPABILITY_MISMATCH");
}

#[test]
fn bad_condition_and_merge_contracts_are_locatable() {
    let mut duplicate_branch = seven_node_definition(typed_condition_expression());
    let FlowNodeConfig::Condition(config) = &mut node_mut(&mut duplicate_branch, CONDITION).config
    else {
        panic!("fixture Condition exists");
    };
    config.branches = vec!["alpha".to_string(), "alpha".to_string()];
    assert_rejects(&duplicate_branch, "CONDITION_BRANCH_DUPLICATE");

    let mut missing_branch = seven_node_definition(typed_condition_expression());
    let FlowNodeConfig::Condition(config) = &mut node_mut(&mut missing_branch, CONDITION).config
    else {
        panic!("fixture Condition exists");
    };
    config.branches.retain(|branch| branch == "alpha");
    assert_rejects(&missing_branch, "CONDITION_BRANCH_MISSING");

    let mut invalid_operator = seven_node_definition(typed_condition_expression());
    let FlowNodeConfig::Condition(config) = &mut node_mut(&mut invalid_operator, CONDITION).config
    else {
        panic!("fixture Condition exists");
    };
    config.expression = ControlExpression::Typed {
        predicate: ConditionPredicate::Lt {
            pointer: "/rank".to_string(),
            value: TypedLiteral::String("10".to_string()),
        },
        true_branch: "alpha".to_string(),
        false_branch: "beta".to_string(),
    };
    assert_rejects(&invalid_operator, "CONDITION_OPERATOR_INVALID");

    let mut invalid_pointer = seven_node_definition(typed_condition_expression());
    let FlowNodeConfig::Condition(config) = &mut node_mut(&mut invalid_pointer, CONDITION).config
    else {
        panic!("fixture Condition exists");
    };
    config.expression = ControlExpression::Typed {
        predicate: ConditionPredicate::Exists {
            pointer: "not/a/pointer".to_string(),
        },
        true_branch: "alpha".to_string(),
        false_branch: "beta".to_string(),
    };
    assert_rejects(&invalid_pointer, "CONDITION_POINTER_INVALID");

    let mut empty_merge_handle = seven_node_definition(typed_condition_expression());
    let FlowNodeConfig::Merge(config) = &mut node_mut(&mut empty_merge_handle, MERGE).config else {
        panic!("fixture Merge exists");
    };
    config.inputs[0].handle.clear();
    assert_rejects(&empty_merge_handle, "MERGE_INPUT_HANDLE_INVALID");

    let mut duplicate_merge = seven_node_definition(typed_condition_expression());
    let FlowNodeConfig::Merge(config) = &mut node_mut(&mut duplicate_merge, MERGE).config else {
        panic!("fixture Merge exists");
    };
    config.inputs[1].handle = config.inputs[0].handle.clone();
    assert_rejects(&duplicate_merge, "MERGE_INPUT_DUPLICATE");

    let mut empty_input_id = seven_node_definition(typed_condition_expression());
    let FlowNodeConfig::Merge(config) = &mut node_mut(&mut empty_input_id, MERGE).config else {
        panic!("fixture Merge exists");
    };
    config.inputs[0].input_id.clear();
    assert_rejects(&empty_input_id, "MERGE_INPUT_ID_INVALID");

    let mut duplicate_input_id = seven_node_definition(typed_condition_expression());
    let FlowNodeConfig::Merge(config) = &mut node_mut(&mut duplicate_input_id, MERGE).config else {
        panic!("fixture Merge exists");
    };
    config.inputs[1].input_id = config.inputs[0].input_id.clone();
    assert_rejects(&duplicate_input_id, "MERGE_INPUT_ID_DUPLICATE");

    let mut invalid_order = seven_node_definition(typed_condition_expression());
    let FlowNodeConfig::Merge(config) = &mut node_mut(&mut invalid_order, MERGE).config else {
        panic!("fixture Merge exists");
    };
    config.inputs[0].order = 2;
    assert_rejects(&invalid_order, "MERGE_ORDER_INVALID");

    let mut duplicate_order = seven_node_definition(typed_condition_expression());
    let FlowNodeConfig::Merge(config) = &mut node_mut(&mut duplicate_order, MERGE).config else {
        panic!("fixture Merge exists");
    };
    config.inputs[0].order = config.inputs[1].order;
    assert_rejects(&duplicate_order, "MERGE_ORDER_INVALID");

    let mut missing_merge_input = seven_node_definition(typed_condition_expression());
    missing_merge_input
        .flow_mut()
        .edges
        .retain(|edge| !(edge.to.node_id == id(MERGE) && edge.to.handle == "beta"));
    assert_rejects(&missing_merge_input, "MERGE_INPUT_POLICY_INVALID");
}

#[test]
fn bad_loop_boundaries_cross_region_cycles_and_nesting_are_locatable() {
    assert!(LoopIterationLimit::new(1).is_ok());
    assert!(LoopIterationLimit::new(MAX_LOOP_ITERATIONS).is_ok());
    assert!(LoopIterationLimit::new(0).is_err());
    assert!(LoopIterationLimit::new(MAX_LOOP_ITERATIONS + 1).is_err());

    let mut unbounded = seven_node_definition(typed_condition_expression());
    let FlowNodeConfig::Loop(config) = &mut node_mut(&mut unbounded, LOOP).config else {
        panic!("fixture Loop exists");
    };
    config.max_iterations = 0;
    assert_rejects(&unbounded, "LOOP_MAX_ITERATIONS_INVALID");

    let mut missing_collection = seven_node_definition(typed_condition_expression());
    missing_collection
        .flow_mut()
        .edges
        .retain(|edge| !(edge.to.node_id == id(LOOP) && edge.to.handle == LOOP_COLLECTION_HANDLE));
    assert_rejects(&missing_collection, "LOOP_ENTRY_INVALID");

    let mut missing_body = seven_node_definition(typed_condition_expression());
    missing_body
        .flow_mut()
        .edges
        .retain(|edge| !(edge.from.node_id == id(LOOP) && edge.from.handle == LOOP_BODY_HANDLE));
    assert_rejects(&missing_body, "LOOP_BODY_INVALID");

    let mut missing_yield = seven_node_definition(typed_condition_expression());
    missing_yield
        .flow_mut()
        .edges
        .retain(|edge| !(edge.to.node_id == id(LOOP) && edge.to.handle == LOOP_YIELD_HANDLE));
    assert_rejects(&missing_yield, "LOOP_YIELD_INVALID");

    let mut missing_done = seven_node_definition(typed_condition_expression());
    missing_done
        .flow_mut()
        .edges
        .retain(|edge| !(edge.from.node_id == id(LOOP) && edge.from.handle == LOOP_DONE_HANDLE));
    assert_rejects(&missing_done, "LOOP_DONE_INVALID");

    let mut multiple_yield = seven_node_definition(typed_condition_expression());
    multiple_yield
        .flow_mut()
        .edges
        .push(edge(MERGE, MERGE_OUTPUT_HANDLE, LOOP, LOOP_YIELD_HANDLE));
    assert_rejects(&multiple_yield, "LOOP_YIELD_INVALID");

    let mut cross_region = seven_node_definition(typed_condition_expression());
    cross_region
        .flow_mut()
        .edges
        .push(edge(CONDITION, "alpha", HTTP, LINEAR_INPUT_HANDLE));
    assert_rejects(&cross_region, "LOOP_CROSS_REGION_EDGE");

    let mut body_bypass = seven_node_definition(typed_condition_expression());
    body_bypass.flow_mut().edges.push(edge(
        EXTRACT,
        LINEAR_OUTPUT_HANDLE,
        MAPPER,
        LINEAR_INPUT_HANDLE,
    ));
    assert_rejects(&body_bypass, "LOOP_BODY_BYPASS");

    let mut naked_cycle = linear_definition();
    naked_cycle.flow_mut().edges.push(edge(
        MAPPER,
        LINEAR_OUTPUT_HANDLE,
        HTTP,
        LINEAR_INPUT_HANDLE,
    ));
    assert_rejects(&naked_cycle, "FLOW_CYCLE_UNSTRUCTURED");

    let mut nested = seven_node_definition(typed_condition_expression());
    nested
        .flow_mut()
        .edges
        .retain(|edge| !(edge.from.node_id == id(LOOP) && edge.from.handle == LOOP_BODY_HANDLE));
    nested.flow_mut().nodes.push(FlowNode::new(
        id(INNER_LOOP),
        FlowNodeConfig::Loop(loop_config()),
    ));
    nested.flow_mut().edges.extend([
        edge(LOOP, LOOP_BODY_HANDLE, INNER_LOOP, LOOP_COLLECTION_HANDLE),
        edge(INNER_LOOP, LOOP_DONE_HANDLE, HTTP, LINEAR_INPUT_HANDLE),
    ]);
    assert_rejects(&nested, "LOOP_NESTING_UNSUPPORTED");
}

#[test]
fn canonical_number_literal_semantics_are_preserved_in_condition_hash_material() {
    let base = seven_node_definition(ControlExpression::Typed {
        predicate: ConditionPredicate::Eq {
            pointer: "/rank".to_string(),
            value: TypedLiteral::Number(CanonicalNumber::new(serde_json::Number::from(1))),
        },
        true_branch: "alpha".to_string(),
        false_branch: "beta".to_string(),
    });
    let same_numeric_value = seven_node_definition(ControlExpression::Typed {
        predicate: ConditionPredicate::Eq {
            pointer: "/rank".to_string(),
            value: TypedLiteral::Number(CanonicalNumber::new(
                serde_json::Number::from_f64(1.0).expect("finite JSON number"),
            )),
        },
        true_branch: "alpha".to_string(),
        false_branch: "beta".to_string(),
    });
    assert_eq!(
        plan_hash(&base),
        plan_hash(&same_numeric_value),
        "numeric equality must use canonical decimal value"
    );

    let different = seven_node_definition(ControlExpression::Typed {
        predicate: ConditionPredicate::Eq {
            pointer: "/rank".to_string(),
            value: TypedLiteral::Number(CanonicalNumber::new(serde_json::Number::from(2))),
        },
        true_branch: "alpha".to_string(),
        false_branch: "beta".to_string(),
    });
    assert_ne!(plan_hash(&base), plan_hash(&different));
}
