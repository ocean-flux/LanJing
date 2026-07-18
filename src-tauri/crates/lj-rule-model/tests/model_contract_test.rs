//! 规则模型合同测试：Definition/Plan 隔离、hash 稳定、EventEnvelope 字段。

use std::collections::BTreeMap;

use lj_capability::{IntentExport, StandardIntent};
use lj_rule_model::{
    CapabilityManifest, EventEnvelope, EventType, ExecutionPlan, FlowGraph, IntentEntry,
    RuleDefinition, SourceIdentity, definition_hash,
};
use uuid::Uuid;

fn sample_definition() -> RuleDefinition {
    let entry = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let mapper = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
    let mut intent_exports = BTreeMap::new();
    intent_exports.insert(StandardIntent::Search, IntentExport::new(entry, mapper));
    RuleDefinition {
        schema_version: 1,
        source_identity: SourceIdentity {
            id: "source:demo".to_string(),
        },
        base_url: "https://example.com".to_string(),
        intent_exports,
        flow: FlowGraph {
            nodes: vec![],
            edges: vec![],
        },
        capability_manifest: CapabilityManifest::default(),
        source_id_rules: vec!["source_item_id".to_string()],
    }
}

fn sample_plan(definition_hash: &str) -> ExecutionPlan {
    let entry = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let mapper = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
    let mut intent_entries = BTreeMap::new();
    intent_entries.insert(
        StandardIntent::Search,
        IntentEntry {
            intent: StandardIntent::Search,
            entry_node: entry,
            mapper_output: mapper,
        },
    );
    ExecutionPlan {
        schema_version: 1,
        compiler_version: "test-compiler@0".to_string(),
        definition_hash: definition_hash.to_string(),
        plan_hash: "plan-hash-placeholder".to_string(),
        nodes: vec![],
        edges: vec![],
        intent_entries,
        effects: vec![],
        capability_requirements: vec!["network".to_string()],
    }
}

#[test]
fn definition_and_plan_not_cross_deserializable() {
    let def = sample_definition();
    let def_json = serde_json::to_string(&def).expect("serialize definition");
    let plan_err = serde_json::from_str::<ExecutionPlan>(&def_json);
    assert!(
        plan_err.is_err(),
        "Definition JSON 不得作为 Plan 反序列化成功"
    );

    let hash = definition_hash(&def).expect("hash");
    let plan = sample_plan(&hash);
    let plan_json = serde_json::to_string(&plan).expect("serialize plan");
    let def_err = serde_json::from_str::<RuleDefinition>(&plan_json);
    assert!(
        def_err.is_err(),
        "Plan JSON 不得作为 Definition 反序列化成功"
    );
}

#[test]
fn definition_hash_is_stable() {
    let a = sample_definition();
    let b = sample_definition();
    let ha = definition_hash(&a).expect("hash a");
    let hb = definition_hash(&b).expect("hash b");
    assert_eq!(ha, hb);
    assert_eq!(ha.len(), 64);
    // 字段顺序扰动：重建相同内容
    let mut c = sample_definition();
    c.source_id_rules = vec!["source_item_id".to_string()];
    assert_eq!(ha, definition_hash(&c).expect("hash c"));
}

#[test]
fn event_envelope_has_required_fields() {
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
        payload: serde_json::json!({"kind": "started"}),
        artifact_refs: vec![],
        secret_refs: vec![],
    };
    let json = serde_json::to_value(&envelope).expect("serialize envelope");
    assert_eq!(json["global_seq"], 42);
    assert_eq!(json["stream_id"], "execution/abc");
    assert_eq!(json["stream_version"], 7);
    assert_eq!(json["schema_version"], 1);
    assert!(json.get("correlation_id").is_some());
    assert_eq!(json["trace_id"], "trace-xyz");
    let back: EventEnvelope = serde_json::from_value(json).expect("roundtrip");
    assert_eq!(back, envelope);
}

#[test]
fn policy_and_capability_manifest_roundtrip() {
    use lj_rule_model::{PolicyCapabilities, SystemCapabilities};
    let caps = PolicyCapabilities {
        network: true,
        system: SystemCapabilities {
            fs: false,
            env: false,
            process: false,
        },
    };
    let json = serde_json::to_string(&caps).unwrap();
    let back: PolicyCapabilities = serde_json::from_str(&json).unwrap();
    assert_eq!(caps, back);
}
