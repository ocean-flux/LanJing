//! `NativeImporter` 集成测试。

use std::collections::HashMap;

use lj_core::endpoint::{EndpointKind, HttpMethod, HttpSpec};
use lj_core::extract_rule::{ExpectedDataType, ExtractSpec};
use lj_core::node::{Edge, Graph, Node, NodeId, NodeKind, NodeSpec, SourceId};
use lj_core::traits::Importer;
use lj_importer::native::NativeImporter;
use uuid::Uuid;

#[test]
fn native_import_round_trip() {
    // 创建符合 GraphSchema 的简单 Graph(Search 端点: Http → Extract)
    let source_id = SourceId(Uuid::new_v4());
    let http_id = NodeId(Uuid::new_v4());
    let extract_id = NodeId(Uuid::new_v4());

    let http_spec = HttpSpec {
        endpoint_kind: EndpointKind::Search,
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
        endpoint_kind: Some(EndpointKind::Search),
        expected_type: ExpectedDataType::Html,
        play_url_parser: None,
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
            },
        },
    ];

    let edges = vec![Edge {
        from: http_id,
        to: extract_id,
        condition_branch: None,
    }];

    let original_graph = Graph {
        nodes,
        edges,
        subroutines: HashMap::new(),
        source_id,
        base_url: String::new(),
    };

    // 序列化 → JSON → 反序列化
    let json = serde_json::to_string(&original_graph).expect("序列化 Graph 失败");
    let graph: Graph = serde_json::from_str(&json).expect("反序列化 Graph 失败");

    // NativeImporter 导入
    let importer = NativeImporter;
    let preview = importer.import(graph).expect("native import 应成功");

    assert_eq!(preview.node_count, 2, "round-trip 后节点数应为 2");
    assert_eq!(preview.edge_count, 1, "round-trip 后边数应为 1");
    assert_eq!(
        preview.http_target_urls,
        vec!["https://example.com/search".to_string()],
        "HTTP 目标 URL 应一致"
    );
}
