//! Compiler contract tests：Definition 规范化、诊断与 Plan hash。

use std::collections::{BTreeMap, HashMap};

use lj_capability::{IntentExport, StandardIntent};
use lj_compiler::Compiler;
use lj_rule_model::definition::MapperOutputKind;
use lj_rule_model::{
    CapabilityManifest, ControlledMapper, ExpectedDataType, ExtractSpec, FlowEdge, FlowGraph,
    FlowNode, FlowNodeKind, HttpMethod, HttpSpec, OutputTarget, PolicyCapabilities, RuleDefinition,
    SourceIdentity, SystemCapabilities,
};
use uuid::Uuid;

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn flow_node(id: Uuid, kind: FlowNodeKind) -> FlowNode {
    FlowNode {
        id,
        kind,
        http: None,
        js_code: None,
        extract: None,
        mapper: None,
        span: None,
    }
}

fn valid_definition() -> RuleDefinition {
    let http = id(1);
    let extract = id(2);
    let mapper = id(3);
    let mut http_node = flow_node(http, FlowNodeKind::Http);
    http_node.http = Some(HttpSpec {
        method: HttpMethod::Get,
        url: "https://example.com/search?q={{key}}".to_string(),
        headers: HashMap::new(),
        body: None,
        charset: None,
        expected_type: ExpectedDataType::Html,
    });
    let mut extract_node = flow_node(extract, FlowNodeKind::Extract);
    extract_node.extract = Some(ExtractSpec {
        rules: Vec::new(),
        field_rules: HashMap::new(),
        expected_type: ExpectedDataType::Html,
        output_target: OutputTarget::default(),
    });
    let mut mapper_node = flow_node(mapper, FlowNodeKind::Mapper);
    mapper_node.mapper = Some(ControlledMapper {
        output: MapperOutputKind::Items,
        identity_fields: vec!["book_url".to_string()],
    });
    let mut intent_exports = BTreeMap::new();
    intent_exports.insert(StandardIntent::Search, IntentExport::new(http, mapper));

    RuleDefinition {
        schema_version: 1,
        source_identity: SourceIdentity {
            id: "source:compiler-test".to_string(),
        },
        base_url: "https://example.com".to_string(),
        intent_exports,
        flow: FlowGraph {
            nodes: vec![http_node, extract_node, mapper_node],
            edges: vec![
                FlowEdge {
                    from: http,
                    to: extract,
                    condition_branch: None,
                },
                FlowEdge {
                    from: extract,
                    to: mapper,
                    condition_branch: None,
                },
            ],
        },
        capability_manifest: CapabilityManifest {
            required: PolicyCapabilities {
                network: true,
                system: SystemCapabilities::default(),
            },
        },
        source_id_rules: vec!["book_url".to_string()],
    }
}

fn assert_compiler_rejects(definition: &RuleDefinition, expected_code: &str) {
    let error = Compiler::default()
        .compile(definition)
        .expect_err("invalid Definition must not compile");
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == expected_code),
        "expected diagnostic {expected_code}, got {:?}",
        error.diagnostics()
    );
}

#[test]
fn same_definition_and_compiler_version_have_same_plan_hash() {
    let compiler = Compiler::with_version("test-compiler@1".to_string());
    let definition = valid_definition();
    let mut reordered = definition.clone();
    reordered.flow.nodes.reverse();
    reordered.flow.edges.reverse();

    let first = compiler
        .compile(&definition)
        .expect("valid definition compiles");
    let second = compiler
        .compile(&reordered)
        .expect("canonical ordering compiles");
    assert_eq!(first.plan_hash, second.plan_hash);

    let mut forward_headers = definition.clone();
    let forward_http = forward_headers
        .flow
        .nodes
        .iter_mut()
        .find(|node| node.id == id(1))
        .expect("HTTP node exists")
        .http
        .as_mut()
        .expect("HTTP configuration exists");
    forward_http.headers = HashMap::from([
        ("accept".to_string(), "application/json".to_string()),
        ("x-source".to_string(), "compiler-test".to_string()),
    ]);
    let mut reverse_headers = definition.clone();
    let reverse_http = reverse_headers
        .flow
        .nodes
        .iter_mut()
        .find(|node| node.id == id(1))
        .expect("HTTP node exists")
        .http
        .as_mut()
        .expect("HTTP configuration exists");
    reverse_http.headers = HashMap::from([
        ("x-source".to_string(), "compiler-test".to_string()),
        ("accept".to_string(), "application/json".to_string()),
    ]);
    let forward_plan = compiler
        .compile(&forward_headers)
        .expect("canonical headers compile");
    let reverse_plan = compiler
        .compile(&reverse_headers)
        .expect("canonical headers compile");
    assert_eq!(forward_plan.definition_hash, reverse_plan.definition_hash);
    assert_eq!(forward_plan.plan_hash, reverse_plan.plan_hash);

    let different_version = Compiler::with_version("test-compiler@2".to_string())
        .compile(&definition)
        .expect("valid definition compiles");
    assert_ne!(first.plan_hash, different_version.plan_hash);
}

#[test]
fn invalid_port_reports_a_locatable_diagnostic() {
    let mut definition = valid_definition();
    let http = id(1);
    let mapper = id(3);
    definition.flow.edges = vec![FlowEdge {
        from: http,
        to: mapper,
        condition_branch: None,
    }];

    assert_compiler_rejects(&definition, "PORT_TYPE_MISMATCH");
}

#[test]
fn unreachable_mapper_is_a_compile_failure() {
    let mut definition = valid_definition();
    definition.flow.edges.clear();

    assert_compiler_rejects(&definition, "MAPPER_UNREACHABLE");
}

#[test]
fn capability_mismatch_is_a_compile_failure() {
    let mut definition = valid_definition();
    definition.capability_manifest.required.network = false;

    assert_compiler_rejects(&definition, "CAPABILITY_MISMATCH");
}

#[test]
fn unbounded_flow_is_a_compile_failure() {
    let mut definition = valid_definition();
    definition
        .flow
        .nodes
        .push(flow_node(id(4), FlowNodeKind::Loop));

    assert_compiler_rejects(&definition, "FLOW_UNBOUNDED");
}

#[test]
#[ignore = "D12 performance calibration; run with cargo test --release on target hardware"]
fn d12_plan_compile_p95_gate() {
    const SAMPLES: usize = 1_000;
    const P95_LIMIT: std::time::Duration = std::time::Duration::from_millis(5);

    if cfg!(debug_assertions) {
        eprintln!("D12 timing gate requires a release build");
        return;
    }
    let compiler = Compiler::with_version("d12-compiler@1".to_string());
    let definition = valid_definition();
    compiler
        .compile(&definition)
        .expect("warm D12 compiler path");
    let mut elapsed = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = std::time::Instant::now();
        compiler
            .compile(&definition)
            .expect("compile valid D12 Definition");
        elapsed.push(started.elapsed());
    }
    elapsed.sort_unstable();
    let p95 = elapsed[(SAMPLES * 95).div_ceil(100) - 1];
    assert!(
        p95 <= P95_LIMIT,
        "D12 Plan compile p95 was {p95:?}, limit is {P95_LIMIT:?}"
    );
    eprintln!(
        "D12 release gate: plan_compile_p95={p95:?}, definition_nodes={}, definition_edges={}, payload=synthetic_minimal_definition",
        definition.flow.nodes.len(),
        definition.flow.edges.len()
    );
}
