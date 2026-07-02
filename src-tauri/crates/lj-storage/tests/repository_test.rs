//! `SQLite` Repository 集成测试。

use std::collections::HashMap;

use lj_core::media::{BookMedia, Media};
use lj_core::node::{Graph, SourceId};
use lj_core::traits::{RepoId, Repository};
use lj_storage::AsyncStorage;
use lj_storage::repository::{CookieMap, SqliteStorage};
use uuid::Uuid;

fn make_test_graph() -> Graph {
    Graph {
        nodes: vec![],
        edges: vec![],
        subroutines: HashMap::new(),
        source_id: SourceId(Uuid::new_v4()),
        base_url: String::new(),
    }
}

#[test]
fn test_graph_save_get_round_trip() {
    let storage = SqliteStorage::in_memory().unwrap();
    let graph = make_test_graph();
    let id = RepoId::<Graph>::new("test-graph".to_string());
    storage.save(&id, &graph).unwrap();
    let loaded = storage.get(&id).unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap(), graph);
}

#[test]
fn test_graph_get_nonexistent() {
    let storage = SqliteStorage::in_memory().unwrap();
    let id = RepoId::<Graph>::new("nonexistent".to_string());
    assert!(storage.get(&id).unwrap().is_none());
}

#[test]
fn test_graph_delete_returns_none() {
    let storage = SqliteStorage::in_memory().unwrap();
    let graph = make_test_graph();
    let id = RepoId::<Graph>::new("test-graph".to_string());
    storage.save(&id, &graph).unwrap();
    storage.delete(&id).unwrap();
    assert!(storage.get(&id).unwrap().is_none());
}

#[test]
fn test_graph_list_returns_saved() {
    let storage = SqliteStorage::in_memory().unwrap();
    let graph = make_test_graph();
    let id = RepoId::<Graph>::new("test-graph".to_string());
    storage.save(&id, &graph).unwrap();
    let list = storage.list().unwrap();
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
            .save(&RepoId::<Graph>::new(format!("g-{i}")), &graph)
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
        let media = Media::Book(BookMedia {
            title: format!("book {i}"),
            author: None,
            cover_url: None,
            description: None,
            kind: None,
            last_chapter: None,
            book_url: None,
            chapters: vec![],
        });
        storage
            .save(&RepoId::<Media>::new(format!("m-{i}")), &media)
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

    // 插入两条相同 source 的 media
    for i in 0..3 {
        let media = Media::Book(BookMedia {
            title: format!("src-book {i}"),
            author: None,
            cover_url: None,
            description: None,
            kind: None,
            last_chapter: None,
            book_url: None,
            chapters: vec![],
        });
        storage
            .save(&RepoId::<Media>::new(format!("src-m-{i}")), &media)
            .unwrap();
    }

    // source_id 当前保存为空字符串
    let result = storage.list_media_by_source("", 10, 0).unwrap();
    assert_eq!(result.len(), 3);

    // 分页
    let paged = storage.list_media_by_source("", 2, 0).unwrap();
    assert_eq!(paged.len(), 2);
}

#[test]
fn test_list_cookies_page_pagination() {
    let storage = SqliteStorage::in_memory().unwrap();
    for i in 0..4 {
        let cm = CookieMap(HashMap::from([(format!("k{i}"), format!("v{i}"))]));
        storage
            .save(&RepoId::<CookieMap>::new(format!("c-{i}")), &cm)
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
    let list: Vec<(RepoId<Graph>, Graph)> = storage.list().unwrap();
    assert!(list.is_empty());
}

#[test]
fn test_media_save_get_round_trip() {
    let storage = SqliteStorage::in_memory().unwrap();
    let media = Media::Book(BookMedia {
        title: "测试图书".to_string(),
        author: Some("作者".to_string()),
        cover_url: None,
        description: None,
        kind: None,
        last_chapter: None,
        book_url: None,
        chapters: vec![],
    });
    let id = RepoId::<Media>::new("test-media".to_string());
    storage.save(&id, &media.clone()).unwrap();
    let loaded = storage.get(&id).unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap(), media);
}

#[test]
fn test_media_delete() {
    let storage = SqliteStorage::in_memory().unwrap();
    let media = Media::Book(BookMedia {
        title: "test".to_string(),
        author: None,
        cover_url: None,
        description: None,
        kind: None,
        last_chapter: None,
        book_url: None,
        chapters: vec![],
    });
    let id = RepoId::<Media>::new("test-media".to_string());
    storage.save(&id, &media).unwrap();
    storage.delete(&id).unwrap();
    assert!(storage.get(&id).unwrap().is_none());
}

#[test]
fn test_media_list() {
    let storage = SqliteStorage::in_memory().unwrap();
    let media = Media::Book(BookMedia {
        title: "test".to_string(),
        author: None,
        cover_url: None,
        description: None,
        kind: None,
        last_chapter: None,
        book_url: None,
        chapters: vec![],
    });
    let id = RepoId::<Media>::new("test-media".to_string());
    storage.save(&id, &media).unwrap();
    let list = storage.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].0, id);
}

#[test]
fn test_cookie_save_get_round_trip() {
    let storage = SqliteStorage::in_memory().unwrap();
    let mut cookies = HashMap::new();
    cookies.insert("session".to_string(), "abc123".to_string());
    cookies.insert("token".to_string(), "xyz".to_string());
    let cookie_map = CookieMap(cookies);
    let id = RepoId::<CookieMap>::new("test-cookie".to_string());
    storage.save(&id, &cookie_map).unwrap();
    let loaded = storage.get(&id).unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap(), cookie_map);
}

#[test]
fn test_cookie_delete() {
    let storage = SqliteStorage::in_memory().unwrap();
    let cookie_map = CookieMap(HashMap::new());
    let id = RepoId::<CookieMap>::new("test-cookie".to_string());
    storage.save(&id, &cookie_map).unwrap();
    storage.delete(&id).unwrap();
    assert!(storage.get(&id).unwrap().is_none());
}

#[test]
fn test_cookie_list() {
    let storage = SqliteStorage::in_memory().unwrap();
    let cookie_map = CookieMap(HashMap::new());
    let id = RepoId::<CookieMap>::new("test-cookie".to_string());
    storage.save(&id, &cookie_map).unwrap();
    let list = storage.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].0, id);
}
