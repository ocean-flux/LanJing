//! library 与标准媒体投影 façade。

use lj_media::{MediaAsset, MediaItem, MediaResourceId, MediaUnit};

use super::EventProjectionStorage;
use crate::repository::projection::{
    get_items_by_ids, get_library_entry_sync, get_payload_by_id, library_projection_sync,
    list_assets_for_unit_bounded, list_units_for_item_bounded, payloads_by_column,
    source_projection_sync,
};
use crate::types::{
    CommitReceipt, LibraryEntry, LibraryProjection, LibraryUpdate, SourceProjectionView,
    StorageError,
};
use crate::writer::WriterCommand;

impl EventProjectionStorage {
    /// 原子更新 library 用户状态并追加 event。
    ///
    /// # Errors
    ///
    /// version、transaction 或 writer 失败时返回 `StorageError`。
    pub async fn update_library(
        &self,
        request: LibraryUpdate,
    ) -> Result<CommitReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::UpdateLibrary { request, reply })
            .await
    }

    /// 在单个只读 transaction 中读取完整 library 投影。
    ///
    /// # Errors
    ///
    /// SQLite、progress JSON 或 stream revision 损坏时返回 `StorageError`。
    pub async fn get_library_projection(&self) -> Result<LibraryProjection, StorageError> {
        self.read(move |conn, _| Box::pin(async move { library_projection_sync(conn).await }))
            .await
    }

    /// 读取单个 library 用户状态。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 `StorageError`。
    pub async fn get_library_entry(
        &self,
        resource_id: MediaResourceId,
    ) -> Result<Option<LibraryEntry>, StorageError> {
        self.read(move |conn, _| {
            Box::pin(async move { get_library_entry_sync(conn, &resource_id.0).await })
        })
        .await
    }

    /// 按来源读取规范化投影。
    ///
    /// # Errors
    ///
    /// `SQLite` 或资源 JSON 损坏时返回 `StorageError`。
    pub async fn source_projection(
        &self,
        source_identity: impl Into<String>,
    ) -> Result<SourceProjectionView, StorageError> {
        let source_identity = source_identity.into();
        self.read(move |conn, _| {
            Box::pin(async move { source_projection_sync(conn, &source_identity).await })
        })
        .await
    }

    /// 按稳定 ID 查询媒体主体。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 `StorageError`。
    pub async fn get_item(
        &self,
        resource_id: MediaResourceId,
    ) -> Result<Option<MediaItem>, StorageError> {
        self.read(move |conn, _| {
            Box::pin(async move {
                get_payload_by_id(conn, "projection_items", "id", &resource_id.0).await
            })
        })
        .await
    }

    /// 有界批量查询媒体主体；跳过缺失项，按 ID 升序。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 `StorageError`。
    pub async fn get_items(
        &self,
        resource_ids: Vec<MediaResourceId>,
    ) -> Result<Vec<MediaItem>, StorageError> {
        let ids = resource_ids.into_iter().map(|id| id.0).collect::<Vec<_>>();
        self.read(move |conn, _| Box::pin(async move { get_items_by_ids(conn, &ids).await }))
            .await
    }

    /// 按稳定 ID 查询消费单元。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 `StorageError`。
    pub async fn get_unit(
        &self,
        resource_id: MediaResourceId,
    ) -> Result<Option<MediaUnit>, StorageError> {
        self.read(move |conn, _| {
            Box::pin(async move {
                get_payload_by_id(conn, "projection_units", "id", &resource_id.0).await
            })
        })
        .await
    }

    /// 按来源索引查询媒体主体。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 `StorageError`。
    pub async fn list_items_by_source(
        &self,
        source_identity: impl Into<String>,
    ) -> Result<Vec<MediaItem>, StorageError> {
        let source_identity = source_identity.into();
        self.read(move |conn, _| {
            Box::pin(async move {
                payloads_by_column(
                    conn,
                    "projection_items",
                    "source_identity",
                    &source_identity,
                )
                .await
            })
        })
        .await
    }

    /// 按 item 索引有界查询 units。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 `StorageError`。
    pub async fn list_units_for_item(
        &self,
        item_id: MediaResourceId,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<MediaUnit>, StorageError> {
        self.read(move |conn, _| {
            Box::pin(
                async move { list_units_for_item_bounded(conn, &item_id.0, offset, limit).await },
            )
        })
        .await
    }

    /// 按 unit 索引有界查询 assets。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 `StorageError`。
    pub async fn list_assets_for_unit(
        &self,
        unit_id: MediaResourceId,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<MediaAsset>, StorageError> {
        self.read(move |conn, _| {
            Box::pin(
                async move { list_assets_for_unit_bounded(conn, &unit_id.0, offset, limit).await },
            )
        })
        .await
    }
}
