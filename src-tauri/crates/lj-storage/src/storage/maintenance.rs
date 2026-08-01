//! checkpoint、recovery 与 retention façade。

use uuid::Uuid;

use super::EventProjectionStorage;
use crate::repository::execution::events_after_source_sync;
use crate::repository::maintenance::{
    load_library_checkpoint_sync, load_source_checkpoint_sync, recover_source_snapshot_sync,
};
use crate::types::{
    CheckpointReceipt, GcReport, LibraryProjectionSnapshot, OrphanRecovery, RetentionPolicy,
    SourceProjectionSnapshot, StorageError, StoredEvent,
};
use crate::writer::WriterCommand;

impl EventProjectionStorage {
    /// 删除 temp 与无 metadata artifact orphan。
    ///
    /// # Errors
    ///
    /// `SQLite` 或文件系统扫描失败时返回 `StorageError`。
    pub async fn recover_orphans(&self) -> Result<OrphanRecovery, StorageError> {
        self.dispatch(WriterCommand::RecoverOrphans).await
    }

    /// 创建并验证 source aggregate checkpoint。
    ///
    /// # Errors
    ///
    /// 来源、artifact、SQLite 或 snapshot 验证失败时返回 `StorageError`。
    pub async fn checkpoint_source(
        &self,
        source_identity: impl Into<String>,
        created_at_ms: i64,
    ) -> Result<CheckpointReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::CheckpointSource {
            source_identity: source_identity.into(),
            created_at_ms,
            reply,
        })
        .await
    }

    /// 创建并验证 library checkpoint。
    ///
    /// # Errors
    ///
    /// artifact、SQLite 或 snapshot 验证失败时返回 `StorageError`。
    pub async fn checkpoint_library(
        &self,
        created_at_ms: i64,
    ) -> Result<CheckpointReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::CheckpointLibrary {
            created_at_ms,
            reply,
        })
        .await
    }

    /// 读取最新 source checkpoint。
    ///
    /// # Errors
    ///
    /// SQLite、artifact 或 JSON 读取失败时返回 `StorageError`。
    pub async fn load_source_checkpoint(
        &self,
        source_identity: impl Into<String>,
    ) -> Result<Option<SourceProjectionSnapshot>, StorageError> {
        let source_identity = source_identity.into();
        self.read(move |conn, artifacts| {
            Box::pin(
                async move { load_source_checkpoint_sync(conn, artifacts, &source_identity).await },
            )
        })
        .await
    }

    /// 读取最新 library checkpoint。
    ///
    /// # Errors
    ///
    /// SQLite、artifact 或 JSON 读取失败时返回 `StorageError`。
    pub async fn load_library_checkpoint(
        &self,
    ) -> Result<Option<LibraryProjectionSnapshot>, StorageError> {
        self.read(|conn, artifacts| {
            Box::pin(async move { load_library_checkpoint_sync(conn, artifacts).await })
        })
        .await
    }

    /// 读取 source checkpoint 后的 durable events。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 `StorageError`。
    pub async fn source_events_after(
        &self,
        source_identity: impl Into<String>,
        after_global_seq: u64,
    ) -> Result<Vec<StoredEvent>, StorageError> {
        let source_identity = source_identity.into();
        self.read(move |conn, _| {
            Box::pin(async move {
                events_after_source_sync(conn, &source_identity, after_global_seq).await
            })
        })
        .await
    }

    /// 从 checkpoint 与后续 delta 重建 source snapshot。
    ///
    /// # Errors
    ///
    /// checkpoint、event、SQLite 或 artifact 失败时返回 `StorageError`。
    pub async fn recover_source_from_checkpoint(
        &self,
        source_identity: impl Into<String>,
    ) -> Result<SourceProjectionSnapshot, StorageError> {
        let source_identity = source_identity.into();
        self.read(move |conn, artifacts| {
            Box::pin(async move {
                recover_source_snapshot_sync(conn, artifacts, &source_identity).await
            })
        })
        .await
    }

    /// 清理一个 terminal execution archive。
    ///
    /// # Errors
    ///
    /// execution 状态、pin 确认、checkpoint、artifact 或 `SQLite` 失败时返回 `StorageError`。
    pub async fn clear_execution_archive(
        &self,
        execution_id: Uuid,
        confirm_pinned: bool,
        now_ms: i64,
    ) -> Result<GcReport, StorageError> {
        self.dispatch(|reply| WriterCommand::ClearExecutionArchive {
            execution_id,
            confirm_pinned,
            now_ms,
            reply,
        })
        .await
    }

    /// 执行 candidate 与 execution archive retention。
    ///
    /// # Errors
    ///
    /// checkpoint、artifact/keyring、SQLite 或文件删除失败时返回 `StorageError`。
    pub async fn run_gc(
        &self,
        policy: RetentionPolicy,
        now_ms: i64,
    ) -> Result<GcReport, StorageError> {
        self.dispatch(|reply| WriterCommand::RunGc {
            policy,
            now_ms,
            reply,
        })
        .await
    }
}
