//! 跨表不变量与原子写操作。

use crate::database::{DatabaseSession, TransactionFuture};
use crate::types::StorageError;

pub(crate) mod archive;
pub(crate) mod candidate;
pub(crate) mod document;
pub(crate) mod event;
pub(crate) mod execution;
pub(crate) mod maintenance;
pub(crate) mod projection;

async fn run<T: Send>(
    connection: &DatabaseSession,
    operation: impl for<'a> FnOnce(&'a mut DatabaseSession) -> TransactionFuture<'a, T>,
) -> Result<T, StorageError> {
    connection.immediate_transaction(operation).await
}
