//! `SQLite` connection 与 storage configuration 的 blocking-lane 初始化。
//!
//! Diesel connection 绝不跨 `.await` 或离开 writer/read blocking lane。writer 初始化时执行
//! migration/WAL pragma；read lane 只开短生命周期独立连接。`StorageConfig` 禁止 `:memory:`，
//! 因为 artifact durability、restart recovery 与单 writer 合同都要求真实文件系统。

use std::path::Path;

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::event_store::database_error;
use crate::schema::run_migrations;
use crate::types::{StorageConfig, StorageError};

pub(crate) fn open_connection(
    path: &Path,
    migrate: bool,
) -> Result<SqliteConnection, StorageError> {
    let url = path.to_string_lossy();
    let mut conn = SqliteConnection::establish(&url).map_err(database_error)?;
    if migrate {
        run_migrations(&mut conn)?;
        conn.batch_execute("PRAGMA busy_timeout = 2000;")
            .map_err(database_error)?;
        conn.batch_execute("PRAGMA journal_mode = WAL;")
            .map_err(database_error)?;
        conn.batch_execute("PRAGMA synchronous = NORMAL;")
            .map_err(database_error)?;
        conn.batch_execute("PRAGMA wal_autocheckpoint = 1000;")
            .map_err(database_error)?;
    } else {
        conn.batch_execute("PRAGMA busy_timeout = 2000;")
            .map_err(database_error)?;
    }
    conn.batch_execute("PRAGMA foreign_keys = ON;")
        .map_err(database_error)?;
    Ok(conn)
}

pub(crate) fn validate_config(config: &StorageConfig) -> Result<(), StorageError> {
    if config.database_path.to_string_lossy() == ":memory:" {
        return Err(StorageError::InvalidInput(
            "Event Store 禁止使用 :memory: SQLite".to_string(),
        ));
    }
    if config.read_concurrency == 0 {
        return Err(StorageError::InvalidInput(
            "read_concurrency 必须至少为 1".to_string(),
        ));
    }
    if let Some(parent) = config.database_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| StorageError::FileSystem(error.to_string()))?;
    }
    std::fs::create_dir_all(&config.artifact_root)
        .map_err(|error| StorageError::FileSystem(error.to_string()))?;
    Ok(())
}
