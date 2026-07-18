//! `NativeImporter` 集成测试。

use std::collections::HashMap;

use lj_capability::{IntentExport, StandardIntent};
use lj_importer::native::NativeImporter;
use lj_rule_model::{ExpectedDataType, ExtractSpec};
use lj_rule_model::{HttpMethod, HttpSpec};
use lj_runtime::{
    Edge, Graph, MapperOutputKind, MapperSpec, Node, NodeId, NodeKind, NodeSpec, SourceId,
};
use uuid::Uuid;

#[test]
fn native_import_round_trip() {
    // 创建符合标准意图契约的简单 Graph(Search: Http → Extract)
    let source_id = SourceId(Uuid::new_v4());
    let http_id = NodeId(Uuid::new_v4());
    let extract_id = NodeId(Uuid::new_v4());
    let mapper_id = NodeId(Uuid::new_v4());

    let http_spec = HttpSpec {
        method: HttpMethod::Get,
        url: "https://example.com/search".to_string(),
        headers: HashMap::new(),
        body: None,
        charset: None,
        expected_type: ExpectedDataType::Html,
    };

    let extract_spec = ExtractSpec {
        rules: Vec::new(),
        field_rules: HashMap::new(),
        expected_type: ExpectedDataType::Html,
        output_target: lj_rule_model::OutputTarget::default(),
    };

    let nodes = vec![
        Node {
            node_id: http_id.clone(),
            import_hash: "a".repeat(64),
            spec: NodeSpec {
                kind: NodeKind::Http,
                http: Some(http_spec),
                js: None,
                extract: None,
                mapper: None,
            },
        },
        Node {
            node_id: extract_id.clone(),
            import_hash: "b".repeat(64),
            spec: NodeSpec {
                kind: NodeKind::Extract,
                http: None,
                js: None,
                extract: Some(extract_spec),
                mapper: None,
            },
        },
        Node {
            node_id: mapper_id.clone(),
            import_hash: "c".repeat(64),
            spec: NodeSpec {
                kind: NodeKind::Mapper,
                http: None,
                js: None,
                extract: None,
                mapper: Some(MapperSpec {
                    output: MapperOutputKind::Items,
                    identity_fields: vec!["url".to_string(), "title".to_string()],
                }),
            },
        },
    ];

    let edges = vec![
        Edge {
            from: http_id.clone(),
            to: extract_id.clone(),
            condition_branch: None,
        },
        Edge {
            from: extract_id.clone(),
            to: mapper_id.clone(),
            condition_branch: None,
        },
    ];

    let mut intent_exports = HashMap::new();
    intent_exports.insert(
        StandardIntent::Search,
        IntentExport::new(http_id.0, mapper_id.0),
    );

    let original_graph = Graph {
        nodes,
        edges,
        subroutines: HashMap::new(),
        source_id,
        base_url: String::new(),
        intent_exports,
    };
    let expected_export = original_graph.intent_exports[&StandardIntent::Search].clone();

    // 序列化 → JSON → 反序列化
    let json = serde_json::to_string(&original_graph).expect("序列化 Graph 失败");
    let graph: Graph = serde_json::from_str(&json).expect("反序列化 Graph 失败");

    // NativeImporter 导入
    let importer = NativeImporter;
    let preview = importer.import(graph).expect("native import 应成功");

    assert_eq!(preview.node_count, 3, "round-trip 后节点数应为 3");
    assert_eq!(preview.edge_count, 2, "round-trip 后边数应为 2");
    assert_eq!(
        preview.http_target_urls,
        vec!["https://example.com/search".to_string()],
        "HTTP 目标 URL 应一致"
    );
    assert_eq!(
        preview.graph.intent_exports[&StandardIntent::Search],
        expected_export,
        "native import 应保留标准意图导出"
    );
}

#[test]
fn legacy_endpoint_graph_json_is_rejected() {
    let endpoint_key = ["endpoint", "kind"].join("_");
    let legacy_stage_key = ["pre", "steps"].join("_");
    let legacy_endpoint_graph = format!(
        "{{\n  \"{endpoint_key}\": \"Search\",\n  \"{legacy_stage_key}\": [\n    {{ \"kind\": \"Http\", \"url\": \"https://example.com/search\" }}\n  ],\n  \"main\": {{ \"kind\": \"Extract\" }}\n}}"
    );

    assert!(
        serde_json::from_str::<Graph>(&legacy_endpoint_graph).is_err(),
        "旧 endpoint 图 JSON 不应被静默反序列化为新 Graph"
    );
}
