//! `SQLite` Repository 集成测试。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Once;

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Text;
use keyring::{mock, set_default_credential_builder};
use lj_capability::{IntentExport, StandardIntent};
use lj_media::{MediaItem, MediaKind, MediaResourceId, ResourceCompleteness};
use lj_rule_model::Error;
use lj_runtime::{Graph, SourceId};
use lj_storage::AsyncStorage;
use lj_storage::RepoId;
use lj_storage::repository::{CookieMap, SqliteStorage};
use lj_storage::{LibraryEntry, LibraryProgress};
use uuid::Uuid;

#[derive(diesel::QueryableByName)]
struct TextValueRow {
    #[diesel(sql_type = Text)]
    value: String,
}

fn init_mock_keyring() {
    static INIT: Once = Once::new();
    INIT.call_once(|| set_default_credential_builder(mock::default_credential_builder()));
}

fn make_test_graph() -> Graph {
    let flow_entry = Uuid::new_v4();
    let mapper_output = Uuid::new_v4();
    let mut intent_exports = HashMap::new();
    intent_exports.insert(
        StandardIntent::Search,
        IntentExport::new(flow_entry, mapper_output),
    );

    Graph {
        nodes: vec![],
        edges: vec![],
        subroutines: HashMap::new(),
        source_id: SourceId(Uuid::new_v4()),
        base_url: "https://example.com".to_string(),
        intent_exports,
    }
}

fn make_test_media(index: usize, source_id: &str) -> MediaItem {
    MediaItem {
        id: MediaResourceId(format!("item:test:{index}")),
        source_id: MediaResourceId(source_id.to_string()),
        media_kind: MediaKind::Text,
        title: format!("测试媒体 {index}"),
        subtitle: None,
        creators: Vec::new(),
        description: None,
        cover_asset_id: None,
        metadata: std::collections::BTreeMap::new(),
        completeness: ResourceCompleteness::Partial,
        updated_at: None,
    }
}

fn temp_db_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.db", Uuid::new_v4()))
}

fn open_file_connection(path: &Path) -> SqliteConnection {
    SqliteConnection::establish(path.to_string_lossy().as_ref()).expect("打开 SQLite 文件失败")
}

#[test]
fn test_graph_save_get_round_trip() {
    let storage = SqliteStorage::in_memory().unwrap();
    let graph = make_test_graph();
    let id = RepoId::<Graph>::new("test-graph".to_string());
    storage.save_graph(&id, &graph).unwrap();
    let loaded = storage.get_graph(&id).unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap(), graph);
}

#[test]
fn test_graph_get_nonexistent() {
    let storage = SqliteStorage::in_memory().unwrap();
    let id = RepoId::<Graph>::new("nonexistent".to_string());
    assert!(storage.get_graph(&id).unwrap().is_none());
}

#[test]
fn test_graph_delete_returns_none() {
    let storage = SqliteStorage::in_memory().unwrap();
    let graph = make_test_graph();
    let id = RepoId::<Graph>::new("test-graph".to_string());
    storage.save_graph(&id, &graph).unwrap();
    storage.delete_graph(&id).unwrap();
    assert!(storage.get_graph(&id).unwrap().is_none());
}

#[test]
fn test_graph_list_returns_saved() {
    let storage = SqliteStorage::in_memory().unwrap();
    let graph = make_test_graph();
    let id = RepoId::<Graph>::new("test-graph".to_string());
    storage.save_graph(&id, &graph).unwrap();
    let list = storage.list_graphs().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].0, id);
}

#[test]
fn test_list_graphs_page_pagination() {
    let storage = SqliteStorage::in_memory().unwrap();
    for i in 0..5 {
        let mut graph = make_test_graph();
        graph.source_id = SourceId(uuid::Uuid::new_v4());
        storage
            .save_graph(&RepoId::<Graph>::new(format!("g-{i}")), &graph)
            .unwrap();
    }

    let page0 = storage.list_graphs_page(2, 0).unwrap();
    assert_eq!(page0.len(), 2);

    let page1 = storage.list_graphs_page(2, 2).unwrap();
    assert_eq!(page1.len(), 2);

    let page2 = storage.list_graphs_page(10, 0).unwrap();
    assert_eq!(page2.len(), 5);

    let empty = storage.list_graphs_page(2, 10).unwrap();
    assert!(empty.is_empty());
}

#[test]
fn test_list_media_page_pagination() {
    let storage = SqliteStorage::in_memory().unwrap();
    for i in 0..3 {
        let media = make_test_media(i, "source:test");
        storage
            .save_media(&RepoId::<MediaItem>::new(format!("m-{i}")), &media)
            .unwrap();
    }

    let page = storage.list_media_page(2, 0).unwrap();
    assert_eq!(page.len(), 2);

    let rest = storage.list_media_page(2, 2).unwrap();
    assert_eq!(rest.len(), 1);
}

#[test]
fn test_list_media_by_source() {
    let storage = SqliteStorage::in_memory().unwrap();

    for i in 0..3 {
        let media = make_test_media(i, "source:shared");
        storage
            .save_media(&RepoId::<MediaItem>::new(format!("src-m-{i}")), &media)
            .unwrap();
    }
    let other_media = make_test_media(99, "source:other");
    storage
        .save_media(
            &RepoId::<MediaItem>::new("other-media".to_string()),
            &other_media,
        )
        .unwrap();

    let result = storage
        .list_media_by_source("source:shared", 10, 0)
        .unwrap();
    assert_eq!(result.len(), 3);

    let paged = storage.list_media_by_source("source:shared", 2, 0).unwrap();
    assert_eq!(paged.len(), 2);
}

#[test]
fn test_list_cookies_page_pagination() {
    init_mock_keyring();
    let storage = SqliteStorage::in_memory().unwrap();
    for i in 0..4 {
        let cm = CookieMap(HashMap::from([(format!("k{i}"), format!("v{i}"))]));
        storage
            .save_cookie(&RepoId::<CookieMap>::new(format!("c-{i}")), &cm)
            .unwrap();
    }

    let page = storage.list_cookies_page(2, 0).unwrap();
    assert_eq!(page.len(), 2);

    let rest = storage.list_cookies_page(2, 2).unwrap();
    assert_eq!(rest.len(), 2);

    let empty = storage.list_cookies_page(10, 10).unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn test_async_storage_graph_round_trip() {
    let inner = SqliteStorage::in_memory().unwrap();
    let storage = AsyncStorage::new(inner);
    let graph = make_test_graph();
    let id = RepoId::<Graph>::new("async-graph".to_string());

    storage.save_graph(&id, &graph).await.unwrap();
    let loaded = storage.get_graph(&id).await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap(), graph);

    storage.delete_graph(&id).await.unwrap();
    let deleted = storage.get_graph(&id).await.unwrap();
    assert!(deleted.is_none());
}

#[tokio::test]
async fn test_async_storage_list_graphs_page() {
    let inner = SqliteStorage::in_memory().unwrap();
    let storage = AsyncStorage::new(inner);

    for i in 0..3 {
        let mut graph = make_test_graph();
        graph.source_id = SourceId(uuid::Uuid::new_v4());
        let id = RepoId::<Graph>::new(format!("async-g-{i}"));
        storage.save_graph(&id, &graph).await.unwrap();
    }

    let page = storage.list_graphs_page(2, 0).await.unwrap();
    assert_eq!(page.len(), 2);

    let page2 = storage.list_graphs_page(10, 0).await.unwrap();
    assert_eq!(page2.len(), 3);
}

#[test]
fn test_graph_list_empty() {
    let storage = SqliteStorage::in_memory().unwrap();
    let list: Vec<(RepoId<Graph>, Graph)> = storage.list_graphs().unwrap();
    assert!(list.is_empty());
}

#[test]
fn test_media_save_get_round_trip() {
    let storage = SqliteStorage::in_memory().unwrap();
    let media = make_test_media(1, "source:test");
    let id = RepoId::<MediaItem>::new("test-media".to_string());
    storage.save_media(&id, &media.clone()).unwrap();
    let loaded = storage.get_media(&id).unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap(), media);
}

#[test]
fn test_media_delete() {
    let storage = SqliteStorage::in_memory().unwrap();
    let media = make_test_media(1, "source:test");
    let id = RepoId::<MediaItem>::new("test-media".to_string());
    storage.save_media(&id, &media).unwrap();
    storage.delete_media(&id).unwrap();
    assert!(storage.get_media(&id).unwrap().is_none());
}

#[test]
fn test_media_list() {
    let storage = SqliteStorage::in_memory().unwrap();
    let media = make_test_media(1, "source:test");
    let id = RepoId::<MediaItem>::new("test-media".to_string());
    storage.save_media(&id, &media).unwrap();
    let list = storage.list_media().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].0, id);
}

#[test]
fn test_media_graph_delta_merge_and_library_projection_share_identity() {
    let storage = SqliteStorage::in_memory().unwrap();
    let item = make_test_media(1, "source:test");
    let graph = lj_media::MediaGraphDelta {
        items: vec![item.clone()],
        ..Default::default()
    };
    storage.merge_media_graph_delta(graph).unwrap();
    storage
        .set_library_entry(&LibraryEntry {
            resource_id: item.id.clone(),
            favorite: true,
            pinned: false,
            last_opened_at: Some("2026-07-15T10:00:00Z".to_string()),
            progress: Some(LibraryProgress {
                unit_id: None,
                position: 12,
                total: Some(100),
            }),
        })
        .unwrap();

    let projection = storage.library_projection().unwrap();
    assert_eq!(projection.graph.items, vec![item.clone()]);
    assert_eq!(projection.entries.len(), 1);
    assert_eq!(projection.entries[0].resource_id, item.id);
    assert!(projection.entries[0].favorite);
    assert_eq!(
        projection.entries[0].progress.as_ref().unwrap().position,
        12
    );
}

#[test]
fn test_media_graph_incremental_merge_deduplicates_by_stable_id() {
    let storage = SqliteStorage::in_memory().unwrap();
    let first = make_test_media(1, "source:test");
    let mut update = first.clone();
    update.title = "补全标题".to_string();
    update.completeness = ResourceCompleteness::Complete;

    storage
        .merge_media_graph_delta(lj_media::MediaGraphDelta {
            items: vec![first],
            ..Default::default()
        })
        .unwrap();
    storage
        .merge_media_graph_delta(lj_media::MediaGraphDelta {
            items: vec![update.clone()],
            ..Default::default()
        })
        .unwrap();

    let projection = storage.library_projection().unwrap();
    assert_eq!(projection.graph.items, vec![update]);
}

#[test]
fn test_library_entry_without_graph_resource_is_rejected() {
    let storage = SqliteStorage::in_memory().unwrap();
    let error = storage
        .set_library_entry(&LibraryEntry::new(MediaResourceId(
            "item:missing".to_string(),
        )))
        .expect_err("资料库状态不能脱离标准资源图存在");
    assert!(matches!(error, Error::Storage(message) if message.contains("标准媒体资源不存在")));
}

#[test]
fn test_cookie_save_get_round_trip() {
    init_mock_keyring();
    let storage = SqliteStorage::in_memory().unwrap();
    let mut cookies = HashMap::new();
    cookies.insert("session".to_string(), "abc123".to_string());
    cookies.insert("token".to_string(), "xyz".to_string());
    let cookie_map = CookieMap(cookies);
    let id = RepoId::<CookieMap>::new("test-cookie".to_string());
    storage.save_cookie(&id, &cookie_map).unwrap();
    let loaded = storage.get_cookie(&id).unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap(), cookie_map);
}

#[test]
fn test_cookie_delete() {
    init_mock_keyring();
    let storage = SqliteStorage::in_memory().unwrap();
    let cookie_map = CookieMap(HashMap::new());
    let id = RepoId::<CookieMap>::new("test-cookie".to_string());
    storage.save_cookie(&id, &cookie_map).unwrap();
    storage.delete_cookie(&id).unwrap();
    assert!(storage.get_cookie(&id).unwrap().is_none());
}

#[test]
fn test_cookie_list() {
    init_mock_keyring();
    let storage = SqliteStorage::in_memory().unwrap();
    let cookie_map = CookieMap(HashMap::new());
    let id = RepoId::<CookieMap>::new("test-cookie".to_string());
    storage.save_cookie(&id, &cookie_map).unwrap();
    let list = storage.list_cookies().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].0, id);
}

#[test]
fn test_new_database_runs_embedded_migrations() {
    let db_path = temp_db_path("lj-storage-migration");
    SqliteStorage::new(&db_path).unwrap();

    let mut conn = open_file_connection(&db_path);
    let rows =
        sql_query("SELECT name AS value FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .load::<TextValueRow>(&mut conn)
            .unwrap();
    let table_names: Vec<String> = rows.into_iter().map(|row| row.value).collect();

    assert!(table_names.contains(&"rules".to_string()));
    assert!(table_names.contains(&"media".to_string()));
    assert!(table_names.contains(&"cookies".to_string()));
    assert!(table_names.contains(&"__diesel_schema_migrations".to_string()));
    drop(conn);

    std::fs::remove_file(&db_path).unwrap();
}

#[test]
fn test_cookie_raw_storage_uses_keyring_marker() {
    init_mock_keyring();
    let db_path = temp_db_path("lj-storage-cookie");
    {
        let storage = SqliteStorage::new(&db_path).unwrap();
        let cookie_map = CookieMap(HashMap::from([(
            "session".to_string(),
            "abc123".to_string(),
        )]));
        let id = RepoId::<CookieMap>::new("test-cookie-raw".to_string());

        storage.save_cookie(&id, &cookie_map).unwrap();

        let mut conn = open_file_connection(&db_path);
        let stored = sql_query("SELECT cookie_json AS value FROM cookies WHERE id = ?")
            .bind::<Text, _>(&id.id)
            .get_result::<TextValueRow>(&mut conn)
            .unwrap();
        assert_eq!(stored.value, "keyring:v1");
        assert!(!stored.value.contains("abc123"));
    }

    std::fs::remove_file(&db_path).unwrap();
}

#[test]
fn test_cookie_missing_keyring_secret_returns_storage_error() {
    init_mock_keyring();
    let db_path = temp_db_path("lj-storage-cookie-missing-secret");
    let id = RepoId::<CookieMap>::new("missing-secret".to_string());
    SqliteStorage::new(&db_path).unwrap();

    let mut conn = open_file_connection(&db_path);
    sql_query("INSERT INTO cookies (id, domain, cookie_json) VALUES (?, '', 'keyring:v1')")
        .bind::<Text, _>(&id.id)
        .execute(&mut conn)
        .unwrap();
    drop(conn);

    let storage = SqliteStorage::new(&db_path).unwrap();
    let error = storage
        .get_cookie(&id)
        .expect_err("缺失 keyring secret 应返回错误");
    assert!(
        matches!(error, Error::Storage(message) if message.contains("读取 Cookie keyring 失败"))
    );
    drop(storage);

    std::fs::remove_file(&db_path).unwrap();
}
