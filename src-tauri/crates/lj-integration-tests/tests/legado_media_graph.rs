//! Legado 本地 fixture 集成测试。
//!
//! 使用本地 mock server 验证导入、执行和标准媒体资源图增量闭环。

mod common;

use lj_capability::{IntentInput, StandardIntent};
use lj_importer::legado::{LegadoImporter, LegadoSourceJson};
use lj_media::{MediaAssetLocator, MediaKind};
use lj_runtime::Graph;
use lj_runtime::SegmentSpec;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn source_json(base_url: &str) -> LegadoSourceJson {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("legado_star_free_novel.json");
    let json = std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("读取 fixture 失败: {}: {e}", fixture_path.display()))
        .replace("http:\\/\\/mock.local", &base_url.replace('/', "\\/"));
    serde_json::from_str(&json).expect("反序列化 Legado fixture")
}

fn import_source(base_url: &str) -> Graph {
    let preview = LegadoImporter
        .import(source_json(base_url))
        .expect("导入 Legado fixture 失败");
    preview.graph
}

fn assert_standard_intents(graph: &Graph) {
    for intent in [
        StandardIntent::Search,
        StandardIntent::Discover,
        StandardIntent::ResolveItem,
        StandardIntent::ListUnits,
        StandardIntent::ResolveAsset,
        StandardIntent::ContinueAction,
    ] {
        assert!(
            graph.intent_exports.contains_key(&intent),
            "Legado fixture 应声明标准意图 {intent:?}"
        );
    }
}

async fn mount_legado_routes(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("q", "修罗"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r"
            <ul>
              <li itemprop='mainEntity'>
                <h2 itemprop='name'>修罗武神</h2>
                <p itemprop='author'>善良的蜜蜂</p>
                <a itemprop='url' href='/book/1'>详情</a>
                <img itemprop='image' src='/cover/1.jpg' />
                <span itemprop='genre'>玄幻</span>
              </li>
            </ul>
            ",
        ))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/haomenzongcai"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r"
            <section>
              <ul>
                <li itemprop='mainEntity'>
                  <h2 itemprop='name'>豪门测试书</h2>
                  <p itemprop='author'>作者甲</p>
                  <a itemprop='url' href='/book/2'>详情</a>
                  <img itemprop='image' src='/cover/2.jpg' />
                </li>
              </ul>
            </section>
            ",
        ))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/book/1"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r"
            <main>
              <h1>修罗武神</h1>
              <figure><img src='/cover/1.jpg' /></figure>
              <a href='/author/bee'>善良的蜜蜂</a>
              <div itemprop='description'>少年成长为强者。</div>
              <ol><li>占位</li><li>占位</li><li><a>玄幻</a></li></ol>
              <span>10万字</span>
              <div id='full-catalog'>
                <a href='/read/1.html'>第一章 起始</a>
                <a href='/read/2.html'>第二章 风起</a>
              </div>
            </main>
            ",
        ))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/read/1.html"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r"<article id='article-content'><p>第一章 起始</p><p>正文内容</p></article>",
        ))
        .mount(server)
        .await;
}

#[tokio::test]
async fn legado_fixture_executes_to_media_graph_delta() {
    common::init_tracing();
    let server = MockServer::start().await;
    mount_legado_routes(&server).await;
    let graph = import_source(&server.uri());
    assert_standard_intents(&graph);

    let search = common::execute_and_collect(
        &graph,
        SegmentSpec {
            intent: StandardIntent::Search,
            input: IntentInput::Query("修罗".to_string()),
        },
        &server.uri(),
        "legado_search_delta",
    )
    .await;
    let search_delta = common::collect_delta(&search);
    assert_eq!(search_delta.sources.len(), 1);
    let item = search_delta
        .items
        .iter()
        .find(|item| item.title == "修罗武神")
        .expect("Search 应返回媒体主体");
    assert_eq!(item.media_kind, MediaKind::Text);
    assert_eq!(item.creators, vec!["善良的蜜蜂"]);
    assert_eq!(item.metadata["kind"], "玄幻");

    let discover = common::execute_and_collect(
        &graph,
        SegmentSpec {
            intent: StandardIntent::Discover,
            input: IntentInput::None,
        },
        &server.uri(),
        "legado_discover_delta",
    )
    .await;
    let discover_delta = common::collect_delta(&discover);
    let action = discover_delta
        .actions
        .iter()
        .find(|action| action.label == "豪门")
        .expect("Discover 应返回可继续动作");
    assert_eq!(action.intent, StandardIntent::ContinueAction);
    assert!(
        action.payload["url"]
            .as_str()
            .is_some_and(|url| url.contains("page=1"))
    );

    let continued = common::execute_and_collect(
        &graph,
        SegmentSpec {
            intent: StandardIntent::ContinueAction,
            input: IntentInput::Opaque(action.payload.clone()),
        },
        &server.uri(),
        "legado_continue_delta",
    )
    .await;
    let continued_delta = common::collect_delta(&continued);
    assert!(
        continued_delta
            .items
            .iter()
            .any(|item| item.title == "豪门测试书"),
        "ContinueAction 应返回下一页媒体主体: {continued_delta:?}"
    );

    let item_id = item.id.0.clone();
    let units = common::execute_and_collect(
        &graph,
        SegmentSpec {
            intent: StandardIntent::ListUnits,
            input: IntentInput::ItemId(item_id.clone()),
        },
        &server.uri(),
        "legado_units_delta",
    )
    .await;
    let units_delta = common::collect_delta(&units);
    let unit = units_delta
        .units
        .iter()
        .find(|unit| unit.title.contains("第一章"))
        .expect("ListUnits 应返回章节单元");

    let assets = common::execute_and_collect(
        &graph,
        SegmentSpec {
            intent: StandardIntent::ResolveAsset,
            input: IntentInput::UnitId(unit.id.0.clone()),
        },
        &server.uri(),
        "legado_asset_delta",
    )
    .await;
    let asset_delta = common::collect_delta(&assets);
    assert!(
        asset_delta.assets.iter().any(|asset| matches!(
            &asset.locator,
            MediaAssetLocator::Text(text) if text.contains("正文内容")
        )),
        "ResolveAsset 应返回正文资产: {asset_delta:?}"
    );
}
