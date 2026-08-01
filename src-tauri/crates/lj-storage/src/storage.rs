//! `EventProjectionStorage` state、async writer 与 read-only pool。

mod archive;
mod candidate;
mod execution;
mod maintenance;
mod query;

use sea_orm::DatabaseConnection;
use tokio::sync::{mpsc, oneshot};

use crate::artifact::ArtifactStore;
use crate::connection::open_connection;
use crate::database::{DatabaseSession, TransactionFuture, connect_pool, validate_config};
use crate::repository::event::now_millis;
use crate::repository::maintenance::{
    mark_interrupted_executions, normalize_artifact_relative_paths, purge_zero_ref_artifacts,
    recover_orphans_sync,
};
use crate::repository::secret::{purge_zero_ref_secrets, validate_secret_ref_counts};
use crate::types::{AppendRequest, CommitReceipt, StorageConfig, StorageError, WRITER_CAPACITY};
use crate::writer::{WriterCommand, writer_loop};

/// single-writer Event Store、规范化投影与 durable archive 的具体 façade。
#[derive(Clone)]
pub struct EventProjectionStorage {
    writer: mpsc::Sender<WriterCommand>,
    read_pool: DatabaseConnection,
    artifacts: ArtifactStore,
}

impl EventProjectionStorage {
    /// 初始化 current `SQLite` schema、artifact recovery、single writer 与 read-only pool。
    ///
    /// # Errors
    ///
    /// 数据库、artifact 根目录、current schema 或 writer 启动失败时返回 `StorageError`。
    pub async fn open(config: StorageConfig) -> Result<Self, StorageError> {
        validate_config(&config)?;
        let mut writer_connection = open_connection(&config.database_path, true).await?;
        let artifacts = ArtifactStore::new(config.artifact_root, &config.keyring_service)?;
        normalize_artifact_relative_paths(&mut writer_connection).await?;
        crate::transaction::candidate::recover(&writer_connection, now_millis()).await?;
        validate_secret_ref_counts(&mut writer_connection).await?;
        purge_zero_ref_artifacts(&mut writer_connection, &artifacts).await?;
        purge_zero_ref_secrets(&mut writer_connection, &artifacts).await?;
        recover_orphans_sync(&mut writer_connection, &artifacts).await?;
        mark_interrupted_executions(&mut writer_connection).await?;

        let read_pool = connect_pool(
            &config.database_path,
            true,
            u32::try_from(config.read_concurrency)
                .map_err(|_| StorageError::InvalidInput("read_concurrency 超出 u32".to_string()))?,
        )
        .await?;

        let (writer, receiver) = mpsc::channel(WRITER_CAPACITY);
        let writer_artifacts = artifacts.clone();
        tokio::spawn(async move {
            writer_loop(receiver, writer_connection, writer_artifacts).await;
        });

        Ok(Self {
            writer,
            read_pool,
            artifacts,
        })
    }

    /// 返回唯一 writer 的固定队列容量。
    #[must_use]
    pub const fn writer_capacity(&self) -> usize {
        WRITER_CAPACITY
    }

    /// 追加一个不影响投影的领域事件。
    ///
    /// # Errors
    ///
    /// optimistic conflict、artifact、transaction 或 writer 失败时返回 `StorageError`。
    pub async fn append_event(
        &self,
        request: AppendRequest,
    ) -> Result<CommitReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::Append { request, reply })
            .await
    }

    /// 有序关闭 single writer。
    ///
    /// # Errors
    ///
    /// writer 已停止或无法完成已接收命令时返回 `StorageError`。
    pub async fn shutdown(&self) -> Result<(), StorageError> {
        self.dispatch(WriterCommand::Shutdown).await
    }

    async fn dispatch<T: Send + 'static>(
        &self,
        build: impl FnOnce(oneshot::Sender<Result<T, StorageError>>) -> WriterCommand,
    ) -> Result<T, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.writer
            .send(build(reply))
            .await
            .map_err(|_| StorageError::WriterClosed)?;
        receiver
            .await
            .map_err(|_| StorageError::WriterUnavailable)?
    }

    async fn read<T: Send + 'static>(
        &self,
        operation: impl for<'a> FnOnce(
            &'a mut DatabaseSession,
            &'a ArtifactStore,
        ) -> TransactionFuture<'a, T>,
    ) -> Result<T, StorageError> {
        let mut connection = DatabaseSession::from_connection(self.read_pool.clone());
        operation(&mut connection, &self.artifacts).await
    }
}
