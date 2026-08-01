//! `SeaORM` `SQLite` session 与 PRAGMA 初始化。

use std::path::Path;

use crate::database::{DatabaseSession, database_error};
use crate::types::StorageError;

pub(crate) async fn open_connection(
    path: &Path,
    migrate: bool,
) -> Result<DatabaseSession, StorageError> {
    let conn = DatabaseSession::connect(path, !migrate).await?;
    if migrate {
        let connection = conn
            .connection()
            .ok_or_else(|| StorageError::Database("writer connection missing".to_string()))?;
        lj_storage_migration::bootstrap_current_schema(connection)
            .await
            .map_err(|error| match error {
                lj_storage_migration::MigrationError::CurrentSchemaRequired
                | lj_storage_migration::MigrationError::SchemaCorrupt(_) => {
                    StorageError::CurrentSchemaRequired
                }
                lj_storage_migration::MigrationError::Database(error) => database_error(error),
            })?;
        conn.execute_unprepared("PRAGMA journal_mode = WAL;")
            .await
            .map_err(database_error)?;
        conn.execute_unprepared("PRAGMA synchronous = NORMAL;")
            .await
            .map_err(database_error)?;
        conn.execute_unprepared("PRAGMA wal_autocheckpoint = 1000;")
            .await
            .map_err(database_error)?;
    }
    conn.execute_unprepared("PRAGMA busy_timeout = 2000;")
        .await
        .map_err(database_error)?;
    conn.execute_unprepared("PRAGMA foreign_keys = ON;")
        .await
        .map_err(database_error)?;
    Ok(conn)
}
