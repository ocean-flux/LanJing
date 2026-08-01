//! Execution lifecycle transaction service。

use crate::artifact::ArtifactStore;
use crate::database::DatabaseSession;
use crate::types::{
    CommitReceipt, DeltaCommit, ExecutionFinish, ExecutionPin, ExecutionRecord, ExecutionStart,
    ExecutionStartReceipt, ReplayExecutionStart, StorageError,
};

pub(crate) async fn start(
    connection: &DatabaseSession,
    artifacts: &ArtifactStore,
    request: ExecutionStart,
) -> Result<ExecutionStartReceipt, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::execution::process_start_execution(transaction, &artifacts, request)
                .await
        })
    })
    .await
}

pub(crate) async fn start_replay(
    connection: &DatabaseSession,
    artifacts: &ArtifactStore,
    request: ReplayExecutionStart,
) -> Result<ExecutionRecord, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::execution::process_start_replay_execution(
                transaction,
                &artifacts,
                &request,
            )
            .await
        })
    })
    .await
}

pub(crate) async fn commit_delta(
    connection: &DatabaseSession,
    request: DeltaCommit,
) -> Result<CommitReceipt, StorageError> {
    super::run(connection, move |transaction| {
        Box::pin(
            async move { crate::repository::execution::process_delta(transaction, request).await },
        )
    })
    .await
}

pub(crate) async fn finish(
    connection: &DatabaseSession,
    request: ExecutionFinish,
) -> Result<ExecutionRecord, StorageError> {
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::execution::process_finish_execution(transaction, request).await
        })
    })
    .await
}

pub(crate) async fn set_pin(
    connection: &DatabaseSession,
    request: ExecutionPin,
) -> Result<ExecutionRecord, StorageError> {
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::execution::process_pin_execution(transaction, request).await
        })
    })
    .await
}
