//! lj-core 类型定义集成测试。

use std::collections::HashMap;

use lj_core::*;

/// 验证 Node/Edge/Graph serde 序列化/反序列化 round-trip。
#[test]
fn test_node_edge_graph_serde_roundtrip() {
    let node_id_1 = NodeId(uuid::Uuid::new_v4());
    let node_id_2 = NodeId(uuid::Uuid::new_v4());

    let node_1 = Node {
        node_id: node_id_1.clone(),
        import_hash: "a".repeat(64),
        spec: NodeSpec {
            kind: NodeKind::Http,
            http: None,
            js: None,
            extract: None,
        },
    };

    let node_2 = Node {
        node_id: node_id_2.clone(),
        import_hash: "b".repeat(64),
        spec: NodeSpec {
            kind: NodeKind::Extract,
            http: None,
            js: None,
            extract: None,
        },
    };

    let edge = Edge {
        from: node_id_1,
        to: node_id_2,
        condition_branch: None,
    };

    let graph = Graph {
        nodes: vec![node_1, node_2],
        edges: vec![edge],
        subroutines: HashMap::new(),
        source_id: SourceId(uuid::Uuid::new_v4()),
        base_url: String::new(),
    };

    // 序列化 → JSON string
    let json = serde_json::to_string(&graph).expect("序列化 Graph 失败");
    // 反序列化 → Graph
    let deserialized: Graph = serde_json::from_str(&json).expect("反序列化 Graph 失败");

    assert_eq!(graph, deserialized, "round-trip 前后应一致");
}

/// 验证 Graph 含 subroutines 的序列化(递归类型)。
#[test]
fn test_graph_with_subroutines_serde() {
    let subroutine_id = SubroutineId(uuid::Uuid::new_v4());
    let inner_node_id = NodeId(uuid::Uuid::new_v4());
    let inner_node = Node {
        node_id: inner_node_id.clone(),
        import_hash: "inner".repeat(16), // 64 chars
        spec: NodeSpec {
            kind: NodeKind::Http,
            http: None,
            js: None,
            extract: None,
        },
    };

    let inner_graph = Graph {
        nodes: vec![inner_node],
        edges: vec![],
        subroutines: HashMap::new(),
        source_id: SourceId(uuid::Uuid::new_v4()),
        base_url: String::new(),
    };

    let outer_node_id = NodeId(uuid::Uuid::new_v4());
    let outer_node = Node {
        node_id: outer_node_id,
        import_hash: "outer".repeat(16), // 64 chars
        spec: NodeSpec {
            kind: NodeKind::Loop,
            http: None,
            js: None,
            extract: None,
        },
    };

    let mut subroutines = HashMap::new();
    subroutines.insert(subroutine_id.clone(), inner_graph);

    let graph = Graph {
        nodes: vec![outer_node],
        edges: vec![],
        subroutines,
        source_id: SourceId(uuid::Uuid::new_v4()),
        base_url: String::new(),
    };

    let json = serde_json::to_string(&graph).expect("序列化含子例程的 Graph 失败");
    let deserialized: Graph = serde_json::from_str(&json).expect("反序列化含子例程的 Graph 失败");

    assert_eq!(graph, deserialized, "递归类型 round-trip 应一致");
    // 子例程表应非空且含正确的 key
    assert!(
        deserialized.subroutines.contains_key(&subroutine_id),
        "子例程 ID 应存在"
    );
}

/// 验证 `NodeData` variant match 穷尽性。
#[test]
fn test_node_data_variant_exhaustive() {
    let variants: Vec<NodeData> = vec![
        NodeData::Raw("test".to_string()),
        NodeData::HttpResponse(HttpResponse {
            status: 200,
            headers: HashMap::new(),
            body: vec![1, 2, 3],
            charset: None,
        }),
        NodeData::Media(Media::Book(BookMedia {
            title: "测试书名".to_string(),
            author: None,
            cover_url: None,
            description: None,
            kind: None,
            last_chapter: None,
            book_url: None,
            chapters: vec![],
        })),
        NodeData::Json(serde_json::json!({"key": "value"})),
        NodeData::Error("测试错误".to_string()),
    ];

    for data in &variants {
        match data.variant() {
            NodeDataVariant::Raw => {
                assert!(matches!(data, NodeData::Raw(_)));
            }
            NodeDataVariant::HttpResponse => {
                assert!(matches!(data, NodeData::HttpResponse(_)));
            }
            NodeDataVariant::Media => {
                assert!(matches!(data, NodeData::Media(_)));
            }
            NodeDataVariant::Json => {
                assert!(matches!(data, NodeData::Json(_)));
            }
            NodeDataVariant::Error => {
                assert!(matches!(data, NodeData::Error(_)));
            }
        }
    }

    assert_eq!(variants.len(), 5, "NodeData 应有 5 个 variant");
}

/// 验证 `ExtractRule` variant 覆盖。
#[test]
fn test_extract_rule_variant_coverage() {
    let rules: Vec<ExtractRule> = vec![
        ExtractRule::CssSelector {
            selector: "h1".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        },
        ExtractRule::XPath {
            expression: "//h1/text()".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        },
        ExtractRule::JsonPath {
            path: "$.title".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        },
        ExtractRule::Regex {
            pattern: r"\d+".to_string(),
            group: 0,
            regex_clean: None,
        },
    ];

    assert_eq!(rules.len(), 4, "ExtractRule 应有 4 个 variant");

    // 验证带 regex_clean 的 variant
    let with_clean = ExtractRule::CssSelector {
        selector: ".price".to_string(),
        extract_type: ExtractType::Text,
        regex_clean: Some(RegexClean {
            pattern: r"\d+\.\d+".to_string(),
            replacement: "{{$0}}".to_string(),
        }),
    };
    assert!(
        matches!(with_clean, ExtractRule::CssSelector { .. }),
        "带 regex_clean 的 variant 应匹配"
    );
}

/// 验证 `RepoId`<T> 类型隔断(编译期类型隔离)。
#[test]
fn test_repo_id_type_isolation() {
    let id_str = "test-id".to_string();
    let graph_repo_id: RepoId<Graph> = RepoId::new(id_str.clone());
    let media_repo_id: RepoId<Media> = RepoId::new(id_str);

    // 同类型的可相等
    let graph_repo_id_2: RepoId<Graph> = RepoId::new("test-id".to_string());
    assert_eq!(graph_repo_id, graph_repo_id_2, "同类型 RepoId 应相等");

    // 不同类型的不可混用(编译期保证)
    // 下行取消注释会编译错误: expected `RepoId<Graph>`, found `RepoId<Media>`
    // let _: RepoId<Graph> = media_repo_id;
    let _ = media_repo_id; // 防止 unused warning
}

/// 验证 `GraphSchema::default_schema()` 返回 5 模板。
#[test]
fn test_default_schema_returns_five_templates() {
    let schema = GraphSchema::default_schema();
    assert_eq!(schema.templates.len(), 5, "默认 schema 应有 5 个端点模板");

    let kinds: Vec<EndpointKind> = schema.templates.iter().map(|t| t.kind.clone()).collect();

    assert!(kinds.contains(&EndpointKind::Search), "应含 Search");
    assert!(kinds.contains(&EndpointKind::Discover), "应含 Discover");
    assert!(kinds.contains(&EndpointKind::Detail), "应含 Detail");
    assert!(kinds.contains(&EndpointKind::Toc), "应含 Toc");
    assert!(kinds.contains(&EndpointKind::Content), "应含 Content");
}

/// 验证 Media enum serde round-trip。
#[test]
fn test_media_serde_roundtrip() {
    let book = Media::Book(BookMedia {
        title: "测试书".to_string(),
        author: Some("测试作者".to_string()),
        cover_url: Some("https://example.com/cover.jpg".to_string()),
        description: Some("这是一本测试书".to_string()),
        kind: Some("玄幻".to_string()),
        last_chapter: Some("第100章".to_string()),
        book_url: Some("https://example.com/book".to_string()),
        chapters: vec![
            BookChapter {
                title: "第一章".to_string(),
                chapter_url: "https://example.com/ch1".to_string(),
                content: None,
            },
            BookChapter {
                title: "第二章".to_string(),
                chapter_url: "https://example.com/ch2".to_string(),
                content: Some("正文内容".to_string()),
            },
        ],
    });

    let json = serde_json::to_string(&book).expect("序列化 BookMedia 失败");
    let deserialized: Media = serde_json::from_str(&json).expect("反序列化 BookMedia 失败");
    assert_eq!(book, deserialized, "Media round-trip 应一致");
}

/// 验证 `NodeData` `HttpResponse` serde round-trip。
#[test]
fn test_http_response_serde_roundtrip() {
    let resp = NodeData::HttpResponse(HttpResponse {
        status: 200,
        headers: {
            let mut h = HashMap::new();
            h.insert("content-type".to_string(), "text/html".to_string());
            h
        },
        body: b"<html>test</html>".to_vec(),
        charset: Some("utf-8".to_string()),
    });

    let json = serde_json::to_string(&resp).expect("序列化 HttpResponse 失败");
    let deserialized: NodeData = serde_json::from_str(&json).expect("反序列化 HttpResponse 失败");
    assert_eq!(resp, deserialized, "HttpResponse round-trip 应一致");
}
