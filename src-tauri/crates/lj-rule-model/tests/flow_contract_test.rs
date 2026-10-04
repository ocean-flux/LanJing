//! RuleDefinition/ExecutionPlan 唯一 current 合同：七类节点、typed control、handles 与 serde golden。

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use lj_capability::{IntentExport, StandardIntent};
use lj_rule_model::{
    CONDITION_INPUT_HANDLE, CanonicalNumber, CapabilityManifest, CollectionSelector,
    ConditionConfig, ConditionOperator, ConditionPredicate, ControlExpression, ControlRegion,
    ControlledMapper, EffectDeclaration, EffectKind, ExecutionPlan, ExecutionPlanParts,
    ExpectedDataType, ExtractSpec, FlowEdge, FlowGraph, FlowNode, FlowNodeConfig, FlowNodeKind,
    FlowPortRef, ForEachConfig, HttpMethod, HttpSpec, JsConfig, JsOutputKind, LINEAR_INPUT_HANDLE,
    LINEAR_OUTPUT_HANDLE, LOOP_BODY_HANDLE, LOOP_COLLECTION_HANDLE, LOOP_DONE_HANDLE,
    LOOP_YIELD_HANDLE, LoopControlRegion, LoopIterationLimit, MAX_LOOP_ITERATIONS,
    MapperOutputKind, MergeConfig, MergeInput, MergeInputActivation, MergeStrategy, OutputTarget,
    PlanEdge, PlanForEachConfig, PlanNode, PlanNodeConfig, PlanPort, PortValueKind, PortValueType,
    RULE_CONTRACT_SCHEMA_VERSION, RuleDefinition, RulePackage, SchemaReadError, SourceIdentity,
    SourceSpan, TypedLiteral, canonical_json, canonical_json_deep_eq, canonical_number_cmp,
    canonical_number_eq, definition_hash, execution_plan_hash, read_execution_plan,
    read_rule_definition, read_rule_package, typed_literal_matches_json,
};
use serde_json::json;
use uuid::Uuid;

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn http_spec() -> HttpSpec {
    HttpSpec {
        method: HttpMethod::Get,
        url: "https://example.invalid/{{key}}".to_string(),
        headers: HashMap::new(),
        body: None,
        charset: None,
        expected_type: ExpectedDataType::Html,
    }
}

fn extract_spec() -> ExtractSpec {
    ExtractSpec {
        rules: vec![],
        field_rules: HashMap::new(),
        expected_type: ExpectedDataType::Html,
        output_target: OutputTarget::Media,
    }
}

fn merge_config(strategy: MergeStrategy) -> MergeConfig {
    MergeConfig {
        inputs: vec![
            MergeInput {
                input_id: "in_zeta".to_string(),
                handle: "zeta".to_string(),
                order: 1,
                activation: MergeInputActivation::Optional,
            },
            MergeInput {
                input_id: "in_alpha".to_string(),
                handle: "alpha".to_string(),
                order: 0,
                activation: MergeInputActivation::Required,
            },
        ],
        strategy,
    }
}

fn condition_config(predicate: ConditionPredicate) -> ConditionConfig {
    ConditionConfig {
        branches: vec!["false_branch".to_string(), "true_branch".to_string()],
        expression: ControlExpression::Typed {
            predicate,
            true_branch: "true_branch".to_string(),
            false_branch: "false_branch".to_string(),
        },
    }
}

fn loop_config(max_iterations: u16) -> ForEachConfig {
    ForEachConfig {
        collection: CollectionSelector::Typed {
            pointer: "/items".to_string(),
        },
        item_binding: "item".to_string(),
        index_binding: "index".to_string(),
        max_iterations,
    }
}

fn seven_nodes() -> Vec<FlowNode> {
    vec![
        FlowNode::new(id(1), FlowNodeConfig::Http(http_spec())).with_span(SourceSpan {
            start: 10,
            end: 20,
            path: Some("/flow/nodes/0".to_string()),
        }),
        FlowNode::new(
            id(2),
            FlowNodeConfig::Js(JsConfig::new(
                "JSON.stringify(input)".to_string(),
                JsOutputKind::Json,
            )),
        ),
        FlowNode::new(id(3), FlowNodeConfig::Extract(extract_spec())),
        FlowNode::new(
            id(4),
            FlowNodeConfig::Mapper(ControlledMapper {
                output: MapperOutputKind::Items,
                identity_fields: vec!["url".to_string(), "id".to_string()],
            }),
        ),
        FlowNode::new(
            id(5),
            FlowNodeConfig::Merge(merge_config(MergeStrategy::CollectArray)),
        ),
        FlowNode::new(
            id(6),
            FlowNodeConfig::Condition(condition_config(ConditionPredicate::Eq {
                pointer: "/enabled".to_string(),
                value: TypedLiteral::Bool(true),
            })),
        ),
        FlowNode::new(id(7), FlowNodeConfig::Loop(loop_config(8))),
    ]
}

fn control_definition() -> RuleDefinition {
    let mut intent_exports = BTreeMap::new();
    intent_exports.insert(StandardIntent::Search, IntentExport::new(id(1), id(4)));
    RuleDefinition::new(
        SourceIdentity {
            id: "source:current-control".to_string(),
        },
        "https://example.invalid",
        intent_exports,
        FlowGraph {
            nodes: seven_nodes(),
            edges: vec![
                FlowEdge::new(
                    FlowPortRef::new(id(1), LINEAR_OUTPUT_HANDLE),
                    FlowPortRef::new(id(3), LINEAR_INPUT_HANDLE),
                ),
                FlowEdge::new(
                    FlowPortRef::new(id(6), "true_branch"),
                    FlowPortRef::new(id(5), "alpha"),
                ),
                FlowEdge::new(
                    FlowPortRef::new(id(7), LOOP_BODY_HANDLE),
                    FlowPortRef::new(id(2), LINEAR_INPUT_HANDLE),
                ),
                FlowEdge::new(
                    FlowPortRef::new(id(2), LINEAR_OUTPUT_HANDLE),
                    FlowPortRef::new(id(7), LOOP_YIELD_HANDLE),
                ),
            ],
        },
        CapabilityManifest::default(),
        vec!["secondary_id".to_string(), "primary_id".to_string()],
    )
}

fn plan_config(config: &FlowNodeConfig) -> PlanNodeConfig {
    match config {
        FlowNodeConfig::Http(config) => PlanNodeConfig::Http(config.clone()),
        FlowNodeConfig::Js(config) => PlanNodeConfig::Js(config.clone()),
        FlowNodeConfig::Extract(config) => PlanNodeConfig::Extract(config.clone()),
        FlowNodeConfig::Mapper(config) => PlanNodeConfig::Mapper(config.clone()),
        FlowNodeConfig::Merge(config) => PlanNodeConfig::Merge(config.clone()),
        FlowNodeConfig::Condition(config) => PlanNodeConfig::Condition(config.clone()),
        FlowNodeConfig::Loop(config) => PlanNodeConfig::Loop(PlanForEachConfig::new(
            config.collection.clone(),
            config.item_binding.clone(),
            config.index_binding.clone(),
            LoopIterationLimit::new(u32::from(config.max_iterations)).unwrap(),
        )),
        FlowNodeConfig::Unavailable(_) => panic!("未安装能力不可编译为 Plan"),
    }
}

fn plan_node(node: &FlowNode) -> PlanNode {
    let json = PortValueType::kind(PortValueKind::Json);
    PlanNode {
        id: node.id,
        inputs: vec![PlanPort::new(LINEAR_INPUT_HANDLE, json.clone())],
        outputs: vec![PlanPort::new(LINEAR_OUTPUT_HANDLE, json)],
        config: plan_config(&node.config),
    }
}

fn control_plan(definition: &RuleDefinition) -> ExecutionPlan {
    let mut nodes = definition
        .flow()
        .nodes
        .iter()
        .map(plan_node)
        .collect::<Vec<_>>();
    let loop_node = nodes.iter_mut().find(|node| node.id == id(7)).unwrap();
    loop_node.inputs = vec![
        PlanPort::new(LOOP_YIELD_HANDLE, PortValueType::kind(PortValueKind::Json)),
        PlanPort::new(
            LOOP_COLLECTION_HANDLE,
            PortValueType::kind(PortValueKind::Json),
        ),
    ];
    loop_node.outputs = vec![
        PlanPort::new(LOOP_DONE_HANDLE, PortValueType::kind(PortValueKind::Json)),
        PlanPort::new(
            LOOP_BODY_HANDLE,
            PortValueType::kind(PortValueKind::LoopBinding),
        ),
    ];
    let condition = nodes.iter_mut().find(|node| node.id == id(6)).unwrap();
    condition.inputs = vec![PlanPort::new(
        CONDITION_INPUT_HANDLE,
        PortValueType::kind(PortValueKind::Json),
    )];
    condition.outputs = vec![
        PlanPort::new("true_branch", PortValueType::kind(PortValueKind::Json)),
        PlanPort::new("false_branch", PortValueType::kind(PortValueKind::Json)),
    ];

    let edges = definition
        .flow()
        .edges
        .iter()
        .cloned()
        .map(|edge| PlanEdge::new(edge.from, edge.to))
        .collect();
    let intent_entries = definition
        .intent_exports()
        .iter()
        .map(|(intent, export)| {
            (
                *intent,
                lj_rule_model::IntentEntry {
                    intent: *intent,
                    entry_node: export.flow_entry,
                    mapper_output: export.mapper_output,
                },
            )
        })
        .collect();
    ExecutionPlan::new(
        "model-current-test@1",
        definition_hash(definition).unwrap(),
        ExecutionPlanParts {
            nodes,
            edges,
            intent_entries,
            effects: vec![EffectDeclaration {
                node_id: id(1),
                kind: EffectKind::Http,
                required_capabilities: vec!["network".to_string()],
            }],
            capability_requirements: vec!["network".to_string()],
            control_regions: vec![ControlRegion::Loop(LoopControlRegion {
                loop_node: id(7),
                body_entry: FlowPortRef::new(id(2), LINEAR_INPUT_HANDLE),
                yield_source: FlowPortRef::new(id(2), LINEAR_OUTPUT_HANDLE),
                body_nodes: vec![id(3), id(2)],
            })],
        },
    )
    .unwrap()
}

#[test]
fn seven_flow_node_configs_roundtrip_as_closed_tagged_current() {
    let definition = control_definition();
    let kinds = definition
        .flow()
        .nodes
        .iter()
        .map(FlowNode::kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            Some(FlowNodeKind::Http),
            Some(FlowNodeKind::Js),
            Some(FlowNodeKind::Extract),
            Some(FlowNodeKind::Mapper),
            Some(FlowNodeKind::Merge),
            Some(FlowNodeKind::Condition),
            Some(FlowNodeKind::Loop),
        ]
    );

    let wire = serde_json::to_value(&definition).unwrap();
    assert_eq!(wire["schema_version"], RULE_CONTRACT_SCHEMA_VERSION);
    let wire_kinds = wire["flow"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["config"]["kind"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        wire_kinds,
        vec![
            "http",
            "js",
            "extract",
            "mapper",
            "merge",
            "condition",
            "loop"
        ]
    );
    assert_eq!(
        wire["flow"]["nodes"][1]["config"]["value"]["output"],
        "json"
    );
    assert!(wire["flow"]["nodes"][0].get("kind").is_none());
    assert_eq!(
        wire["flow"]["nodes"][4]["config"]["value"]["inputs"][0]["input_id"],
        "in_zeta"
    );
    assert_eq!(
        wire["flow"]["nodes"][4]["config"]["value"]["inputs"][0]["order"],
        1
    );
    let reread = read_rule_definition(&serde_json::to_vec(&wire).unwrap()).unwrap();
    assert_eq!(reread, definition);
}

#[test]
fn rule_package_round_trip_keeps_one_contract_and_canonical_definition_hash() {
    let definition = control_definition();
    let package = RulePackage::new(
        definition.source_identity().clone(),
        "v1",
        definition.clone(),
    );

    let bytes = serde_json::to_vec(&package).unwrap();
    let reread = read_rule_package(&bytes).unwrap();

    assert_eq!(reread, package);
    assert_eq!(reread.source_identity(), definition.source_identity());
    assert_eq!(reread.version(), "v1");
    assert_eq!(
        definition_hash(reread.definition()).unwrap(),
        definition_hash(&definition).unwrap()
    );
    // 每个内置能力都走同一 Rule Contract 的 typed wire。
    let kinds = |definition: &RuleDefinition| {
        definition
            .flow()
            .nodes
            .iter()
            .map(FlowNode::kind)
            .collect::<Vec<_>>()
    };
    assert_eq!(kinds(reread.definition()), kinds(&definition));
    assert!(kinds(&definition).iter().all(Option::is_some));
    // writer 幂等：读回再写出与首次写出逐字节一致。
    assert_eq!(serde_json::to_vec(&reread).unwrap(), bytes);
}

#[test]
fn execution_plan_reader_rejects_unknown_node_capability() {
    let definition = control_definition();
    let plan = control_plan(&definition);
    let mut value = serde_json::to_value(&plan).unwrap();
    value["nodes"][0]["config"] =
        json!({ "kind": "custom_reader", "value": { "selector": ".entry" } });

    let error = read_execution_plan(&serde_json::to_vec(&value).unwrap()).unwrap_err();
    assert_eq!(error.code(), "RULE_CONTRACT_INVALID_DATA");
    assert!(matches!(
        error,
        SchemaReadError::InvalidData {
            contract: lj_rule_model::SchemaContract::ExecutionPlan,
            ..
        }
    ));
}

#[test]
fn typed_and_js_control_configs_are_single_active_tagged_values() {
    let typed = condition_config(ConditionPredicate::Contains {
        pointer: "/tags".to_string(),
        value: TypedLiteral::String("rust".to_string()),
    });
    let typed_wire = serde_json::to_value(&typed).unwrap();
    assert_eq!(typed_wire["expression"]["mode"], "typed");
    assert_eq!(
        typed_wire["expression"]["predicate"]["operator"],
        "contains"
    );
    assert!(typed_wire["expression"].get("code").is_none());

    let js = ConditionConfig {
        branches: vec!["match".to_string(), "miss".to_string()],
        expression: ControlExpression::Js {
            code: "return input.ok ? 'match' : 'miss'".to_string(),
        },
    };
    let js_wire = serde_json::to_value(&js).unwrap();
    assert_eq!(js_wire["expression"]["mode"], "js");
    assert!(js_wire["expression"].get("predicate").is_none());
    let typed_loop = loop_config(u16::try_from(MAX_LOOP_ITERATIONS).unwrap());
    let typed_loop_wire = serde_json::to_value(&typed_loop).unwrap();
    assert_eq!(typed_loop_wire["collection"]["mode"], "typed");
    let js_loop = ForEachConfig {
        collection: CollectionSelector::Js {
            code: "return input.items".to_string(),
        },
        item_binding: "item".to_string(),
        index_binding: "index".to_string(),
        max_iterations: 16,
    };
    assert_eq!(
        serde_json::to_value(js_loop).unwrap()["collection"]["mode"],
        "js"
    );
}

#[test]
fn every_closed_condition_merge_js_and_port_variant_roundtrips() {
    let number = TypedLiteral::Number(CanonicalNumber::new(serde_json::Number::from(2)));
    let predicates = vec![
        ConditionPredicate::Exists {
            pointer: "/value".to_string(),
        },
        ConditionPredicate::IsNull {
            pointer: "/value".to_string(),
        },
        ConditionPredicate::Eq {
            pointer: "/value".to_string(),
            value: number.clone(),
        },
        ConditionPredicate::Ne {
            pointer: "/value".to_string(),
            value: number.clone(),
        },
        ConditionPredicate::Lt {
            pointer: "/value".to_string(),
            value: number.clone(),
        },
        ConditionPredicate::Lte {
            pointer: "/value".to_string(),
            value: number.clone(),
        },
        ConditionPredicate::Gt {
            pointer: "/value".to_string(),
            value: number.clone(),
        },
        ConditionPredicate::Gte {
            pointer: "/value".to_string(),
            value: number,
        },
        ConditionPredicate::Contains {
            pointer: "/value".to_string(),
            value: TypedLiteral::String("needle".to_string()),
        },
    ];
    let operators = [
        ConditionOperator::Exists,
        ConditionOperator::IsNull,
        ConditionOperator::Eq,
        ConditionOperator::Ne,
        ConditionOperator::Lt,
        ConditionOperator::Lte,
        ConditionOperator::Gt,
        ConditionOperator::Gte,
        ConditionOperator::Contains,
    ];
    for (predicate, operator) in predicates.into_iter().zip(operators) {
        assert_eq!(predicate.operator(), operator);
        let wire = serde_json::to_value(&predicate).unwrap();
        assert_eq!(
            serde_json::from_value::<ConditionPredicate>(wire).unwrap(),
            predicate
        );
    }

    for (strategy, wire) in [
        (MergeStrategy::SingleActive, "single_active"),
        (MergeStrategy::CollectArray, "collect_array"),
        (MergeStrategy::ConcatArrays, "concat_arrays"),
        (MergeStrategy::OverlayObjects, "overlay_objects"),
    ] {
        assert_eq!(serde_json::to_value(strategy).unwrap(), wire);
    }
    assert_eq!(serde_json::to_value(JsOutputKind::Json).unwrap(), "json");
    assert_eq!(serde_json::to_value(JsOutputKind::Raw).unwrap(), "raw");

    let kinds = [
        PortValueKind::IntentInput,
        PortValueKind::Raw,
        PortValueKind::HttpResponse,
        PortValueKind::Json,
        PortValueKind::Delta,
        PortValueKind::LoopBinding,
    ];
    let union = PortValueType::union(kinds);
    assert_eq!(
        serde_json::from_value::<PortValueType>(serde_json::to_value(&union).unwrap()).unwrap(),
        union
    );
}

#[test]
fn author_loop_limit_preserves_invalid_drafts_while_plan_limit_is_validated() {
    assert!(LoopIterationLimit::new(0).is_err());
    assert!(LoopIterationLimit::new(MAX_LOOP_ITERATIONS).is_ok());
    assert!(LoopIterationLimit::new(MAX_LOOP_ITERATIONS + 1).is_err());

    for value in [0_u16, 257_u16] {
        let wire = json!({
            "collection": { "mode": "typed", "pointer": "/items" },
            "item_binding": "item",
            "index_binding": "index",
            "max_iterations": value
        });
        let author = serde_json::from_value::<ForEachConfig>(wire.clone()).unwrap();
        assert_eq!(author.max_iterations, value);
        assert!(serde_json::from_value::<PlanForEachConfig>(wire).is_err());
    }
}

#[test]
fn plan_uses_closed_ports_typed_edges_configs_and_control_regions() {
    let definition = control_definition();
    let plan = control_plan(&definition);
    assert!(plan.has_control_flow());
    assert_eq!(plan.control_regions().len(), 1);
    assert_eq!(plan.plan_hash(), execution_plan_hash(&plan).unwrap());

    let wire = serde_json::to_value(&plan).unwrap();
    assert_eq!(wire["schema_version"], RULE_CONTRACT_SCHEMA_VERSION);
    assert!(wire["nodes"][0].get("kind").is_none());
    assert!(wire["nodes"][0].get("config").is_some());
    assert!(wire["nodes"][0]["inputs"][0].get("type_tag").is_none());
    assert!(wire["nodes"][0]["inputs"][0].get("value_type").is_some());
    assert!(wire["edges"][0]["from"].get("node_id").is_some());
    assert_eq!(wire["control_regions"][0]["kind"], "loop");

    let reread = read_execution_plan(&serde_json::to_vec(&wire).unwrap()).unwrap();
    assert_eq!(reread, plan);
}

#[test]
fn plan_reader_preserves_semantic_object_keys_named_contract() {
    let mut definition = control_definition();
    let condition = definition
        .flow_mut()
        .nodes
        .iter_mut()
        .find(|node| node.id == id(6))
        .expect("fixture Condition exists");
    let FlowNodeConfig::Condition(config) = &mut condition.config else {
        panic!("fixture node is a Condition");
    };
    config.expression = ControlExpression::Typed {
        predicate: ConditionPredicate::Eq {
            pointer: "/metadata".to_string(),
            value: TypedLiteral::Object(BTreeMap::from([(
                "contract".to_string(),
                TypedLiteral::String("semantic-value".to_string()),
            )])),
        },
        true_branch: "true_branch".to_string(),
        false_branch: "false_branch".to_string(),
    };

    let plan = control_plan(&definition);
    let reread = read_execution_plan(&serde_json::to_vec(&plan).unwrap()).unwrap();
    assert_eq!(reread, plan);
}

#[test]
fn definition_and_plan_hashes_ignore_declaration_order_and_layout_span() {
    let definition = control_definition();
    let expected_definition_hash = definition_hash(&definition).unwrap();
    let mut reordered = definition.clone();
    reordered.flow_mut().nodes.reverse();
    reordered.flow_mut().edges.reverse();
    reordered.source_id_rules_mut().reverse();
    for node in &mut reordered.flow_mut().nodes {
        node.span = Some(SourceSpan {
            start: 999,
            end: 1000,
            path: Some("/editor-only".to_string()),
        });
        match &mut node.config {
            FlowNodeConfig::Merge(config) => config.inputs.reverse(),
            FlowNodeConfig::Condition(config) => config.branches.reverse(),
            FlowNodeConfig::Mapper(config) => config.identity_fields.reverse(),
            FlowNodeConfig::Http(_)
            | FlowNodeConfig::Js(_)
            | FlowNodeConfig::Extract(_)
            | FlowNodeConfig::Loop(_)
            | FlowNodeConfig::Unavailable(_) => {}
        }
    }
    assert_eq!(
        definition_hash(&reordered).unwrap(),
        expected_definition_hash
    );

    let plan = control_plan(&definition);
    let mut nodes = plan.nodes().to_vec();
    nodes.reverse();
    for node in &mut nodes {
        node.inputs.reverse();
        node.outputs.reverse();
        if let PlanNodeConfig::Merge(config) = &mut node.config {
            config.inputs.reverse();
        }
    }
    let mut edges = plan.edges().to_vec();
    edges.reverse();
    let mut regions = plan.control_regions().to_vec();
    let ControlRegion::Loop(region) = &mut regions[0];
    region.body_nodes.reverse();
    let reordered_plan = ExecutionPlan::new(
        plan.compiler_version(),
        plan.definition_hash(),
        ExecutionPlanParts {
            nodes,
            edges,
            intent_entries: plan.intent_entries().clone(),
            effects: plan.effects().iter().cloned().rev().collect(),
            capability_requirements: plan
                .capability_requirements()
                .iter()
                .cloned()
                .rev()
                .collect(),
            control_regions: regions,
        },
    )
    .unwrap();
    assert_eq!(reordered_plan.plan_hash(), plan.plan_hash());
}

#[test]
fn every_control_semantic_change_changes_definition_and_plan_hashes() {
    let base = control_definition();
    let base_hash = definition_hash(&base).unwrap();

    let mut merge_changed = base.clone();
    for node in &mut merge_changed.flow_mut().nodes {
        if let FlowNodeConfig::Merge(config) = &mut node.config {
            config.strategy = MergeStrategy::OverlayObjects;
        }
    }
    assert_ne!(definition_hash(&merge_changed).unwrap(), base_hash);

    let mut input_id_changed = base.clone();
    for node in &mut input_id_changed.flow_mut().nodes {
        if let FlowNodeConfig::Merge(config) = &mut node.config {
            config.inputs[0].input_id.push_str("-changed");
        }
    }
    assert_ne!(definition_hash(&input_id_changed).unwrap(), base_hash);

    let mut order_changed = base.clone();
    for node in &mut order_changed.flow_mut().nodes {
        if let FlowNodeConfig::Merge(config) = &mut node.config {
            for input in &mut config.inputs {
                if input.handle == "alpha" {
                    input.order = 1;
                } else if input.handle == "zeta" {
                    input.order = 0;
                }
            }
        }
    }
    assert_ne!(definition_hash(&order_changed).unwrap(), base_hash);

    let mut activation_changed = base.clone();
    for node in &mut activation_changed.flow_mut().nodes {
        if let FlowNodeConfig::Merge(config) = &mut node.config {
            for input in &mut config.inputs {
                if input.handle == "zeta" {
                    input.activation = MergeInputActivation::Required;
                }
            }
        }
    }
    assert_ne!(definition_hash(&activation_changed).unwrap(), base_hash);

    let mut condition_changed = base.clone();
    for node in &mut condition_changed.flow_mut().nodes {
        if let FlowNodeConfig::Condition(config) = &mut node.config {
            config.expression = ControlExpression::Js {
                code: "return 'true_branch'".to_string(),
            };
        }
    }
    assert_ne!(definition_hash(&condition_changed).unwrap(), base_hash);

    let mut loop_changed = base.clone();
    for node in &mut loop_changed.flow_mut().nodes {
        if let FlowNodeConfig::Loop(config) = &mut node.config {
            config.max_iterations = 9;
        }
    }
    assert_ne!(definition_hash(&loop_changed).unwrap(), base_hash);

    let base_plan = control_plan(&base);
    let changed_plan = control_plan(&merge_changed);
    assert_ne!(base_plan.plan_hash(), changed_plan.plan_hash());
    assert_ne!(
        base_plan.plan_hash(),
        control_plan(&input_id_changed).plan_hash()
    );
}

#[test]
fn unknown_fields_are_rejected_at_root_config_port_and_control_region() {
    let definition = control_definition();
    let mut root = serde_json::to_value(&definition).unwrap();
    root["unknown"] = json!(true);
    assert!(matches!(
        read_rule_definition(&serde_json::to_vec(&root).unwrap()),
        Err(SchemaReadError::InvalidData { .. })
    ));

    let mut config = serde_json::to_value(&definition).unwrap();
    config["flow"]["nodes"][0]["config"]["value"]["unknown"] = json!(true);
    assert!(read_rule_definition(&serde_json::to_vec(&config).unwrap()).is_err());

    let plan = control_plan(&definition);
    let mut port = serde_json::to_value(&plan).unwrap();
    port["nodes"][0]["inputs"][0]["unknown"] = json!(true);
    assert!(read_execution_plan(&serde_json::to_vec(&port).unwrap()).is_err());

    let mut region = serde_json::to_value(&plan).unwrap();
    region["control_regions"][0]["value"]["unknown"] = json!(true);
    assert!(read_execution_plan(&serde_json::to_vec(&region).unwrap()).is_err());
}

#[test]
fn canonical_number_and_typed_literal_helpers_use_numeric_json_equality() {
    let one = serde_json::from_str::<serde_json::Number>("1").unwrap();
    let one_decimal = serde_json::from_str::<serde_json::Number>("1.0").unwrap();
    let one_exponent = serde_json::from_str::<serde_json::Number>("1e0").unwrap();
    assert!(canonical_number_eq(&one, &one_decimal));
    assert!(canonical_number_eq(&one_decimal, &one_exponent));

    let integer_literal = TypedLiteral::Number(CanonicalNumber::new(one.clone()));
    let decimal_literal = TypedLiteral::Number(CanonicalNumber::new(one_decimal.clone()));
    assert_eq!(
        canonical_json(&integer_literal).unwrap(),
        canonical_json(&decimal_literal).unwrap()
    );

    let mut integer_definition = control_definition();
    let mut decimal_definition = control_definition();
    for (definition, value) in [
        (&mut integer_definition, integer_literal),
        (&mut decimal_definition, decimal_literal),
    ] {
        for node in &mut definition.flow_mut().nodes {
            if let FlowNodeConfig::Condition(config) = &mut node.config {
                config.expression = ControlExpression::Typed {
                    predicate: ConditionPredicate::Eq {
                        pointer: "/number".to_string(),
                        value,
                    },
                    true_branch: "true_branch".to_string(),
                    false_branch: "false_branch".to_string(),
                };
                break;
            }
        }
    }
    assert_eq!(
        definition_hash(&integer_definition).unwrap(),
        definition_hash(&decimal_definition).unwrap()
    );
    assert_eq!(
        control_plan(&integer_definition).plan_hash(),
        control_plan(&decimal_definition).plan_hash()
    );

    let negative = serde_json::from_str::<serde_json::Number>("-2.5").unwrap();
    let positive = serde_json::from_str::<serde_json::Number>("1000000000000000000").unwrap();
    assert_eq!(canonical_number_cmp(&negative, &positive), Ordering::Less);

    let literal = TypedLiteral::Object(BTreeMap::from([(
        "nested".to_string(),
        TypedLiteral::Array(vec![TypedLiteral::Number(CanonicalNumber::new(one))]),
    )]));
    let runtime = json!({ "nested": [1.0] });
    assert!(typed_literal_matches_json(&literal, &runtime));
    assert!(literal.equals_json_value(&runtime));
    assert!(canonical_json_deep_eq(&json!({"nested": [1]}), &runtime));
    assert_eq!(
        serde_json::from_value::<TypedLiteral>(serde_json::to_value(&literal).unwrap()).unwrap(),
        literal
    );
}
