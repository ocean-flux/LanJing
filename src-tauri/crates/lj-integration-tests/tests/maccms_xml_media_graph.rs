//! Maccms XML 本地 fixture 集成测试。
//!
//! 使用本地 mock server 验证 XML 采集响应能映射为标准媒体资源图增量。

mod common;

use lj_capability::{IntentInput, StandardIntent};
use lj_core::media::{MediaAssetKind, MediaAssetLocator, MediaKind};
use lj_core::node::Graph;
use lj_core::traits::{Importer, SegmentSpec};
use lj_importer::maccms::{MaccmsFormat, MaccmsImporter, MaccmsSourceUrl};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn import_source(base_url: &str) -> (Graph, String) {
    let source_url = format!("{base_url}/api.php/provide/vod/at/xml/");
    let preview = MaccmsImporter
        .import(MaccmsSourceUrl {
            url: source_url,
            at: MaccmsFormat::Xml,
        })
        .expect("Maccms XML 导入失败");
    let base_url = preview.graph.base_url.clone();
    (preview.graph, base_url)
}

fn assert_standard_intents(graph: &Graph) {
    for intent in [
        StandardIntent::Discover,
        StandardIntent::ResolveItem,
        StandardIntent::ListUnits,
        StandardIntent::ResolveAsset,
    ] {
        assert!(
            graph.intent_exports.contains_key(&intent),
            "Maccms XML 应声明标准意图 {intent:?}"
        );
    }
}

async fn mount_maccms_xml_routes(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api.php/provide/vod/at/xml/"))
        .and(query_param("ac", "list"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<?xml version="1.0" encoding="utf-8"?>
            <rss version="5.1">
              <list page="1" pagecount="1" pagesize="20" recordcount="1">
                <video>
                  <id>240001</id>
                  <name>本地 XML 影片</name>
                  <pic>/covers/xml-240001.jpg</pic>
                  <type>纪录片</type>
                  <note>完结</note>
                </video>
              </list>
            </rss>"#,
        ))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api.php/provide/vod/at/xml/"))
        .and(query_param("ac", "detail"))
        .and(query_param("ids", "240001"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<?xml version="1.0" encoding="utf-8"?>
            <rss version="5.1">
              <list page="1" pagecount="1" pagesize="20" recordcount="1">
                <video>
                  <id>240001</id>
                  <name>本地 XML 影片</name>
                  <pic>/covers/xml-240001.jpg</pic>
                  <type>纪录片</type>
                  <note>完结</note>
                  <des>XML mock 简介。</des>
                  <dl>
                    <dd flag="xml-line">上$mock-stream://xml/a.m3u8#下$mock-stream://xml/b.m3u8</dd>
                    <dd flag="backup">正片$mock-stream://xml/main.mp4</dd>
                  </dl>
                </video>
              </list>
            </rss>"#,
        ))
        .mount(server)
        .await;
}

#[tokio::test]
async fn maccms_xml_fixture_executes_to_media_graph_delta() {
    common::init_tracing();
    let server = MockServer::start().await;
    mount_maccms_xml_routes(&server).await;
    let (graph, base_url) = import_source(&server.uri());
    assert_standard_intents(&graph);

    let discover = common::execute_and_collect(
        &graph,
        SegmentSpec {
            intent: StandardIntent::Discover,
            input: IntentInput::None,
        },
        &base_url,
        "maccms_xml_discover_delta",
    )
    .await;
    let discover_delta = common::collect_delta(&discover);
    let item = discover_delta
        .items
        .iter()
        .find(|item| item.title == "本地 XML 影片")
        .expect("Discover 应返回 XML 视频媒体主体");
    assert_eq!(item.media_kind, MediaKind::Video);
    assert_eq!(item.subtitle.as_deref(), Some("完结"));
    assert_eq!(item.metadata["source_item_id"], "240001");

    let item_id = item.id.0.clone();
    let detail = common::execute_and_collect(
        &graph,
        SegmentSpec {
            intent: StandardIntent::ResolveItem,
            input: IntentInput::ItemId(item_id.clone()),
        },
        &base_url,
        "maccms_xml_detail_delta",
    )
    .await;
    let detail_delta = common::collect_delta(&detail);
    assert!(
        detail_delta.items.iter().any(|item| {
            item.title == "本地 XML 影片" && item.description.as_deref() == Some("XML mock 简介。")
        }),
        "ResolveItem 应返回 XML 媒体详情: {detail_delta:?}"
    );
    assert!(
        detail_delta.assets.iter().any(|asset| {
            asset.asset_kind == MediaAssetKind::VideoStream
                && matches!(&asset.locator, MediaAssetLocator::Url(url) if url.ends_with("/xml/a.m3u8"))
        }),
        "ResolveItem 应从 XML 播放线路生成视频资产: {detail_delta:?}"
    );

    let units = common::execute_and_collect(
        &graph,
        SegmentSpec {
            intent: StandardIntent::ListUnits,
            input: IntentInput::ItemId(item_id.clone()),
        },
        &base_url,
        "maccms_xml_units_delta",
    )
    .await;
    let units_delta = common::collect_delta(&units);
    let first_unit = units_delta
        .units
        .iter()
        .find(|unit| unit.title == "上")
        .expect("ListUnits 应返回 XML 播放单元");
    assert_eq!(first_unit.metadata["line"], "xml-line");
    assert!(
        units_delta.assets.is_empty(),
        "ListUnits 只返回单元，不返回资产"
    );

    let assets = common::execute_and_collect(
        &graph,
        SegmentSpec {
            intent: StandardIntent::ResolveAsset,
            input: IntentInput::UnitId(first_unit.id.0.clone()),
        },
        &base_url,
        "maccms_xml_asset_delta",
    )
    .await;
    let asset_delta = common::collect_delta(&assets);
    assert!(
        asset_delta.assets.iter().any(|asset| {
            asset.asset_kind == MediaAssetKind::VideoStream
                && asset.unit_id.as_ref().map(|id| id.0.as_str()) == Some(first_unit.id.0.as_str())
                && matches!(&asset.locator, MediaAssetLocator::Url(url) if url.ends_with("/xml/a.m3u8"))
        }),
        "ResolveAsset 应返回与选中 XML 单元绑定的播放流资产: {asset_delta:?}"
    );
    assert!(
        asset_delta.units.is_empty(),
        "ResolveAsset 只返回资产，不返回单元"
    );
}
