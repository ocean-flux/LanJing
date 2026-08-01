//! Checkpoint、recovery 与 GC transaction services。

use uuid::Uuid;

use crate::artifact::ArtifactStore;
use crate::database::DatabaseSession;
use crate::types::{CheckpointReceipt, GcReport, OrphanRecovery, RetentionPolicy, StorageError};

pub(crate) async fn checkpoint_source(
    connection: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    source_identity: &str,
    created_at_ms: i64,
) -> Result<CheckpointReceipt, StorageError> {
    let artifacts = artifacts.clone();
    let source_identity = source_identity.to_string();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::maintenance::process_checkpoint_source(
                transaction,
                &artifacts,
                &source_identity,
                created_at_ms,
            )
            .await
        })
    })
    .await
}

pub(crate) async fn checkpoint_library(
    connection: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    created_at_ms: i64,
) -> Result<CheckpointReceipt, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::maintenance::process_checkpoint_library(
                transaction,
                &artifacts,
                created_at_ms,
            )
            .await
        })
    })
    .await
}

pub(crate) async fn run_gc(
    connection: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    policy: RetentionPolicy,
    now_ms: i64,
) -> Result<GcReport, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::maintenance::process_gc(transaction, &artifacts, policy, now_ms)
                .await
        })
    })
    .await
}

pub(crate) async fn clear_execution_archive(
    connection: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    execution_id: Uuid,
    confirm_pinned: bool,
    now_ms: i64,
) -> Result<GcReport, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::maintenance::process_clear_execution_archive(
                transaction,
                &artifacts,
                execution_id,
                confirm_pinned,
                now_ms,
            )
            .await
        })
    })
    .await
}

pub(crate) async fn recover_orphans(
    connection: &mut DatabaseSession,
    artifacts: &ArtifactStore,
) -> Result<OrphanRecovery, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::maintenance::recover_orphans_sync(transaction, &artifacts).await
        })
    })
    .await
}
