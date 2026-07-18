//! Maccms JSON 本地 fixture 集成测试。
//!
//! 使用本地 mock server 验证导入、执行和标准媒体资源图增量闭环。

mod common;

use lj_capability::{IntentInput, StandardIntent};
use lj_importer::maccms::{MaccmsFormat, MaccmsImporter, MaccmsSourceUrl};
use lj_media::{MediaAssetKind, MediaAssetLocator, MediaKind};
use lj_runtime::Graph;
use lj_runtime::SegmentSpec;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn import_source(base_url: &str) -> (Graph, String) {
    let source_url = format!("{base_url}/api.php/provide/vod/");
    let preview = MaccmsImporter
        .import(MaccmsSourceUrl {
            url: source_url,
            at: MaccmsFormat::Json,
        })
        .expect("Maccms JSON 导入失败");
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
            "Maccms JSON 应声明标准意图 {intent:?}"
        );
    }
}

async fn mount_maccms_json_routes(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api.php/provide/vod/"))
        .and(query_param("ac", "list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": 1,
            "page": 1,
            "pagecount": 1,
            "limit": 20,
            "total": 1,
            "list": [
                {
                    "vod_id": 140_789,
                    "vod_name": "爱情没有神话",
                    "vod_pic": "/covers/140789.jpg",
                    "type_name": "国产剧",
                    "vod_remarks": "第02集"
                }
            ]
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api.php/provide/vod/"))
        .and(query_param("ac", "detail"))
        .and(query_param("ids", "140789"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": 1,
            "list": [
                {
                    "vod_id": 140_789,
                    "vod_name": "爱情没有神话",
                    "vod_pic": "/covers/140789.jpg",
                    "type_name": "国产剧",
                    "vod_remarks": "第02集",
                    "vod_content": "一段本地 mock 的剧情简介。",
                    "vod_play_from": "hnyun,hnm3u8",
                    "vod_play_url": "第1集$mock-stream://json/1.m3u8#第2集$mock-stream://json/2.m3u8###正片$mock-stream://json/main.mp4"
                }
            ]
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn maccms_json_fixture_executes_to_media_graph_delta() {
    common::init_tracing();
    let server = MockServer::start().await;
    mount_maccms_json_routes(&server).await;
    let (graph, base_url) = import_source(&server.uri());
    assert_standard_intents(&graph);

    let discover = common::execute_and_collect(
        &graph,
        SegmentSpec {
            intent: StandardIntent::Discover,
            input: IntentInput::None,
        },
        &base_url,
        "maccms_json_discover_delta",
    )
    .await;
    let discover_delta = common::collect_delta(&discover);
    let item = discover_delta
        .items
        .iter()
        .find(|item| item.title == "爱情没有神话")
        .expect("Discover 应返回视频媒体主体");
    assert_eq!(item.media_kind, MediaKind::Video);
    assert_eq!(item.subtitle.as_deref(), Some("第02集"));
    assert_eq!(item.metadata["source_item_id"], "140789");
    assert!(
        discover_delta
            .collections
            .iter()
            .any(|collection| collection.item_ids.contains(&item.id)),
        "Discover 应把媒体主体挂入发现集合"
    );

    let item_id = item.id.0.clone();
    let detail = common::execute_and_collect(
        &graph,
        SegmentSpec {
            intent: StandardIntent::ResolveItem,
            input: IntentInput::ItemId(item_id.clone()),
        },
        &base_url,
        "maccms_json_detail_delta",
    )
    .await;
    let detail_delta = common::collect_delta(&detail);
    assert!(
        detail_delta.items.iter().any(|item| {
            item.title == "爱情没有神话"
                && item.description.as_deref() == Some("一段本地 mock 的剧情简介。")
        }),
        "ResolveItem 应返回完整媒体主体: {detail_delta:?}"
    );
    assert!(
        detail_delta.assets.iter().any(|asset| {
            asset.asset_kind == MediaAssetKind::VideoStream
                && matches!(&asset.locator, MediaAssetLocator::Url(url) if url.ends_with("/json/1.m3u8"))
        }),
        "ResolveItem 应从播放线路生成视频资产: {detail_delta:?}"
    );

    let units = common::execute_and_collect(
        &graph,
        SegmentSpec {
            intent: StandardIntent::ListUnits,
            input: IntentInput::ItemId(item_id.clone()),
        },
        &base_url,
        "maccms_json_units_delta",
    )
    .await;
    let units_delta = common::collect_delta(&units);
    let first_unit = units_delta
        .units
        .iter()
        .find(|unit| unit.title == "第1集")
        .expect("ListUnits 应返回播放单元");
    assert_eq!(first_unit.metadata["line"], "hnyun");
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
        "maccms_json_asset_delta",
    )
    .await;
    let asset_delta = common::collect_delta(&assets);
    assert!(
        asset_delta.assets.iter().any(|asset| {
            asset.asset_kind == MediaAssetKind::VideoStream
                && asset.unit_id.as_ref().map(|id| id.0.as_str()) == Some(first_unit.id.0.as_str())
                && matches!(&asset.locator, MediaAssetLocator::Url(url) if url.ends_with("/json/1.m3u8"))
        }),
        "ResolveAsset 应返回与选中单元绑定的播放流资产: {asset_delta:?}"
    );
    assert!(
        asset_delta.units.is_empty(),
        "ResolveAsset 只返回资产，不返回单元"
    );
}
