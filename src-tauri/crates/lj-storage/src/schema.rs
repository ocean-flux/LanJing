//! Diesel embedded migrations 的唯一入口。
//!
//! 运行时查询使用 storage owner 内的 Diesel `sql_query`，不把 schema 行模型暴露给
//! application 或 RuleSystem。migration 始终在 blocking lane 的初始化阶段执行。

use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::types::StorageError;

pub(crate) const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

const SOURCE_DOCUMENT_VAULT_MIGRATION_VERSION: &str = "0007";
const SOURCE_DOCUMENT_PIN_MIGRATION_VERSION: &str = "0008";

/// 应用全部 pending migration，并报告本次调用是否刚应用 0007 vault migration。
pub(crate) fn run_migrations(conn: &mut SqliteConnection) -> Result<bool, StorageError> {
    let applied = conn
        .run_pending_migrations(MIGRATIONS)
        .map_err(|error| StorageError::Database(error.to_string()))?;
    Ok(applied
        .iter()
        .any(|version| version.to_string() == SOURCE_DOCUMENT_VAULT_MIGRATION_VERSION))
}

/// 仅回滚刚应用且尚未开放给业务读写的 0007 vault migration 及其 0008 pin schema。
pub(crate) fn rollback_source_document_vault(
    conn: &mut SqliteConnection,
) -> Result<(), StorageError> {
    let mut reverted = conn
        .revert_last_migration(MIGRATIONS)
        .map_err(|_| StorageError::VaultMigrationFailed)?
        .to_string();
    if reverted == SOURCE_DOCUMENT_PIN_MIGRATION_VERSION {
        reverted = conn
            .revert_last_migration(MIGRATIONS)
            .map_err(|_| StorageError::VaultMigrationFailed)?
            .to_string();
    }
    if reverted != SOURCE_DOCUMENT_VAULT_MIGRATION_VERSION {
        return Err(StorageError::VaultMigrationFailed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use diesel::prelude::*;
    use diesel::sql_query;
    use diesel::sql_types::Text;
    use diesel::sqlite::SqliteConnection;

    use super::{rollback_source_document_vault, run_migrations};

    #[derive(QueryableByName)]
    struct TextRow {
        #[diesel(sql_type = Text)]
        value: String,
    }

    #[test]
    fn source_document_vault_migration_round_trips_legacy_staging() {
        let mut conn = SqliteConnection::establish(":memory:").expect("open migration fixture");
        assert!(run_migrations(&mut conn).expect("apply all migrations"));
        rollback_source_document_vault(&mut conn).expect("return fixture to migration 0006");

        sql_query(
            "INSERT INTO source_credential_staging (candidate_id, source_identity, cookie_namespace, secret_artifact_hash, expires_at_ms, created_at_ms) VALUES ('candidate', 'source:test', 'cookie', 'legacy-secret-hash', 20, 10)",
        )
        .execute(&mut conn)
        .expect("seed legacy credential staging");

        assert!(run_migrations(&mut conn).expect("apply vault migration"));
        rollback_source_document_vault(&mut conn).expect("rollback unused vault migration");
        let restored = sql_query(
            "SELECT secret_artifact_hash AS value FROM source_credential_staging WHERE candidate_id = 'candidate'",
        )
        .get_result::<TextRow>(&mut conn)
        .expect("read restored legacy credential staging");
        assert_eq!(restored.value, "legacy-secret-hash");
    }
}
