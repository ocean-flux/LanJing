//! Library projection transaction service。

use crate::database::DatabaseSession;
use crate::types::{CommitReceipt, LibraryUpdate, StorageError};

pub(crate) async fn update_library(
    connection: &DatabaseSession,
    request: LibraryUpdate,
) -> Result<CommitReceipt, StorageError> {
    super::run(connection, move |transaction| {
        Box::pin(async move {
            crate::repository::projection::process_library_update(transaction, request).await
        })
    })
    .await
}
