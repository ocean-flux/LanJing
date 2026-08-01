//! execution lifecycle、source pin 与 event catch-up façade。

use uuid::Uuid;

use super::EventProjectionStorage;
use crate::repository::execution::{
    events_after_stream_sync, execution_stream_id, get_execution_sync,
    load_execution_replay_pin_sync, load_execution_source_credentials_sync,
};
use crate::types::{
    CommitReceipt, DeltaCommit, ExecutionFinish, ExecutionPin, ExecutionRecord, ExecutionReplayPin,
    ExecutionSourceCredentials, ExecutionStart, ExecutionStartReceipt, ReplayExecutionStart,
    StorageError, StoredEvent,
};
use crate::writer::WriterCommand;

impl EventProjectionStorage {
    /// 原子写 Started + revision pin，并返回同 revision installed snapshot。
    ///
    /// # Errors
    ///
    /// snapshot、credential、artifact、ID 或 transaction 失败时返回 `StorageError`。
    pub async fn start_execution(
        &self,
        request: ExecutionStart,
    ) -> Result<ExecutionStartReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::StartExecution { request, reply })
            .await
    }

    /// 从历史 pin 建立 replay archive，不读取 current source。
    ///
    /// # Errors
    ///
    /// pin/archive/artifact/mode 或写入失败时返回 `StorageError`。
    pub async fn start_replay_execution(
        &self,
        request: ReplayExecutionStart,
    ) -> Result<ExecutionRecord, StorageError> {
        self.dispatch(|reply| WriterCommand::StartReplayExecution {
            request: Box::new(request),
            reply,
        })
        .await
    }

    /// 原子写 execution Event 与规范化资源 delta。
    ///
    /// # Errors
    ///
    /// execution、version、ownership 或 transaction 失败时返回 `StorageError`。
    pub async fn commit_execution_delta(
        &self,
        request: DeltaCommit,
    ) -> Result<CommitReceipt, StorageError> {
        self.dispatch(|reply| WriterCommand::CommitDelta {
            request: Box::new(request),
            reply,
        })
        .await
    }

    /// 将 execution 推进到唯一终态。
    ///
    /// # Errors
    ///
    /// execution、version、terminal 或持久化失败时返回 `StorageError`。
    pub async fn finish_execution(
        &self,
        request: ExecutionFinish,
    ) -> Result<ExecutionRecord, StorageError> {
        self.dispatch(|reply| WriterCommand::FinishExecution { request, reply })
            .await
    }

    /// 修改 execution archive pin。
    ///
    /// # Errors
    ///
    /// execution、version 或持久化失败时返回 `StorageError`。
    pub async fn set_execution_pin(
        &self,
        request: ExecutionPin,
    ) -> Result<ExecutionRecord, StorageError> {
        self.dispatch(|reply| WriterCommand::SetExecutionPin { request, reply })
            .await
    }

    /// 读取 execution summary。
    ///
    /// # Errors
    ///
    /// `SQLite` 或状态损坏时返回 `StorageError`。
    pub async fn get_execution(
        &self,
        execution_id: Uuid,
    ) -> Result<Option<ExecutionRecord>, StorageError> {
        self.read(move |conn, _| {
            Box::pin(async move { get_execution_sync(conn, execution_id).await })
        })
        .await
    }

    /// 解密 live execution 固定 source-version credential snapshot。
    ///
    /// # Errors
    ///
    /// execution/source version/secret 无效或 replay 调用时返回 `StorageError`。
    pub async fn load_execution_source_credentials(
        &self,
        execution_id: Uuid,
    ) -> Result<ExecutionSourceCredentials, StorageError> {
        self.read(move |conn, artifacts| {
            Box::pin(async move {
                load_execution_source_credentials_sync(conn, artifacts, execution_id).await
            })
        })
        .await
    }

    /// 读取并验证 execution 固定 immutable Plan replay pin。
    ///
    /// # Errors
    ///
    /// archive、artifact、key 或 Plan integrity 失败时返回 `StorageError`。
    pub async fn load_execution_replay_pin(
        &self,
        execution_id: Uuid,
    ) -> Result<ExecutionReplayPin, StorageError> {
        self.read(move |conn, artifacts| {
            Box::pin(
                async move { load_execution_replay_pin_sync(conn, artifacts, execution_id).await },
            )
        })
        .await
    }

    /// 从指定 execution stream version 后补读 durable events。
    ///
    /// # Errors
    ///
    /// `SQLite` 或 JSON 读取失败时返回 `StorageError`。
    pub async fn catch_up_execution(
        &self,
        execution_id: Uuid,
        after_version: u64,
    ) -> Result<Vec<StoredEvent>, StorageError> {
        self.read(move |conn, _| {
            Box::pin(async move {
                events_after_stream_sync(conn, &execution_stream_id(execution_id), after_version)
                    .await
            })
        })
        .await
    }
}
