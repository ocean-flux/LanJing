//! Native graph 集成测试。
//!
//! 显式验证 native import -> validate -> execute -> delta 闭环。

mod common;

use std::collections::HashMap;

use lj_capability::{IntentExport, IntentInput, StandardIntent};
use lj_importer::native::NativeImporter;
use lj_importer::validate::validate_graph;
use lj_media::MediaKind;
use lj_rule_model::{ExpectedDataType, ExtractRule, ExtractSpec, ExtractType, OutputTarget};
use lj_rule_model::{HttpMethod, HttpSpec};
use lj_runtime::SegmentSpec;
use lj_runtime::{
    Edge, Graph, MapperOutputKind, MapperSpec, Node, NodeId, NodeKind, NodeSpec, SourceId,
};
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn build_native_graph(base_url: &str) -> Graph {
    let source_id = SourceId(Uuid::new_v4());
    let http_id = NodeId(Uuid::new_v4());
    let extract_id = NodeId(Uuid::new_v4());
    let mapper_id = NodeId(Uuid::new_v4());

    let http_node = Node {
        node_id: http_id.clone(),
        import_hash: "http-search".to_string(),
        spec: NodeSpec {
            kind: NodeKind::Http,
            http: Some(HttpSpec {
                method: HttpMethod::Get,
                url: format!("{base_url}/search"),
                headers: HashMap::new(),
                body: None,
                charset: None,
                expected_type: ExpectedDataType::Html,
            }),
            js: None,
            extract: None,
            mapper: None,
        },
    };

    let mut field_rules = HashMap::new();
    field_rules.insert(
        "name".to_string(),
        vec![ExtractRule::CssSelector {
            selector: "h2[itemprop='name']".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }],
    );
    field_rules.insert(
        "author".to_string(),
        vec![ExtractRule::CssSelector {
            selector: "p[itemprop='author']".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }],
    );
    field_rules.insert(
        "bookUrl".to_string(),
        vec![ExtractRule::CssSelector {
            selector: "a[itemprop='url']".to_string(),
            extract_type: ExtractType::Href,
            regex_clean: None,
        }],
    );
    field_rules.insert(
        "kind".to_string(),
        vec![ExtractRule::CssSelector {
            selector: "span[itemprop='genre']".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }],
    );

    let extract_node = Node {
        node_id: extract_id.clone(),
        import_hash: "extract-search".to_string(),
        spec: NodeSpec {
            kind: NodeKind::Extract,
            http: None,
            js: None,
            extract: Some(ExtractSpec {
                rules: vec![ExtractRule::CssSelector {
                    selector: "li[itemprop='mainEntity']".to_string(),
                    extract_type: ExtractType::Text,
                    regex_clean: None,
                }],
                field_rules,
                expected_type: ExpectedDataType::Html,
                output_target: OutputTarget::Media,
            }),
            mapper: None,
        },
    };

    let mapper_node = Node {
        node_id: mapper_id.clone(),
        import_hash: "mapper-search-items".to_string(),
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
    };

    let mut intent_exports = HashMap::new();
    intent_exports.insert(
        StandardIntent::Search,
        IntentExport::new(http_id.0, mapper_id.0),
    );

    Graph {
        nodes: vec![http_node, extract_node, mapper_node],
        edges: vec![
            Edge {
                from: http_id,
                to: extract_id.clone(),
                condition_branch: None,
            },
            Edge {
                from: extract_id,
                to: mapper_id,
                condition_branch: None,
            },
        ],
        subroutines: HashMap::new(),
        source_id,
        base_url: base_url.to_string(),
        intent_exports,
    }
}

async fn mount_native_routes(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r"
            <ul>
              <li itemprop='mainEntity'>
                <h2 itemprop='name'>修罗武神</h2>
                <p itemprop='author'>善良的蜜蜂</p>
                <a itemprop='url' href='/book/1'>详情</a>
                <span itemprop='genre'>玄幻</span>
              </li>
            </ul>
            ",
        ))
        .mount(server)
        .await;
}

#[tokio::test]
async fn native_graph_executes_to_media_graph_delta() {
    common::init_tracing();
    let server = MockServer::start().await;
    mount_native_routes(&server).await;

    let original_graph = build_native_graph(&server.uri());
    let preview = NativeImporter
        .import(original_graph)
        .expect("native graph 导入失败");
    assert!(
        preview
            .graph
            .intent_exports
            .contains_key(&StandardIntent::Search),
        "native graph 应声明 Search 标准意图"
    );
    validate_graph(&preview.graph).expect("native graph 显式校验应通过");

    let results = common::execute_and_collect(
        &preview.graph,
        SegmentSpec {
            intent: StandardIntent::Search,
            input: IntentInput::Query("修罗".to_string()),
        },
        &server.uri(),
        "native_search_delta",
    )
    .await;
    let delta = common::collect_delta(&results);

    assert_eq!(delta.sources.len(), 1);
    let item = delta
        .items
        .iter()
        .find(|item| item.title == "修罗武神")
        .expect("native Search 应返回媒体主体");
    assert_eq!(item.media_kind, MediaKind::Text);
    assert_eq!(item.creators, vec!["善良的蜜蜂"]);
    assert_eq!(item.metadata["kind"], "玄幻");
}
