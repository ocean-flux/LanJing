//! `validate_graph` 单元测试。

use std::collections::HashMap;

use lj_core::endpoint::{EndpointKind, HttpMethod, HttpSpec};
use lj_core::extract_rule::{ExpectedDataType, ExtractSpec};
use lj_core::graph_schema::{EndpointTemplate, GraphSchema};
use lj_core::node::{
    Edge, Graph, JsSpec, Node, NodeId, NodeKind, NodeSpec, SourceId, SubroutineId,
};
use lj_importer::validate::validate_graph;
use uuid::Uuid;

fn make_node_id() -> NodeId {
    NodeId(Uuid::new_v4())
}

fn make_http_node(id: NodeId, kind: EndpointKind, url: &str) -> Node {
    Node {
        node_id: id,
        import_hash: "a".repeat(64),
        spec: NodeSpec {
            kind: NodeKind::Http,
            http: Some(HttpSpec {
                endpoint_kind: kind,
                method: HttpMethod::Get,
                url: url.to_string(),
                headers: HashMap::new(),
                body: None,
                charset: None,
                expected_type: ExpectedDataType::Html,
            }),
            js: None,
            extract: None,
        },
    }
}

fn make_extract_node(id: NodeId, kind: EndpointKind) -> Node {
    Node {
        node_id: id,
        import_hash: "b".repeat(64),
        spec: NodeSpec {
            kind: NodeKind::Extract,
            http: None,
            js: None,
            extract: Some(ExtractSpec {
                rules: vec![],
                field_rules: HashMap::new(),
                endpoint_kind: Some(kind),
                expected_type: ExpectedDataType::Html,
                play_url_parser: None,
            }),
        },
    }
}

fn make_js_node(id: NodeId, kind: EndpointKind) -> Node {
    Node {
        node_id: id,
        import_hash: "c".repeat(64),
        spec: NodeSpec {
            kind: NodeKind::Js,
            http: None,
            js: Some(JsSpec {
                code: "var x = 1;".to_string(),
                endpoint_kind: Some(kind),
            }),
            extract: None,
        },
    }
}

fn default_schema() -> GraphSchema {
    GraphSchema::default_schema()
}

/// 正常 Search 子图(Http → Extract)通过验证。
#[test]
fn valid_graph_passes() {
    let http_id = make_node_id();
    let extract_id = make_node_id();
    let graph = Graph {
        nodes: vec![
            make_http_node(
                http_id.clone(),
                EndpointKind::Search,
                "https://example.com/search?q={{key}}",
            ),
            make_extract_node(extract_id.clone(), EndpointKind::Search),
        ],
        edges: vec![Edge {
            from: http_id,
            to: extract_id,
            condition_branch: None,
        }],
        subroutines: HashMap::new(),
        source_id: SourceId(Uuid::new_v4()),
        base_url: String::new(),
    };
    assert!(validate_graph(&graph, &default_schema()).is_ok());
}

/// 缺少期望的节点类型时报错。
#[test]
fn missing_node_type_errors() {
    // Search 端点缺 Extract 节点
    let http_id = make_node_id();
    let graph = Graph {
        nodes: vec![make_http_node(
            http_id,
            EndpointKind::Search,
            "https://example.com/search?q={{key}}",
        )],
        edges: vec![],
        subroutines: HashMap::new(),
        source_id: SourceId(Uuid::new_v4()),
        base_url: String::new(),
    };
    let result = validate_graph(&graph, &default_schema());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("缺少") && err.contains("Extract"));
}

/// 边目标节点不存在时报错。
#[test]
fn edge_target_not_found_errors() {
    let missing_id = make_node_id();
    let graph = Graph {
        nodes: vec![],
        edges: vec![Edge {
            from: missing_id.clone(),
            to: missing_id,
            condition_branch: None,
        }],
        subroutines: HashMap::new(),
        source_id: SourceId(Uuid::new_v4()),
        base_url: String::new(),
    };
    let result = validate_graph(&graph, &default_schema());
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("不存在"));
}

/// 边类型不匹配时报错。
#[test]
fn edge_type_mismatch_errors() {
    // Js(output=Raw) → Extract(input=HttpResponse) 不匹配
    // 用自定义 schema 使节点序列 [Js, Extract] 通过检查
    let schema = GraphSchema {
        templates: vec![EndpointTemplate {
            kind: EndpointKind::Discover,
            node_sequence: vec![NodeKind::Js, NodeKind::Extract],
            input_type: None,
            output_type: lj_core::node_data::NodeDataVariant::Media,
        }],
    };

    let js_id = make_node_id();
    let extract_id = make_node_id();
    let graph = Graph {
        nodes: vec![
            make_js_node(js_id.clone(), EndpointKind::Discover),
            make_extract_node(extract_id.clone(), EndpointKind::Discover),
        ],
        edges: vec![Edge {
            from: js_id,
            to: extract_id,
            condition_branch: None,
        }],
        subroutines: HashMap::new(),
        source_id: SourceId(Uuid::new_v4()),
        base_url: String::new(),
    };
    let result = validate_graph(&graph, &schema);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("类型不匹配"));
}

/// 子例程递归引用自身时报错。
#[test]
fn subroutine_self_reference_errors() {
    let sub_id = SubroutineId(Uuid::new_v4());

    // 内层子图也包含同名 sub_id(递归引用)
    let inner_subs = {
        let mut m = HashMap::new();
        m.insert(
            sub_id.clone(),
            Graph {
                nodes: vec![],
                edges: vec![],
                subroutines: HashMap::new(),
                source_id: SourceId(Uuid::new_v4()),
                base_url: String::new(),
            },
        );
        m
    };

    let outer_graph = Graph {
        nodes: vec![],
        edges: vec![],
        subroutines: {
            let mut m = HashMap::new();
            m.insert(
                sub_id,
                Graph {
                    nodes: vec![],
                    edges: vec![],
                    subroutines: inner_subs,
                    source_id: SourceId(Uuid::new_v4()),
                    base_url: String::new(),
                },
            );
            m
        },
        source_id: SourceId(Uuid::new_v4()),
        base_url: String::new(),
    };

    let result = validate_graph(&outer_graph, &default_schema());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("递归"));
}
