//! lj-core 类型定义集成测试。

use std::collections::{BTreeMap, HashMap};

use lj_capability::{IntentExport, StandardIntent};
use lj_core::extract_rule::{ExtractRule, ExtractType, RegexClean};
use lj_core::media::{
    MediaAsset, MediaAssetKind, MediaAssetLocator, MediaGraphDelta, MediaItem, MediaKind,
    MediaResourceId, MediaUnit, ResourceCompleteness,
};
use lj_core::node::{Edge, Graph, Node, NodeId, NodeKind, NodeSpec, SourceId, SubroutineId};
use lj_core::node_data::{HttpResponse, NodeData, NodeDataVariant};
use lj_core::traits::RepoId;

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
            mapper: None,
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
            mapper: None,
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
        intent_exports: HashMap::new(),
    };

    let json = serde_json::to_string(&graph).expect("序列化 Graph 失败");
    let deserialized: Graph = serde_json::from_str(&json).expect("反序列化 Graph 失败");

    assert_eq!(graph, deserialized, "round-trip 前后应一致");
}

/// 验证 Graph 含 subroutines 的序列化。
#[test]
fn test_graph_with_subroutines_serde() {
    let subroutine_id = SubroutineId(uuid::Uuid::new_v4());
    let inner_node_id = NodeId(uuid::Uuid::new_v4());
    let inner_node = Node {
        node_id: inner_node_id,
        import_hash: "inner".repeat(16),
        spec: NodeSpec {
            kind: NodeKind::Http,
            http: None,
            js: None,
            extract: None,
            mapper: None,
        },
    };

    let inner_graph = Graph {
        nodes: vec![inner_node],
        edges: vec![],
        subroutines: HashMap::new(),
        source_id: SourceId(uuid::Uuid::new_v4()),
        base_url: String::new(),
        intent_exports: HashMap::new(),
    };

    let outer_node = Node {
        node_id: NodeId(uuid::Uuid::new_v4()),
        import_hash: "outer".repeat(16),
        spec: NodeSpec {
            kind: NodeKind::Loop,
            http: None,
            js: None,
            extract: None,
            mapper: None,
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
        intent_exports: HashMap::new(),
    };

    let json = serde_json::to_string(&graph).expect("序列化含子例程的 Graph 失败");
    let deserialized: Graph = serde_json::from_str(&json).expect("反序列化含子例程的 Graph 失败");

    assert_eq!(graph, deserialized, "递归类型 round-trip 应一致");
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
        NodeData::Json(serde_json::json!({"key": "value"})),
        NodeData::Delta(MediaGraphDelta::default()),
        NodeData::Error("测试错误".to_string()),
    ];

    for data in &variants {
        match data.variant() {
            NodeDataVariant::Raw => assert!(matches!(data, NodeData::Raw(_))),
            NodeDataVariant::HttpResponse => assert!(matches!(data, NodeData::HttpResponse(_))),
            NodeDataVariant::Json => assert!(matches!(data, NodeData::Json(_))),
            NodeDataVariant::Delta => assert!(matches!(data, NodeData::Delta(_))),
            NodeDataVariant::Error => assert!(matches!(data, NodeData::Error(_))),
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

/// 验证 `RepoId`<T> 类型隔断。
#[test]
fn test_repo_id_type_isolation() {
    let id_str = "test-id".to_string();
    let graph_repo_id: RepoId<Graph> = RepoId::new(id_str.clone());
    let media_repo_id: RepoId<MediaItem> = RepoId::new(id_str);

    let graph_repo_id_2: RepoId<Graph> = RepoId::new("test-id".to_string());
    assert_eq!(graph_repo_id, graph_repo_id_2, "同类型 RepoId 应相等");

    // 不同类型不可混用，取消下一行注释会编译失败。
    // let _: RepoId<Graph> = media_repo_id;
    let _ = media_repo_id;
}

/// 验证 `Graph` 含 `intent_exports` 的 serde round-trip。
#[test]
fn test_graph_with_intent_exports_serde() {
    let flow_entry = uuid::Uuid::new_v4();
    let mapper_output = uuid::Uuid::new_v4();

    let mut intent_exports = HashMap::new();
    intent_exports.insert(
        StandardIntent::Search,
        IntentExport::new(flow_entry, mapper_output),
    );

    let graph = Graph {
        nodes: vec![],
        edges: vec![],
        subroutines: HashMap::new(),
        source_id: SourceId(uuid::Uuid::new_v4()),
        base_url: "https://example.com".to_string(),
        intent_exports,
    };

    let json = serde_json::to_string(&graph).expect("序列化含 intent_exports 的 Graph 失败");
    let deserialized: Graph =
        serde_json::from_str(&json).expect("反序列化含 intent_exports 的 Graph 失败");

    assert_eq!(graph, deserialized, "intent_exports round-trip 应一致");
    assert_eq!(
        deserialized.intent_exports.len(),
        1,
        "应有 1 个标准意图入口"
    );
    assert_eq!(
        deserialized.intent_exports[&StandardIntent::Search].mapper_output,
        mapper_output,
        "标准意图应保留 Mapper 输出节点"
    );
}

/// 验证标准媒体资源图增量 serde round-trip。
#[test]
fn test_media_graph_delta_serde_roundtrip() {
    let source_id = MediaResourceId("source:demo".to_string());
    let item_id = MediaResourceId("item:demo:1".to_string());
    let unit_id = MediaResourceId("unit:demo:1:1".to_string());
    let asset_id = MediaResourceId("asset:demo:1:1:text".to_string());
    let delta = MediaGraphDelta {
        items: vec![MediaItem {
            id: item_id.clone(),
            source_id: source_id.clone(),
            media_kind: MediaKind::Text,
            title: "测试书名".to_string(),
            subtitle: None,
            creators: vec!["作者".to_string()],
            description: None,
            cover_asset_id: None,
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Partial,
            updated_at: None,
        }],
        units: vec![MediaUnit {
            id: unit_id.clone(),
            source_id: source_id.clone(),
            item_id: item_id.clone(),
            title: "第一章".to_string(),
            position: Some(1),
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Complete,
        }],
        assets: vec![MediaAsset {
            id: asset_id,
            source_id,
            unit_id: Some(unit_id),
            asset_kind: MediaAssetKind::Text,
            locator: MediaAssetLocator::Text("正文内容".to_string()),
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Complete,
        }],
        ..MediaGraphDelta::default()
    };

    let data = NodeData::Delta(delta);
    let json = serde_json::to_string(&data).expect("序列化 Delta 失败");
    let back: NodeData = serde_json::from_str(&json).expect("反序列化 Delta 失败");
    assert_eq!(data, back);
}

/// 验证 `Graph` 反序列化缺少 `intent_exports` 时默认空 `HashMap`。
#[test]
fn test_graph_deserialize_missing_intent_exports_defaults_empty() {
    let json = r#"{
        "nodes": [],
        "edges": [],
        "subroutines": {},
        "source_id": "00000000-0000-0000-0000-000000000001",
        "base_url": ""
    }"#;
    let graph: Graph = serde_json::from_str(json).expect("反序列化旧格式 Graph 失败");
    assert!(
        graph.intent_exports.is_empty(),
        "缺少 intent_exports 应默认空 HashMap"
    );
}
