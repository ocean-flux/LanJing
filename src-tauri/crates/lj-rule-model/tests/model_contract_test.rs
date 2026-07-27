//! 规则模型基础合同：版本 reader/writer、hash、合同隔离与既有 Event/Policy DTO。

use std::collections::{BTreeMap, HashMap};

use lj_capability::{IntentExport, StandardIntent};
use lj_rule_model::{
    CapabilityManifest, ContractSchemaVersion, EventEnvelope, EventType, ExecutionPlan, FlowGraph,
    RuleDefinition, SchemaContract, SchemaReadError, SourceIdentity, canonical_json,
    definition_hash, execution_plan_hash, read_execution_plan, read_rule_definition,
    read_rule_package,
};
use serde::Serialize;
use serde_json::{Value, json};
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
        "test-compiler@2",
        definition_hash,
        vec![],
        vec![],
        intent_entries,
        vec![],
        vec!["network".to_string()],
        vec![],
    )
    .expect("sample Plan must seal")
}

fn blake3_canonical(value: &impl Serialize) -> String {
    let canonical = canonical_json(value).expect("hash material must canonicalize");
    blake3::hash(canonical.as_bytes()).to_hex().to_string()
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
fn v2_definition_and_plan_writers_use_owned_constants_and_sealed_hash() {
    let definition = sample_definition();
    assert_eq!(definition.schema_version(), ContractSchemaVersion::V2);
    let definition_wire = serde_json::to_value(&definition).expect("serialize definition");
    assert_eq!(
        definition_wire["schema_version"],
        lj_rule_model::RULE_DEFINITION_SCHEMA_VERSION
    );

    let definition_hash = definition_hash(&definition).expect("definition hash");
    let plan = sample_plan(&definition_hash);
    assert_eq!(plan.schema_version(), ContractSchemaVersion::V2);
    assert_eq!(plan.plan_hash(), execution_plan_hash(&plan).unwrap());
    let plan_wire = serde_json::to_value(&plan).expect("serialize Plan");
    assert_eq!(
        plan_wire["schema_version"],
        lj_rule_model::EXECUTION_PLAN_SCHEMA_VERSION
    );
    assert_eq!(plan_wire["plan_hash"], plan.plan_hash());
}

#[test]
fn v1_definition_package_and_plan_are_read_only_with_exact_legacy_hashes() {
    let legacy_definition = json!({
        "contract": "rule_definition",
        "schema_version": ContractSchemaVersion::V1.as_u32(),
        "source_identity": { "id": "source:legacy" },
        "base_url": "https://legacy.example",
        "intent_exports": {},
        "flow": { "nodes": [], "edges": [] },
        "capability_manifest": {
            "required": {
                "network": false,
                "system": { "fs": false, "env": false, "process": false }
            }
        },
        "source_id_rules": ["legacy_id"]
    });
    let expected_definition_hash = blake3_canonical(&legacy_definition);
    let definition_bytes = serde_json::to_vec(&legacy_definition).unwrap();
    let definition = read_rule_definition(&definition_bytes).expect("v1 Definition must read");
    assert_eq!(definition.schema_version(), ContractSchemaVersion::V1);
    assert_eq!(
        definition_hash(&definition).unwrap(),
        expected_definition_hash
    );
    assert!(
        serde_json::to_value(&definition).is_err(),
        "v1 Definition reader object must not become an implicit migration writer"
    );

    let legacy_package = json!({
        "contract": "rule_package",
        "schema_version": ContractSchemaVersion::V1.as_u32(),
        "source_identity": { "id": "source:legacy" },
        "version": expected_definition_hash,
        "definition": legacy_definition
    });
    let package = read_rule_package(&serde_json::to_vec(&legacy_package).unwrap())
        .expect("v1 package must read");
    assert_eq!(package.schema_version(), ContractSchemaVersion::V1);
    assert_eq!(
        package.definition().schema_version(),
        ContractSchemaVersion::V1
    );
    assert!(
        serde_json::to_value(&package).is_err(),
        "v1 Package reader object must not become an implicit migration writer"
    );

    let mut legacy_plan = json!({
        "contract": "execution_plan",
        "schema_version": ContractSchemaVersion::V1.as_u32(),
        "compiler_version": "legacy-compiler@1",
        "definition_hash": package.version(),
        "plan_hash": "",
        "nodes": [],
        "edges": [],
        "intent_entries": {},
        "effects": [],
        "capability_requirements": []
    });
    let expected_plan_hash = blake3_canonical(&legacy_plan);
    legacy_plan["plan_hash"] = Value::String(expected_plan_hash.clone());
    let plan = read_execution_plan(&serde_json::to_vec(&legacy_plan).unwrap())
        .expect("v1 installed Plan must read");
    assert_eq!(plan.schema_version(), ContractSchemaVersion::V1);
    assert_eq!(plan.plan_hash(), expected_plan_hash);
    assert_eq!(execution_plan_hash(&plan).unwrap(), expected_plan_hash);

    assert!(
        serde_json::to_value(&plan).is_err(),
        "v1 Plan reader object must not become an implicit migration writer"
    );
}

#[test]
fn unknown_contract_versions_are_typed_incompatible() {
    for (contract, reader) in [(
        SchemaContract::RuleDefinition,
        read_rule_definition as fn(&[u8]) -> Result<RuleDefinition, SchemaReadError>,
    )] {
        let bytes = serde_json::to_vec(&json!({
            "contract": contract.wire_name(),
            "schema_version": 99
        }))
        .unwrap();
        assert!(matches!(
            reader(&bytes),
            Err(SchemaReadError::IncompatibleVersion {
                contract: actual,
                version: 99
            }) if actual == contract
        ));
    }

    let package = serde_json::to_vec(&json!({
        "contract": "rule_package",
        "schema_version": 99
    }))
    .unwrap();
    assert!(matches!(
        read_rule_package(&package),
        Err(SchemaReadError::IncompatibleVersion {
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
        Err(SchemaReadError::IncompatibleVersion {
            contract: SchemaContract::ExecutionPlan,
            version: 99
        })
    ));
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
