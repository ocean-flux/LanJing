//! 标准媒体投影有界查询：批量点查、分页、排序与 tombstone 后缺失语义。

use super::*;

fn multi_unit_delta(source_id: &str) -> ProjectionDelta {
    let media = item(source_id, "有界目录书");
    let units = vec![
        MediaUnit {
            id: MediaResourceId("unit:test:c".to_string()),
            source_id: MediaResourceId(source_id.to_string()),
            item_id: media.id.clone(),
            title: "第三章".to_string(),
            position: Some(3),
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Complete,
        },
        MediaUnit {
            id: MediaResourceId("unit:test:a".to_string()),
            source_id: MediaResourceId(source_id.to_string()),
            item_id: media.id.clone(),
            title: "第一章".to_string(),
            position: Some(1),
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Complete,
        },
        MediaUnit {
            id: MediaResourceId("unit:test:null".to_string()),
            source_id: MediaResourceId(source_id.to_string()),
            item_id: media.id.clone(),
            title: "无序单元".to_string(),
            position: None,
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Complete,
        },
        MediaUnit {
            id: MediaResourceId("unit:test:b".to_string()),
            source_id: MediaResourceId(source_id.to_string()),
            item_id: media.id.clone(),
            title: "第二章".to_string(),
            position: Some(2),
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Complete,
        },
    ];
    let assets = vec![
        MediaAsset {
            id: MediaResourceId("asset:test:z".to_string()),
            source_id: MediaResourceId(source_id.to_string()),
            unit_id: Some(MediaResourceId("unit:test:a".to_string())),
            asset_kind: MediaAssetKind::Text,
            locator: MediaAssetLocator::Text("后插资产".to_string()),
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Complete,
        },
        MediaAsset {
            id: MediaResourceId("asset:test:a".to_string()),
            source_id: MediaResourceId(source_id.to_string()),
            unit_id: Some(MediaResourceId("unit:test:a".to_string())),
            asset_kind: MediaAssetKind::Text,
            locator: MediaAssetLocator::Text("先插资产".to_string()),
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Complete,
        },
    ];
    ProjectionDelta {
        upserts: MediaGraphDelta {
            items: vec![media],
            units,
            assets,
            ..MediaGraphDelta::default()
        },
        tombstones: ProjectionTombstones::default(),
    }
}

async fn commit_delta(
    storage: &EventProjectionStorage,
    execution_id: Uuid,
    expected_version: u64,
    delta: ProjectionDelta,
    now: i64,
) {
    storage
        .commit_execution_delta(DeltaCommit {
            execution_id,
            expected_version,
            event_id: Uuid::new_v4(),
            trace_id: format!("trace-media-query-{expected_version}"),
            occurred_at_ms: now,
            delta,
        })
        .await
        .expect("commit media query projection");
}

#[tokio::test]
async fn bounded_media_queries_order_page_and_skip_missing() {
    let temp = TempStore::new("media-query-bounded");
    let storage = temp.open().await;
    let now = 1_750_001_000_000;
    install_source(&storage, now).await;

    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:test".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-media-query-start".to_string(),
            started_at_ms: now + 1,
            correlation_id: None,
        })
        .await
        .expect("start media query execution");

    commit_delta(
        &storage,
        execution_id,
        1,
        multi_unit_delta("source:test"),
        now + 2,
    )
    .await;

    let item_id = MediaResourceId("item:test:1".to_string());
    let missing_id = MediaResourceId("item:missing".to_string());

    assert!(
        storage
            .get_item(missing_id.clone())
            .await
            .expect("missing item query")
            .is_none(),
        "缺失资源必须返回 None"
    );

    let batch = storage
        .get_items(vec![
            MediaResourceId("item:z-later".to_string()),
            item_id.clone(),
            missing_id,
            MediaResourceId("item:a-earlier".to_string()),
        ])
        .await
        .expect("batch get items");
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].id, item_id);
    assert_eq!(batch[0].title, "有界目录书");

    let units_page = storage
        .list_units_for_item(item_id.clone(), 0, 2)
        .await
        .expect("units page 0");
    assert_eq!(
        units_page
            .iter()
            .map(|unit| unit.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["unit:test:a", "unit:test:b"]
    );

    let units_page_1 = storage
        .list_units_for_item(item_id.clone(), 2, 2)
        .await
        .expect("units page 1");
    assert_eq!(
        units_page_1
            .iter()
            .map(|unit| unit.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["unit:test:c", "unit:test:null"]
    );

    let empty_units = storage
        .list_units_for_item(MediaResourceId("item:ghost".to_string()), 0, 50)
        .await
        .expect("missing parent units");
    assert!(empty_units.is_empty());

    let assets = storage
        .list_assets_for_unit(MediaResourceId("unit:test:a".to_string()), 0, 50)
        .await
        .expect("assets ordered by id");
    assert_eq!(
        assets
            .iter()
            .map(|asset| asset.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["asset:test:a", "asset:test:z"]
    );

    let assets_limit = storage
        .list_assets_for_unit(MediaResourceId("unit:test:a".to_string()), 0, 1)
        .await
        .expect("assets limit");
    assert_eq!(assets_limit.len(), 1);
    assert_eq!(assets_limit[0].id.0, "asset:test:a");

    storage
        .commit_execution_delta(DeltaCommit {
            execution_id,
            expected_version: 2,
            event_id: Uuid::new_v4(),
            trace_id: "trace-media-query-tombstone".to_string(),
            occurred_at_ms: now + 3,
            delta: ProjectionDelta {
                upserts: MediaGraphDelta::default(),
                tombstones: ProjectionTombstones {
                    items: vec![item_id.clone()],
                    units: vec![
                        MediaResourceId("unit:test:a".to_string()),
                        MediaResourceId("unit:test:b".to_string()),
                        MediaResourceId("unit:test:c".to_string()),
                        MediaResourceId("unit:test:null".to_string()),
                    ],
                    assets: vec![
                        MediaResourceId("asset:test:a".to_string()),
                        MediaResourceId("asset:test:z".to_string()),
                    ],
                    ..ProjectionTombstones::default()
                },
            },
        })
        .await
        .expect("tombstone media resources");

    assert!(
        storage
            .get_item(item_id.clone())
            .await
            .expect("item after tombstone")
            .is_none()
    );
    assert!(
        storage
            .list_units_for_item(item_id, 0, 50)
            .await
            .expect("units after tombstone")
            .is_empty()
    );
    assert!(
        storage
            .list_assets_for_unit(MediaResourceId("unit:test:a".to_string()), 0, 50)
            .await
            .expect("assets after tombstone")
            .is_empty()
    );

    storage.shutdown().await.expect("writer shutdown");
}
