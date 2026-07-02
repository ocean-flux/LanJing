//! async 封装层 — 把同步 Repository 调用包到 `spawn_blocking`。
//!
//! 避免 `SQLite` `Mutex<Connection>` 同步 API 阻塞 tokio worker 线程。
//! 首刀只实现 Graph 类型的 async 方法（实际使用类型），其他类型按需添加。

use std::sync::Arc;

use lj_core::error::CoreError;
use lj_core::node::Graph;
use lj_core::traits::{RepoId, Repository};

use crate::repository::SqliteStorage;

/// 异步存储封装。
///
/// 内部持有 `Arc<SqliteStorage>`，所有公开方法通过 `tokio::task::spawn_blocking`
/// 将同步 Repository 调用派发到阻塞线程池。
pub struct AsyncStorage {
    inner: Arc<SqliteStorage>,
}

impl AsyncStorage {
    /// 创建新的 `AsyncStorage` 实例。
    #[must_use]
    pub fn new(storage: SqliteStorage) -> Self {
        Self {
            inner: Arc::new(storage),
        }
    }

    /// 异步获取 Graph。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库查询失败或 `spawn_blocking` panic。
    pub async fn get_graph(&self, id: &RepoId<Graph>) -> Result<Option<Graph>, CoreError> {
        let inner = self.inner.clone();
        let id = id.clone();
        tokio::task::spawn_blocking(move || inner.get(&id))
            .await
            .map_err(|e| CoreError::Storage(format!("spawn_blocking panic: {e}")))?
    }

    /// 异步保存 Graph。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库写入失败或 `spawn_blocking` panic。
    pub async fn save_graph(&self, id: &RepoId<Graph>, value: &Graph) -> Result<(), CoreError> {
        let inner = self.inner.clone();
        let id = id.clone();
        let value = value.clone();
        tokio::task::spawn_blocking(move || inner.save(&id, &value))
            .await
            .map_err(|e| CoreError::Storage(format!("spawn_blocking panic: {e}")))?
    }

    /// 异步删除 Graph。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库删除失败或 `spawn_blocking` panic。
    pub async fn delete_graph(&self, id: &RepoId<Graph>) -> Result<(), CoreError> {
        let inner = self.inner.clone();
        let id = id.clone();
        tokio::task::spawn_blocking(move || inner.delete(&id))
            .await
            .map_err(|e| CoreError::Storage(format!("spawn_blocking panic: {e}")))?
    }

    /// 异步分页列出 Graph。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库查询失败或 `spawn_blocking` panic。
    pub async fn list_graphs_page(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(RepoId<Graph>, Graph)>, CoreError> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || inner.list_graphs_page(limit, offset))
            .await
            .map_err(|e| CoreError::Storage(format!("spawn_blocking panic: {e}")))?
    }
}
