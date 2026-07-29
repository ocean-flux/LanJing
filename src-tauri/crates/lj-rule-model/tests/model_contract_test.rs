//! 规则模型基础合同：唯一 current reader/writer、hash、合同隔离与既有 Event/Policy DTO。

use std::collections::{BTreeMap, HashMap};

use lj_capability::{IntentExport, StandardIntent};
use lj_rule_model::{
    CapabilityManifest, EventEnvelope, EventType, ExecutionPlan, ExecutionPlanParts, FlowGraph,
    RULE_CONTRACT_SCHEMA_VERSION, RuleDefinition, SchemaContract, SchemaReadError, SourceIdentity,
    canonical_json, definition_hash, execution_plan_hash, read_execution_plan,
    read_rule_definition, read_rule_package,
};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Serialize)]
struct NestedMaps {
    values: HashMap<String, HashMap<String, String>>,
}

fn sample_definition() -> RuleDefinition {
    let entry = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let mapper = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
    let mut intent_exports = BTreeMap::new();
    intent_exports.insert(StandardIntent::Search, IntentExport::new(entry, mapper));
    RuleDefinition::new(
        SourceIdentity {
            id: "source:demo".to_string(),
        },
        "https://example.com",
        intent_exports,
        FlowGraph {
            nodes: vec![],
            edges: vec![],
        },
        CapabilityManifest::default(),
        vec!["source_item_id".to_string()],
    )
}

fn sample_plan(definition_hash: &str) -> ExecutionPlan {
    let entry = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let mapper = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
    let mut intent_entries = BTreeMap::new();
    intent_entries.insert(
        StandardIntent::Search,
        lj_rule_model::IntentEntry {
            intent: StandardIntent::Search,
            entry_node: entry,
            mapper_output: mapper,
        },
    );
    ExecutionPlan::new(
        "test-compiler@1",
        definition_hash,
        ExecutionPlanParts {
            nodes: vec![],
            edges: vec![],
            intent_entries,
            effects: vec![],
            capability_requirements: vec!["network".to_string()],
            control_regions: vec![],
        },
    )
    .expect("sample Plan must seal")
}

#[test]
fn definition_and_plan_not_cross_deserializable() {
    let definition = sample_definition();
    let definition_json = serde_json::to_string(&definition).expect("serialize definition");
    assert!(
        serde_json::from_str::<ExecutionPlan>(&definition_json).is_err(),
        "Definition JSON 不得作为 Plan 反序列化成功"
    );

    let plan = sample_plan(&definition_hash(&definition).expect("hash"));
    let plan_json = serde_json::to_string(&plan).expect("serialize plan");
    assert!(
        serde_json::from_str::<RuleDefinition>(&plan_json).is_err(),
        "Plan JSON 不得作为 Definition 反序列化成功"
    );
}

#[test]
fn current_definition_and_plan_writers_use_shared_schema_constant_and_sealed_hash() {
    let definition = sample_definition();
    let definition_wire = serde_json::to_value(&definition).expect("serialize definition");
    assert_eq!(
        definition_wire["schema_version"],
        RULE_CONTRACT_SCHEMA_VERSION
    );

    let definition_hash = definition_hash(&definition).expect("definition hash");
    let plan = sample_plan(&definition_hash);
    assert_eq!(plan.plan_hash(), execution_plan_hash(&plan).unwrap());
    let plan_wire = serde_json::to_value(&plan).expect("serialize Plan");
    assert_eq!(plan_wire["schema_version"], RULE_CONTRACT_SCHEMA_VERSION);
    assert_eq!(plan_wire["plan_hash"], plan.plan_hash());
}

#[test]
fn non_current_shapes_are_current_invalid_data_without_shape_classification() {
    let legacy_definition = json!({
        "contract": "rule_definition",
        "schema_version": 1,
        "source_identity": { "id": "source:legacy" },
        "base_url": "https://legacy.example",
        "intent_exports": {},
        "flow": {
            "nodes": [{
                "id": "11111111-1111-1111-1111-111111111111",
                "kind": "Js",
                "js_code": "return input"
            }],
            "edges": []
        },
        "capability_manifest": {
            "required": {
                "network": false,
                "system": { "fs": false, "env": false, "process": false }
            }
        },
        "source_id_rules": ["legacy_id"]
    });
    let error = read_rule_definition(&serde_json::to_vec(&legacy_definition).unwrap()).unwrap_err();
    assert_eq!(error.code(), "RULE_CONTRACT_INVALID_DATA");
    assert!(matches!(
        error,
        SchemaReadError::InvalidData {
            contract: SchemaContract::RuleDefinition,
            ..
        }
    ));

    let legacy_package = json!({
        "contract": "rule_package",
        "schema_version": 1,
        "source_identity": { "id": "source:legacy" },
        "version": "deadbeef",
        "definition": legacy_definition
    });
    assert!(matches!(
        read_rule_package(&serde_json::to_vec(&legacy_package).unwrap()),
        Err(SchemaReadError::InvalidData {
            contract: SchemaContract::RuleDefinition,
            ..
        })
    ));

    let legacy_plan = json!({
        "contract": "execution_plan",
        "schema_version": 1,
        "compiler_version": "legacy-compiler@1",
        "definition_hash": "deadbeef",
        "plan_hash": "cafebabe",
        "nodes": [],
        "edges": [],
        "intent_entries": {},
        "effects": [],
        "capability_requirements": []
    });
    assert!(matches!(
        read_execution_plan(&serde_json::to_vec(&legacy_plan).unwrap()),
        Err(SchemaReadError::InvalidData {
            contract: SchemaContract::ExecutionPlan,
            ..
        })
    ));
}

#[test]
fn unknown_contract_versions_are_schema_unsupported() {
    let contract = SchemaContract::RuleDefinition;
    let bytes = serde_json::to_vec(&json!({
        "contract": contract.wire_name(),
        "schema_version": 99
    }))
    .unwrap();
    let error = read_rule_definition(&bytes).unwrap_err();
    assert_eq!(error.code(), "RULE_CONTRACT_SCHEMA_UNSUPPORTED");
    assert!(matches!(
        error,
        SchemaReadError::SchemaUnsupported {
            contract: actual,
            version: 99
        } if actual == contract
    ));

    let package = serde_json::to_vec(&json!({
        "contract": "rule_package",
        "schema_version": 99
    }))
    .unwrap();
    assert!(matches!(
        read_rule_package(&package),
        Err(SchemaReadError::SchemaUnsupported {
            contract: SchemaContract::RulePackage,
            version: 99
        })
    ));

    let plan = serde_json::to_vec(&json!({
        "contract": "execution_plan",
        "schema_version": 99
    }))
    .unwrap();
    assert!(matches!(
        read_execution_plan(&plan),
        Err(SchemaReadError::SchemaUnsupported {
            contract: SchemaContract::ExecutionPlan,
            version: 99
        })
    ));
}

#[test]
fn package_reader_preserves_nested_definition_error_categories() {
    let definition = sample_definition();
    let package = lj_rule_model::RulePackage::new(
        definition.source_identity().clone(),
        definition_hash(&definition).unwrap(),
        definition,
    );

    let mut unknown_schema = serde_json::to_value(&package).unwrap();
    unknown_schema["definition"]["schema_version"] = json!(99);
    unknown_schema["definition"]["flow"]["nodes"] = json!([{
        "id": "11111111-1111-1111-1111-111111111111",
        "kind": "Js",
        "js_code": "return input"
    }]);
    let error = read_rule_package(&serde_json::to_vec(&unknown_schema).unwrap()).unwrap_err();
    assert_eq!(error.code(), "RULE_CONTRACT_SCHEMA_UNSUPPORTED");
    assert!(matches!(
        error,
        SchemaReadError::SchemaUnsupported {
            contract: SchemaContract::RuleDefinition,
            version: 99
        }
    ));

    let mut non_object_definition = serde_json::to_value(&package).unwrap();
    non_object_definition["definition"] = json!("not-an-object");
    assert!(matches!(
        read_rule_package(&serde_json::to_vec(&non_object_definition).unwrap()),
        Err(SchemaReadError::InvalidData {
            contract: SchemaContract::RuleDefinition,
            ..
        })
    ));

    let mut invalid_definition = serde_json::to_value(&package).unwrap();
    invalid_definition["definition"]["unknown"] = json!(true);
    assert!(matches!(
        read_rule_package(&serde_json::to_vec(&invalid_definition).unwrap()),
        Err(SchemaReadError::InvalidData {
            contract: SchemaContract::RuleDefinition,
            ..
        })
    ));
}

#[test]
fn schema_one_with_current_empty_flow_still_reads() {
    let current = sample_definition();
    let bytes = serde_json::to_vec(&current).unwrap();
    let reread = read_rule_definition(&bytes).expect("current empty flow must read");
    assert_eq!(reread, current);

    let package = lj_rule_model::RulePackage::new(
        current.source_identity().clone(),
        definition_hash(&current).unwrap(),
        current.clone(),
    );
    let package_wire = serde_json::to_value(&package).unwrap();
    assert_eq!(package_wire["schema_version"], RULE_CONTRACT_SCHEMA_VERSION);
    assert_eq!(
        package_wire["definition"]["schema_version"],
        RULE_CONTRACT_SCHEMA_VERSION
    );
    let package_bytes = serde_json::to_vec(&package_wire).unwrap();
    assert_eq!(
        read_rule_package(&package_bytes).unwrap().definition(),
        &current
    );
}

#[test]
fn canonical_json_sorts_nested_hash_map_keys() {
    let mut forward_inner = HashMap::new();
    forward_inner.insert("alpha".to_string(), "one".to_string());
    forward_inner.insert("zeta".to_string(), "two".to_string());
    let mut forward_outer = HashMap::new();
    forward_outer.insert("first".to_string(), forward_inner);

    let mut reverse_inner = HashMap::new();
    reverse_inner.insert("zeta".to_string(), "two".to_string());
    reverse_inner.insert("alpha".to_string(), "one".to_string());
    let mut reverse_outer = HashMap::new();
    reverse_outer.insert("first".to_string(), reverse_inner);

    assert_eq!(
        canonical_json(&NestedMaps {
            values: forward_outer
        })
        .unwrap(),
        canonical_json(&NestedMaps {
            values: reverse_outer
        })
        .unwrap()
    );
}

#[test]
fn event_envelope_has_required_fields_without_rule_schema_cutover() {
    let envelope = EventEnvelope {
        global_seq: 42,
        stream_id: "execution/abc".to_string(),
        stream_version: 7,
        event_id: Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
        event_type: EventType::Execution,
        schema_version: 1,
        correlation_id: Some(Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap()),
        causation_id: None,
        trace_id: "trace-xyz".to_string(),
        occurred_at: "2026-07-18T00:00:00Z".to_string(),
        payload: json!({"kind": "started"}),
        artifact_refs: vec![],
        secret_refs: vec![],
    };
    let wire = serde_json::to_value(&envelope).expect("serialize envelope");
    assert_eq!(wire["schema_version"], 1);
    assert_eq!(
        serde_json::from_value::<EventEnvelope>(wire).unwrap(),
        envelope
    );
}

#[test]
fn policy_and_capability_manifest_roundtrip() {
    use lj_rule_model::{PolicyCapabilities, SystemCapabilities};
    let capabilities = PolicyCapabilities {
        network: true,
        system: SystemCapabilities {
            fs: false,
            env: false,
            process: false,
        },
    };
    let json = serde_json::to_string(&capabilities).unwrap();
    assert_eq!(
        serde_json::from_str::<PolicyCapabilities>(&json).unwrap(),
        capabilities
    );
}
