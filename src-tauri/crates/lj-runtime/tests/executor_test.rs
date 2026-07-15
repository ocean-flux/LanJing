//! 执行器集成测试。

use std::collections::{BTreeMap, HashMap};

use futures::stream::{BoxStream, StreamExt};
use lj_capability::{IntentExport, IntentInput, StandardIntent};
use lj_core::endpoint::{HttpMethod, HttpSpec};
use lj_core::extract_rule::{ExpectedDataType, ExtractSpec};
use lj_core::media::{
    MediaGraphDelta, MediaItem, MediaKind, MediaResourceId, ResourceCompleteness,
};
use lj_core::node::{
    Edge, Graph, MapperOutputKind, MapperSpec, Node, NodeId, NodeKind, NodeSpec, SourceId,
};
use lj_core::node_data::{HttpResponse, NodeData, NodeDataVariant};
use lj_core::sandbox::Sandbox;
use lj_core::traits::{ExecutionContext, Executor, NodeProcessor, SegmentSpec};
use lj_runtime::executor::GraphExecutor;
use lj_runtime::tap::{TapSummary, tap_stream};
use uuid::Uuid;

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
        _spec: &'a NodeSpec,
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

struct JsonToDeltaMapper;

impl NodeProcessor for JsonToDeltaMapper {
    fn kind(&self) -> NodeKind {
        NodeKind::Extract
    }

    fn input_type(&self) -> Option<NodeDataVariant> {
        Some(NodeDataVariant::Json)
    }

    fn output_type(&self) -> NodeDataVariant {
        NodeDataVariant::Delta
    }

    fn process<'a>(
        &'a self,
        _ctx: &'a ExecutionContext,
        _spec: &'a NodeSpec,
        input: BoxStream<'a, NodeData>,
    ) -> BoxStream<'a, NodeData> {
        input
            .map(|item| match item {
                NodeData::Json(value) => NodeData::Delta(MediaGraphDelta {
                    items: vec![MediaItem {
                        id: MediaResourceId("item:test:1".to_string()),
                        source_id: MediaResourceId("source:test".to_string()),
                        media_kind: MediaKind::Text,
                        title: value
                            .get("title")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("未知")
                            .to_string(),
                        subtitle: None,
                        creators: Vec::new(),
                        description: None,
                        cover_asset_id: None,
                        metadata: BTreeMap::new(),
                        completeness: ResourceCompleteness::Partial,
                        updated_at: None,
                    }],
                    ..MediaGraphDelta::default()
                }),
                other => other,
            })
            .boxed()
    }
}

struct JsonRecordProcessor;

impl NodeProcessor for JsonRecordProcessor {
    fn kind(&self) -> NodeKind {
        NodeKind::Extract
    }

    fn input_type(&self) -> Option<NodeDataVariant> {
        None
    }

    fn output_type(&self) -> NodeDataVariant {
        NodeDataVariant::Json
    }

    fn process<'a>(
        &'a self,
        _ctx: &'a ExecutionContext,
        _spec: &'a NodeSpec,
        _input: BoxStream<'a, NodeData>,
    ) -> BoxStream<'a, NodeData> {
        futures::stream::once(async {
            NodeData::Json(serde_json::json!({
                "title": "修罗武神",
                "author": "善良的蜜蜂",
                "url": "https://example.com/book/1",
                "kind": "玄幻"
            }))
        })
        .boxed()
    }
}

fn node(kind: NodeKind) -> Node {
    Node {
        node_id: NodeId(Uuid::new_v4()),
        import_hash: format!("{kind:?}"),
        spec: NodeSpec {
            kind,
            http: None,
            js: None,
            extract: None,
            mapper: None,
        },
    }
}

fn http_node(url: &str) -> Node {
    Node {
        node_id: NodeId(Uuid::new_v4()),
        import_hash: "http".to_string(),
        spec: NodeSpec {
            kind: NodeKind::Http,
            http: Some(HttpSpec {
                method: HttpMethod::Get,
                url: url.to_string(),
                headers: HashMap::new(),
                body: None,
                charset: None,
                expected_type: ExpectedDataType::Html,
            }),
            js: None,
            extract: None,
            mapper: None,
        },
    }
}

fn extract_node() -> Node {
    Node {
        node_id: NodeId(Uuid::new_v4()),
        import_hash: "extract".to_string(),
        spec: NodeSpec {
            kind: NodeKind::Extract,
            http: None,
            js: None,
            extract: Some(ExtractSpec {
                rules: Vec::new(),
                field_rules: HashMap::new(),
                expected_type: ExpectedDataType::Html,
                output_target: lj_core::extract_rule::OutputTarget::default(),
            }),
            mapper: None,
        },
    }
}

fn mapper_node(output: MapperOutputKind) -> Node {
    Node {
        node_id: NodeId(Uuid::new_v4()),
        import_hash: format!("mapper:{output:?}"),
        spec: NodeSpec {
            kind: NodeKind::Mapper,
            http: None,
            js: None,
            extract: None,
            mapper: Some(MapperSpec {
                output,
                identity_fields: vec!["url".to_string(), "title".to_string()],
            }),
        },
    }
}

fn graph_with_export(
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    intent: StandardIntent,
    flow_entry: &NodeId,
    mapper_output: &NodeId,
) -> Graph {
    let mut intent_exports = HashMap::new();
    intent_exports.insert(intent, IntentExport::new(flow_entry.0, mapper_output.0));
    Graph {
        nodes,
        edges,
        subroutines: HashMap::new(),
        source_id: SourceId(Uuid::new_v4()),
        base_url: String::new(),
        intent_exports,
    }
}

fn ctx() -> ExecutionContext {
    ExecutionContext {
        cookies: HashMap::new(),
        caps: Sandbox::default(),
        trace_id: "test-trace".into(),
        base_url: String::new(),
    }
}

#[tokio::test]
async fn tap_emits_summary() {
    let summaries: std::sync::Arc<std::sync::Mutex<Vec<TapSummary>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let summaries_clone = summaries.clone();
    let node_id = NodeId(Uuid::new_v4());
    let input = futures::stream::iter(vec![
        NodeData::Raw("test1".to_string()),
        NodeData::Raw("test2".to_string()),
    ])
    .boxed();

    let tapped = tap_stream(node_id, input, move |summary| {
        summaries_clone.lock().unwrap().push(summary);
    });

    let results: Vec<NodeData> = tapped.collect().await;
    assert_eq!(results.len(), 2);
    let guard = summaries.lock().unwrap();
    assert_eq!(guard.len(), 2);
    assert_eq!(guard[0].variant, "Raw");
    assert_eq!(guard[0].summary, "test1");
}

#[test]
fn select_subgraph_filters_by_standard_intent() {
    let http = http_node("http://example.com/search");
    let extract = extract_node();
    let other_http = http_node("http://example.com/detail");
    let graph = graph_with_export(
        vec![http.clone(), extract.clone(), other_http],
        vec![Edge {
            from: http.node_id.clone(),
            to: extract.node_id.clone(),
            condition_branch: None,
        }],
        StandardIntent::Search,
        &http.node_id,
        &extract.node_id,
    );

    let selected = GraphExecutor::new().select_subgraph(&graph, &StandardIntent::Search);
    assert_eq!(selected.len(), 2);
    assert!(selected.iter().any(|n| n.node_id == http.node_id));
    assert!(selected.iter().any(|n| n.node_id == extract.node_id));
}

#[test]
fn select_subgraph_returns_empty_when_no_entry() {
    let graph = Graph {
        nodes: vec![extract_node()],
        edges: Vec::new(),
        subroutines: HashMap::new(),
        source_id: SourceId(Uuid::new_v4()),
        base_url: String::new(),
        intent_exports: HashMap::new(),
    };

    let selected = GraphExecutor::new().select_subgraph(&graph, &StandardIntent::Search);
    assert!(selected.is_empty());
}

#[test]
fn select_subgraph_stops_at_declared_mapper_branch() {
    let http = http_node("http://example.com/detail");
    let item_mapper = extract_node();
    let unit_mapper = extract_node();
    let graph = graph_with_export(
        vec![http.clone(), item_mapper.clone(), unit_mapper.clone()],
        vec![
            Edge {
                from: http.node_id.clone(),
                to: item_mapper.node_id.clone(),
                condition_branch: None,
            },
            Edge {
                from: http.node_id.clone(),
                to: unit_mapper.node_id.clone(),
                condition_branch: None,
            },
        ],
        StandardIntent::ResolveItem,
        &http.node_id,
        &item_mapper.node_id,
    );

    let selected = GraphExecutor::new().select_subgraph(&graph, &StandardIntent::ResolveItem);
    assert_eq!(selected.len(), 2);
    assert!(selected.iter().any(|n| n.node_id == http.node_id));
    assert!(selected.iter().any(|n| n.node_id == item_mapper.node_id));
    assert!(!selected.iter().any(|n| n.node_id == unit_mapper.node_id));
}

#[tokio::test]
async fn tap_http_response_summary() {
    let summaries: std::sync::Arc<std::sync::Mutex<Vec<TapSummary>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let summaries_clone = summaries.clone();
    let input = futures::stream::iter(vec![NodeData::HttpResponse(HttpResponse {
        status: 200,
        headers: HashMap::new(),
        body: vec![0; 1024],
        charset: Some("utf-8".to_string()),
    })])
    .boxed();

    let tapped = tap_stream(NodeId(Uuid::new_v4()), input, move |summary| {
        summaries_clone.lock().unwrap().push(summary);
    });
    let _: Vec<NodeData> = tapped.collect().await;

    let guard = summaries.lock().unwrap();
    assert_eq!(guard.len(), 1);
    assert_eq!(guard[0].variant, "HttpResponse");
    assert!(guard[0].summary.contains("status=200"));
    assert!(guard[0].summary.contains("bytes=1024"));
}

#[tokio::test]
async fn tap_delta_summary() {
    let summaries: std::sync::Arc<std::sync::Mutex<Vec<TapSummary>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let summaries_clone = summaries.clone();
    let delta = MediaGraphDelta {
        items: vec![MediaItem {
            id: MediaResourceId("item:1".to_string()),
            source_id: MediaResourceId("source:1".to_string()),
            media_kind: MediaKind::Text,
            title: "标题".to_string(),
            subtitle: None,
            creators: Vec::new(),
            description: None,
            cover_asset_id: None,
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Partial,
            updated_at: None,
        }],
        ..MediaGraphDelta::default()
    };

    let input = futures::stream::iter(vec![NodeData::Delta(delta)]).boxed();
    let tapped = tap_stream(NodeId(Uuid::new_v4()), input, move |summary| {
        summaries_clone.lock().unwrap().push(summary);
    });
    let _: Vec<NodeData> = tapped.collect().await;

    let guard = summaries.lock().unwrap();
    assert_eq!(guard[0].variant, "Delta");
    assert!(guard[0].summary.contains("items=1"));
}

#[test]
fn topological_sort_linear_chain() {
    let n1 = node(NodeKind::Http);
    let n2 = node(NodeKind::Extract);
    let edges = vec![Edge {
        from: n1.node_id.clone(),
        to: n2.node_id.clone(),
        condition_branch: None,
    }];
    let nodes = [&n2, &n1];

    let sorted = GraphExecutor::new().topological_sort(&nodes, &edges);
    assert_eq!(sorted[0].node_id, n1.node_id);
    assert_eq!(sorted[1].node_id, n2.node_id);
}

#[tokio::test]
async fn entry_input_maps_standard_intent_inputs() {
    let cases = [
        (
            IntentInput::Query("修罗".to_string()),
            NodeData::Raw("修罗".to_string()),
        ),
        (
            IntentInput::ItemId("item:1".to_string()),
            NodeData::Raw("item:1".to_string()),
        ),
        (
            IntentInput::UnitId("unit:1".to_string()),
            NodeData::Raw("unit:1".to_string()),
        ),
        (
            IntentInput::ActionId("action:1".to_string()),
            NodeData::Raw("action:1".to_string()),
        ),
        (
            IntentInput::Page("2".to_string()),
            NodeData::Raw("2".to_string()),
        ),
        (IntentInput::None, NodeData::Raw(String::new())),
    ];

    for (input, expected) in cases {
        let segment = SegmentSpec {
            intent: StandardIntent::Search,
            input,
        };
        let values: Vec<NodeData> = GraphExecutor::entry_input(&segment).collect().await;
        assert_eq!(values, vec![expected]);
    }

    let segment = SegmentSpec {
        intent: StandardIntent::ContinueAction,
        input: IntentInput::Opaque(serde_json::json!({"cursor":"next"})),
    };
    let values: Vec<NodeData> = GraphExecutor::entry_input(&segment).collect().await;
    assert_eq!(
        values,
        vec![NodeData::Json(serde_json::json!({"cursor":"next"}))]
    );
}

#[tokio::test]
async fn pipeline_two_node_chain() {
    let n1 = http_node("http://example.com/search");
    let n2 = extract_node();
    let graph = graph_with_export(
        vec![n1.clone(), n2.clone()],
        vec![Edge {
            from: n1.node_id.clone(),
            to: n2.node_id.clone(),
            condition_branch: None,
        }],
        StandardIntent::Search,
        &n1.node_id,
        &n2.node_id,
    );
    let segment = SegmentSpec {
        intent: StandardIntent::Search,
        input: IntentInput::Query("测试".to_string()),
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

    let items: Vec<(NodeId, NodeData)> = GraphExecutor::new()
        .execute(&graph, segment, &ctx(), &processors)
        .collect()
        .await;

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, n2.node_id);
    assert_eq!(
        items[0].1,
        NodeData::Raw("processed: processed: 测试".to_string())
    );
}

#[tokio::test]
async fn pipeline_can_emit_media_graph_delta() {
    let n1 = node(NodeKind::Js);
    let n2 = extract_node();
    let graph = graph_with_export(
        vec![n1.clone(), n2.clone()],
        vec![Edge {
            from: n1.node_id.clone(),
            to: n2.node_id.clone(),
            condition_branch: None,
        }],
        StandardIntent::ResolveItem,
        &n1.node_id,
        &n2.node_id,
    );
    let segment = SegmentSpec {
        intent: StandardIntent::ResolveItem,
        input: IntentInput::Opaque(serde_json::json!({"title":"媒体标题"})),
    };
    let mut processors: HashMap<NodeKind, Box<dyn NodeProcessor>> = HashMap::new();
    processors.insert(NodeKind::Js, Box::new(MockProcessor { kind: NodeKind::Js }));
    processors.insert(NodeKind::Extract, Box::new(JsonToDeltaMapper));

    let items: Vec<(NodeId, NodeData)> = GraphExecutor::new()
        .execute(&graph, segment, &ctx(), &processors)
        .collect()
        .await;

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, n2.node_id);
    let NodeData::Delta(delta) = &items[0].1 else {
        panic!("期望媒体资源图增量, 得到 {:?}", items[0].1);
    };
    assert_eq!(delta.items[0].title, "媒体标题");
}

#[tokio::test]
async fn pipeline_maps_declared_mapper_json_to_media_graph_delta() {
    let extract = extract_node();
    let mapper = mapper_node(MapperOutputKind::Items);
    let graph = graph_with_export(
        vec![extract.clone(), mapper.clone()],
        vec![Edge {
            from: extract.node_id.clone(),
            to: mapper.node_id.clone(),
            condition_branch: None,
        }],
        StandardIntent::Search,
        &extract.node_id,
        &mapper.node_id,
    );
    let segment = SegmentSpec {
        intent: StandardIntent::Search,
        input: IntentInput::Query("修罗".to_string()),
    };
    let mut processors: HashMap<NodeKind, Box<dyn NodeProcessor>> = HashMap::new();
    processors.insert(NodeKind::Extract, Box::new(JsonRecordProcessor));

    let items: Vec<(NodeId, NodeData)> = GraphExecutor::new()
        .execute(&graph, segment, &ctx(), &processors)
        .collect()
        .await;

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, mapper.node_id);
    let NodeData::Delta(delta) = &items[0].1 else {
        panic!("期望受控 Mapper 输出媒体资源图增量, 得到 {:?}", items[0].1);
    };
    assert_eq!(delta.sources.len(), 1);
    assert_eq!(delta.items.len(), 1);
    assert_eq!(delta.items[0].title, "修罗武神");
    assert_eq!(delta.items[0].creators, vec!["善良的蜜蜂"]);
    assert_eq!(delta.items[0].metadata["kind"], "玄幻");
}

#[tokio::test]
async fn pipeline_empty_subgraph_returns_empty() {
    let graph = Graph {
        nodes: vec![],
        edges: vec![],
        subroutines: HashMap::new(),
        source_id: SourceId(Uuid::new_v4()),
        base_url: String::new(),
        intent_exports: HashMap::new(),
    };
    let segment = SegmentSpec {
        intent: StandardIntent::Search,
        input: IntentInput::Query("测试".to_string()),
    };
    let processors: HashMap<NodeKind, Box<dyn NodeProcessor>> = HashMap::new();
    let items: Vec<(NodeId, NodeData)> = GraphExecutor::new()
        .execute(&graph, segment, &ctx(), &processors)
        .collect()
        .await;
    assert!(items.is_empty());
}

#[tokio::test]
async fn pipeline_missing_processor_returns_error() {
    let n1 = http_node("http://example.com/search");
    let graph = graph_with_export(
        vec![n1.clone()],
        vec![],
        StandardIntent::Search,
        &n1.node_id,
        &n1.node_id,
    );
    let segment = SegmentSpec {
        intent: StandardIntent::Search,
        input: IntentInput::Query("测试".to_string()),
    };
    let processors: HashMap<NodeKind, Box<dyn NodeProcessor>> = HashMap::new();

    let items: Vec<(NodeId, NodeData)> = GraphExecutor::new()
        .execute(&graph, segment, &ctx(), &processors)
        .collect()
        .await;

    assert_eq!(items.len(), 1);
    assert!(matches!(&items[0].1, NodeData::Error(_)));
}
