//! 执行器集成测试。
//!
//! # 覆盖
//!
//! 1. `tap_stream` 正确 emit summary 并透传 item
//! 2. 子图裁剪正确按端点类型选节点
//! 3. `topological_sort` 正确排序
//! 4. `entry_input` 按端点类型注入正确参数
//! 5. 完整 pipeline 执行(含 mock processor)

use std::collections::HashMap;

use futures::stream::{BoxStream, StreamExt};
use lj_core::endpoint::{EndpointKind, HttpMethod, HttpSpec};
use lj_core::extract_rule::{ExpectedDataType, ExtractSpec};
use lj_core::node::{Edge, Graph, Node, NodeId, NodeKind, NodeSpec, SourceId};
use lj_core::node_data::{HttpResponse, NodeData, NodeDataVariant};
use lj_core::sandbox::Sandbox;
use lj_core::traits::{ExecutionContext, Executor, NodeProcessor, SegmentSpec};
use lj_runtime::executor::GraphExecutor;
use lj_runtime::tap::{NodeDataSummary, tap_stream};
use uuid::Uuid;

// ===== tap_stream 测试 =====

/// `tap_stream` 正确 emit summary 并透传 item。
#[tokio::test]
async fn tap_emits_summary() {
    let summaries: std::sync::Arc<std::sync::Mutex<Vec<NodeDataSummary>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let summaries_clone = summaries.clone();
    let node_id = NodeId(Uuid::new_v4());

    let input = futures::stream::iter(vec![
        NodeData::Raw("test1".to_string()),
        NodeData::Raw("test2".to_string()),
    ])
    .boxed();

    let tapped = tap_stream(input, node_id, move |summary| {
        summaries_clone.lock().unwrap().push(summary);
        async {}
    });

    let results: Vec<NodeData> = tapped.collect().await;
    assert_eq!(results.len(), 2);
    assert_eq!(summaries.lock().unwrap().len(), 2);

    // 验证 summary 内容
    let first = &summaries.lock().unwrap()[0];
    assert_eq!(first.variant, "Raw");
    assert_eq!(first.summary, "test1");
}

// ===== select_subgraph 测试 =====

/// 子图裁剪:按 search endpoint 选正确节点。
#[tokio::test]
async fn select_subgraph_filters_by_endpoint_kind() {
    let gen_id = || NodeId(Uuid::new_v4());

    let http_node = Node {
        node_id: gen_id(),
        import_hash: "a".to_string(),
        spec: NodeSpec {
            kind: NodeKind::Http,
            http: Some(HttpSpec {
                endpoint_kind: EndpointKind::Search,
                method: HttpMethod::Get,
                url: "http://example.com/search".to_string(),
                headers: HashMap::new(),
                body: None,
                charset: None,
                expected_type: ExpectedDataType::Html,
            }),
            js: None,
            extract: None,
        },
    };

    let extract_node = Node {
        node_id: gen_id(),
        import_hash: "b".to_string(),
        spec: NodeSpec {
            kind: NodeKind::Extract,
            http: None,
            js: None,
            extract: Some(ExtractSpec {
                rules: Vec::new(),
                field_rules: HashMap::new(),
                endpoint_kind: None,
                expected_type: ExpectedDataType::Html,
                play_url_parser: None,
            }),
        },
    };

    // 另一个端点的 Http 节点(不应被选中)
    let other_http = Node {
        node_id: gen_id(),
        import_hash: "c".to_string(),
        spec: NodeSpec {
            kind: NodeKind::Http,
            http: Some(HttpSpec {
                endpoint_kind: EndpointKind::Detail,
                method: HttpMethod::Get,
                url: "http://example.com/detail".to_string(),
                headers: HashMap::new(),
                body: None,
                charset: None,
                expected_type: ExpectedDataType::Html,
            }),
            js: None,
            extract: None,
        },
    };

    let from_id = http_node.node_id.clone();
    let to_id = extract_node.node_id.clone();
    let source_id = SourceId(Uuid::new_v4());
    let graph = Graph {
        nodes: vec![http_node, extract_node, other_http],
        edges: vec![Edge {
            from: from_id,
            to: to_id,
            condition_branch: None,
        }],
        subroutines: HashMap::new(),
        source_id,
        base_url: String::new(),
    };

    let executor = GraphExecutor::new();
    let selected = executor.select_subgraph(&graph, &EndpointKind::Search);

    // 应选中 http_node(search) + extract_node, 排除 other_http(detail)
    assert_eq!(selected.len(), 2);
    assert!(selected.iter().any(|n| n.spec.kind == NodeKind::Http));
    assert!(selected.iter().any(|n| n.spec.kind == NodeKind::Extract));
}

/// 子图裁剪:无匹配 entry 时返回空。
#[test]
fn select_subgraph_returns_empty_when_no_entry() {
    let graph = Graph {
        nodes: vec![Node {
            node_id: NodeId(Uuid::new_v4()),
            import_hash: "d".to_string(),
            spec: NodeSpec {
                kind: NodeKind::Extract,
                http: None,
                js: None,
                extract: None,
            },
        }],
        edges: Vec::new(),
        subroutines: HashMap::new(),
        source_id: SourceId(Uuid::new_v4()),
        base_url: String::new(),
    };

    let executor = GraphExecutor::new();
    let selected = executor.select_subgraph(&graph, &EndpointKind::Search);
    assert!(selected.is_empty());
}

// ===== tap_stream + summarize 测试 =====

/// `summarize` 对 `HttpResponse` 正确提取状态码和 body 长度。
#[tokio::test]
async fn tap_http_response_summary() {
    let summaries: std::sync::Arc<std::sync::Mutex<Vec<NodeDataSummary>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let summaries_clone = summaries.clone();
    let node_id = NodeId(Uuid::new_v4());

    let resp = NodeData::HttpResponse(HttpResponse {
        status: 200,
        headers: HashMap::new(),
        body: vec![0; 1024],
        charset: Some("utf-8".to_string()),
    });

    let input = futures::stream::iter(vec![resp]).boxed();

    let tapped = tap_stream(input, node_id, move |s| {
        summaries_clone.lock().unwrap().push(s);
        async {}
    });

    let _: Vec<NodeData> = tapped.collect().await;

    let guard = summaries.lock().unwrap();
    assert_eq!(guard.len(), 1);
    assert_eq!(guard[0].variant, "HttpResponse");
    assert!(guard[0].summary.contains("status=200"));
    assert!(guard[0].summary.contains("body_len=1024"));
}

/// `NodeData::Json` 截断测试(超过 200 字符)。
#[tokio::test]
async fn tap_json_truncation() {
    let summaries: std::sync::Arc<std::sync::Mutex<Vec<NodeDataSummary>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let summaries_clone = summaries.clone();
    let node_id = NodeId(Uuid::new_v4());

    // 构造超过 200 字符的 JSON
    let long_val = "a".repeat(300);
    let json = NodeData::Json(serde_json::json!({"data": long_val}));

    let input = futures::stream::iter(vec![json]).boxed();

    let tapped = tap_stream(input, node_id, move |s| {
        summaries_clone.lock().unwrap().push(s);
        async {}
    });

    let _: Vec<NodeData> = tapped.collect().await;

    let guard = summaries.lock().unwrap();
    assert_eq!(guard[0].variant, "Json");
    assert!(guard[0].summary.contains("..."));
}

// ===== topological_sort 测试 =====

/// 拓扑排序:线性链 Http→Extract。
#[test]
fn topological_sort_linear_chain() {
    let n1 = NodeId(Uuid::new_v4());
    let n2 = NodeId(Uuid::new_v4());

    let nodes = [
        Node {
            node_id: n1.clone(),
            import_hash: "a".into(),
            spec: NodeSpec {
                kind: NodeKind::Http,
                http: None,
                js: None,
                extract: None,
            },
        },
        Node {
            node_id: n2.clone(),
            import_hash: "b".into(),
            spec: NodeSpec {
                kind: NodeKind::Extract,
                http: None,
                js: None,
                extract: None,
            },
        },
    ];
    let edges = [Edge {
        from: n1.clone(),
        to: n2.clone(),
        condition_branch: None,
    }];

    let executor = GraphExecutor::new();
    let refs: Vec<&Node> = nodes.iter().collect();
    let sorted = executor.topological_sort(&refs, &edges);

    assert_eq!(sorted.len(), 2);
    assert_eq!(sorted[0].node_id, n1);
    assert_eq!(sorted[1].node_id, n2);
}

/// 拓扑排序:多节点链。
#[test]
fn topological_sort_three_node_chain() {
    let n1 = NodeId(Uuid::new_v4());
    let n2 = NodeId(Uuid::new_v4());
    let n3 = NodeId(Uuid::new_v4());

    let nodes = [
        Node {
            node_id: n1.clone(),
            import_hash: "a".into(),
            spec: NodeSpec {
                kind: NodeKind::Http,
                http: None,
                js: None,
                extract: None,
            },
        },
        Node {
            node_id: n2.clone(),
            import_hash: "b".into(),
            spec: NodeSpec {
                kind: NodeKind::Js,
                http: None,
                js: None,
                extract: None,
            },
        },
        Node {
            node_id: n3.clone(),
            import_hash: "c".into(),
            spec: NodeSpec {
                kind: NodeKind::Extract,
                http: None,
                js: None,
                extract: None,
            },
        },
    ];
    let edges = [
        Edge {
            from: n1.clone(),
            to: n2.clone(),
            condition_branch: None,
        },
        Edge {
            from: n2.clone(),
            to: n3.clone(),
            condition_branch: None,
        },
    ];

    let executor = GraphExecutor::new();
    let refs: Vec<&Node> = nodes.iter().collect();
    let sorted = executor.topological_sort(&refs, &edges);

    assert_eq!(sorted.len(), 3);
    assert_eq!(sorted[0].node_id, n1);
    assert_eq!(sorted[1].node_id, n2);
    assert_eq!(sorted[2].node_id, n3);
}

/// 拓扑排序:单节点(无入边)。
#[test]
fn topological_sort_single_node() {
    let n1 = NodeId(Uuid::new_v4());
    let nodes = [Node {
        node_id: n1.clone(),
        import_hash: "a".into(),
        spec: NodeSpec {
            kind: NodeKind::Http,
            http: None,
            js: None,
            extract: None,
        },
    }];

    let executor = GraphExecutor::new();
    let refs: Vec<&Node> = nodes.iter().collect();
    let sorted = executor.topological_sort(&refs, &[]);

    assert_eq!(sorted.len(), 1);
    assert_eq!(sorted[0].node_id, n1);
}

// ===== entry_input 测试 =====

/// `entry_input:Search` 段注入 query。
#[test]
fn entry_input_search() {
    let segment = SegmentSpec {
        endpoint_kind: EndpointKind::Search,
        query: Some("修罗".to_string()),
        book_url: None,
        chapter_url: None,
        vod_id: None,
    };
    let stream = GraphExecutor::entry_input(&segment);
    let rt = tokio::runtime::Runtime::new().expect("创建 tokio runtime");
    let items: Vec<NodeData> = rt.block_on(stream.collect());
    assert_eq!(items.len(), 1);
    assert_eq!(items[0], NodeData::Raw("修罗".to_string()));
}

/// `entry_input:Detail` 段注入 `book_url`。
#[test]
fn entry_input_detail() {
    let segment = SegmentSpec {
        endpoint_kind: EndpointKind::Detail,
        query: None,
        book_url: Some("http://source.com/book/1".to_string()),
        chapter_url: None,
        vod_id: None,
    };
    let stream = GraphExecutor::entry_input(&segment);
    let rt = tokio::runtime::Runtime::new().expect("创建 tokio runtime");
    let items: Vec<NodeData> = rt.block_on(stream.collect());
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        NodeData::Raw("http://source.com/book/1".to_string())
    );
}

/// `entry_input:Toc` 段注入 `book_url`。
#[test]
fn entry_input_toc() {
    let segment = SegmentSpec {
        endpoint_kind: EndpointKind::Toc,
        query: None,
        book_url: Some("http://source.com/book/2".to_string()),
        chapter_url: None,
        vod_id: None,
    };
    let stream = GraphExecutor::entry_input(&segment);
    let rt = tokio::runtime::Runtime::new().expect("创建 tokio runtime");
    let items: Vec<NodeData> = rt.block_on(stream.collect());
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        NodeData::Raw("http://source.com/book/2".to_string())
    );
}

/// `entry_input:Content` 段注入 `chapter_url`。
#[test]
fn entry_input_content() {
    let segment = SegmentSpec {
        endpoint_kind: EndpointKind::Content,
        query: None,
        book_url: None,
        chapter_url: Some("http://source.com/chapter/3".to_string()),
        vod_id: None,
    };
    let stream = GraphExecutor::entry_input(&segment);
    let rt = tokio::runtime::Runtime::new().expect("创建 tokio runtime");
    let items: Vec<NodeData> = rt.block_on(stream.collect());
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        NodeData::Raw("http://source.com/chapter/3".to_string())
    );
}

/// `entry_input:Discover` 段返回空 stream。
#[test]
fn entry_input_discover() {
    let segment = SegmentSpec {
        endpoint_kind: EndpointKind::Discover,
        query: None,
        book_url: None,
        chapter_url: None,
        vod_id: None,
    };
    let stream = GraphExecutor::entry_input(&segment);
    let rt = tokio::runtime::Runtime::new().expect("创建 tokio runtime");
    let items: Vec<NodeData> = rt.block_on(stream.collect());
    assert!(items.is_empty());
}

/// `entry_input:段参数缺失时注入空字符串`。
#[test]
fn entry_input_missing_param() {
    let segment = SegmentSpec {
        endpoint_kind: EndpointKind::Search,
        query: None,
        book_url: None,
        chapter_url: None,
        vod_id: None,
    };
    let stream = GraphExecutor::entry_input(&segment);
    let rt = tokio::runtime::Runtime::new().expect("创建 tokio runtime");
    let items: Vec<NodeData> = rt.block_on(stream.collect());
    assert_eq!(items.len(), 1);
    assert_eq!(items[0], NodeData::Raw(String::new()));
}

// ===== pipeline 集成测试 =====

/// 模拟 processor:将输入首项转换为 `processed: {input}`。
struct MockProcessor {
    kind: NodeKind,
}

impl NodeProcessor for MockProcessor {
    fn kind(&self) -> NodeKind {
        self.kind.clone()
    }

    fn input_type(&self) -> Option<NodeDataVariant> {
        None
    }

    fn output_type(&self) -> NodeDataVariant {
        NodeDataVariant::Raw
    }

    fn process<'a>(
        &'a self,
        _ctx: &'a ExecutionContext,
        _spec: &'a lj_core::node::NodeSpec,
        input: BoxStream<'a, NodeData>,
    ) -> BoxStream<'a, NodeData> {
        input
            .map(|item| match item {
                NodeData::Raw(s) => NodeData::Raw(format!("processed: {s}")),
                other => other,
            })
            .boxed()
    }
}

/// pipeline:Http + Extract 两个 mock processor 链。
#[tokio::test]
async fn pipeline_two_node_chain() {
    let n1 = NodeId(Uuid::new_v4());
    let n2 = NodeId(Uuid::new_v4());

    let graph = Graph {
        nodes: vec![
            Node {
                node_id: n1.clone(),
                import_hash: "a".into(),
                spec: NodeSpec {
                    kind: NodeKind::Http,
                    http: Some(HttpSpec {
                        endpoint_kind: EndpointKind::Search,
                        method: HttpMethod::Get,
                        url: "http://example.com/search".to_string(),
                        headers: HashMap::new(),
                        body: None,
                        charset: None,
                        expected_type: ExpectedDataType::Html,
                    }),
                    js: None,
                    extract: None,
                },
            },
            Node {
                node_id: n2.clone(),
                import_hash: "b".into(),
                spec: NodeSpec {
                    kind: NodeKind::Extract,
                    http: None,
                    js: None,
                    extract: Some(ExtractSpec {
                        rules: Vec::new(),
                        field_rules: HashMap::new(),
                        endpoint_kind: None,
                        expected_type: ExpectedDataType::Html,
                        play_url_parser: None,
                    }),
                },
            },
        ],
        edges: vec![Edge {
            from: n1.clone(),
            to: n2.clone(),
            condition_branch: None,
        }],
        subroutines: HashMap::new(),
        source_id: SourceId(Uuid::new_v4()),
        base_url: String::new(),
    };

    let segment = SegmentSpec {
        endpoint_kind: EndpointKind::Search,
        query: Some("测试".to_string()),
        book_url: None,
        chapter_url: None,
        vod_id: None,
    };

    let ctx = ExecutionContext {
        cookies: HashMap::new(),
        caps: Sandbox::default(),
        trace_id: "test-trace".into(),
        base_url: String::new(),
    };

    let mut processors: HashMap<NodeKind, Box<dyn NodeProcessor>> = HashMap::new();
    processors.insert(
        NodeKind::Http,
        Box::new(MockProcessor {
            kind: NodeKind::Http,
        }),
    );
    processors.insert(
        NodeKind::Extract,
        Box::new(MockProcessor {
            kind: NodeKind::Extract,
        }),
    );

    let executor = GraphExecutor::new();
    let output = executor.execute(&graph, segment, &ctx, &processors);
    let items: Vec<(NodeId, NodeData)> = output.collect().await;

    // 两个 mock processor 分别添加 "processed: " 前缀
    // entry: "测试" → Http: "processed: 测试" → Extract: "processed: processed: 测试"
    assert_eq!(items.len(), 1);
    // node_id 应为最后一个节点(Extract),即 n2
    assert_eq!(items[0].0, n2);
    assert_eq!(
        items[0].1,
        NodeData::Raw("processed: processed: 测试".to_string())
    );
}

/// pipeline:搜索无结果,返回空 stream。
#[tokio::test]
async fn pipeline_empty_subgraph_returns_empty() {
    let graph = Graph {
        nodes: vec![],
        edges: vec![],
        subroutines: HashMap::new(),
        source_id: SourceId(Uuid::new_v4()),
        base_url: String::new(),
    };

    let segment = SegmentSpec {
        endpoint_kind: EndpointKind::Search,
        query: Some("测试".to_string()),
        book_url: None,
        chapter_url: None,
        vod_id: None,
    };

    let ctx = ExecutionContext {
        cookies: HashMap::new(),
        caps: Sandbox::default(),
        trace_id: "test-trace".into(),
        base_url: String::new(),
    };

    let processors: HashMap<NodeKind, Box<dyn NodeProcessor>> = HashMap::new();
    let executor = GraphExecutor::new();
    let output = executor.execute(&graph, segment, &ctx, &processors);
    let items: Vec<(NodeId, NodeData)> = output.collect().await;
    assert!(items.is_empty());
}

/// pipeline:缺少 processor 时返回 Error。
#[tokio::test]
async fn pipeline_missing_processor_returns_error() {
    let n1 = NodeId(Uuid::new_v4());

    let graph = Graph {
        nodes: vec![Node {
            node_id: n1,
            import_hash: "a".into(),
            spec: NodeSpec {
                kind: NodeKind::Http,
                http: Some(HttpSpec {
                    endpoint_kind: EndpointKind::Search,
                    method: HttpMethod::Get,
                    url: "http://example.com/search".to_string(),
                    headers: HashMap::new(),
                    body: None,
                    charset: None,
                    expected_type: ExpectedDataType::Html,
                }),
                js: None,
                extract: None,
            },
        }],
        edges: vec![],
        subroutines: HashMap::new(),
        source_id: SourceId(Uuid::new_v4()),
        base_url: String::new(),
    };

    let segment = SegmentSpec {
        endpoint_kind: EndpointKind::Search,
        query: Some("测试".to_string()),
        book_url: None,
        chapter_url: None,
        vod_id: None,
    };

    let ctx = ExecutionContext {
        cookies: HashMap::new(),
        caps: Sandbox::default(),
        trace_id: "test-trace".into(),
        base_url: String::new(),
    };

    // 不注册 Http processor
    let processors: HashMap<NodeKind, Box<dyn NodeProcessor>> = HashMap::new();
    let executor = GraphExecutor::new();
    let output = executor.execute(&graph, segment, &ctx, &processors);
    let items: Vec<(NodeId, NodeData)> = output.collect().await;

    assert_eq!(items.len(), 1);
    assert!(matches!(&items[0].1, NodeData::Error(_)));
}
