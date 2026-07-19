//! Diesel embedded migrations 的唯一入口。
//!
//! 运行时查询使用 storage owner 内的 Diesel `sql_query`，不把 schema 行模型暴露给
//! application 或 RuleSystem。migration 始终在 blocking lane 的初始化阶段执行。

use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::types::StorageError;

pub(crate) const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// 在新建或升级数据库时运行嵌入 migration。
///
/// # Errors
///
/// Diesel 无法应用 migration 时返回 [`StorageError::Database`]。
pub(crate) fn run_migrations(conn: &mut SqliteConnection) -> Result<(), StorageError> {
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|error| StorageError::Database(error.to_string()))?;
    Ok(())
}
