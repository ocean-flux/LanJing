//! 启动期路径归一化、orphan recovery 与 interrupted execution 收敛。

use std::collections::HashSet;

use crate::artifact::ArtifactStore;
use crate::database::{DatabaseSession, statement};
use crate::repository::event::{ArtifactPathRow, TextValue, database_error, now_millis};
use crate::repository::secret::referenced_secret_paths;
use crate::types::{OrphanRecovery, StorageError};

pub(crate) async fn purge_zero_ref_artifacts(
    connection: &mut DatabaseSession,
    artifacts: &ArtifactStore,
) -> Result<(), StorageError> {
    let rows = statement("SELECT relative_path FROM artifact_metadata WHERE ref_count = 0")
        .load::<ArtifactPathRow>(connection)
        .await
        .map_err(database_error)?;
    connection
        .immediate_transaction(|connection| {
            Box::pin(async move {
                statement("DELETE FROM artifact_metadata WHERE ref_count = 0")
                    .execute(connection)
                    .await
                    .map_err(database_error)?;
                Ok(())
            })
        })
        .await?;
    for artifact in &rows {
        artifacts.remove_file(&artifact.relative_path)?;
    }
    Ok(())
}

pub(crate) async fn normalize_artifact_relative_paths(
    connection: &mut DatabaseSession,
) -> Result<(), StorageError> {
    statement(
        "UPDATE artifact_metadata SET relative_path = REPLACE(relative_path, ?, ?) WHERE INSTR(relative_path, ?) > 0",
    )
    .bind("\\")
    .bind("/")
    .bind("\\")
    .execute(connection)
    .await
    .map_err(database_error)?;
    Ok(())
}

pub(crate) async fn recover_orphans_sync(
    connection: &mut DatabaseSession,
    artifacts: &ArtifactStore,
) -> Result<OrphanRecovery, StorageError> {
    let rows = statement("SELECT relative_path AS value FROM artifact_metadata")
        .load::<TextValue>(connection)
        .await
        .map_err(database_error)?;
    let mut paths = rows
        .into_iter()
        .map(|row| row.value)
        .collect::<HashSet<_>>();
    paths.extend(referenced_secret_paths(connection).await?);
    artifacts.recover_orphans(&paths)
}

pub(crate) async fn mark_interrupted_executions(
    connection: &mut DatabaseSession,
) -> Result<(), StorageError> {
    statement("UPDATE execution_projection SET status = 'incomplete', finished_at_ms = COALESCE(finished_at_ms, ?) WHERE status = 'running'")
        .bind(now_millis())
        .execute(connection)
        .await
        .map_err(database_error)?;
    Ok(())
}
