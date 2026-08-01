//! 通用 Event append transaction service。

use crate::artifact::ArtifactStore;
use crate::database::DatabaseSession;
use crate::types::{AppendRequest, CommitReceipt, StorageError};

pub(crate) async fn append(
    connection: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    request: AppendRequest,
) -> Result<CommitReceipt, StorageError> {
    let artifacts = artifacts.clone();
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::event::process_append(transaction, &artifacts, request).await
        })
    })
    .await
}
