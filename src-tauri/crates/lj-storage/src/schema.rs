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
const CONTROL_INVOCATION_ARCHIVE_MIGRATION_VERSION: &str = "0009";

/// 应用全部 pending migration，并报告本次调用是否刚应用 0007 vault migration。
pub(crate) fn run_migrations(conn: &mut SqliteConnection) -> Result<bool, StorageError> {
    let applied = conn
        .run_pending_migrations(MIGRATIONS)
        .map_err(|error| StorageError::Database(error.to_string()))?;
    Ok(applied
        .iter()
        .any(|version| version.to_string() == SOURCE_DOCUMENT_VAULT_MIGRATION_VERSION))
}

/// 仅回滚刚应用且尚未开放给业务读写的 0007 vault migration 及其后续依赖 schema。
pub(crate) fn rollback_source_document_vault(
    conn: &mut SqliteConnection,
) -> Result<(), StorageError> {
    let mut reverted = conn
        .revert_last_migration(MIGRATIONS)
        .map_err(|_| StorageError::VaultMigrationFailed)?
        .to_string();
    if reverted == CONTROL_INVOCATION_ARCHIVE_MIGRATION_VERSION {
        reverted = conn
            .revert_last_migration(MIGRATIONS)
            .map_err(|_| StorageError::VaultMigrationFailed)?
            .to_string();
    }
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
    use diesel::sql_types::{BigInt, Nullable, Text};
    use diesel::sqlite::SqliteConnection;
    use diesel_migrations::MigrationHarness;

    use super::{MIGRATIONS, rollback_source_document_vault, run_migrations};

    #[derive(QueryableByName)]
    struct TextRow {
        #[diesel(sql_type = Text)]
        value: String,
    }

    #[derive(QueryableByName)]
    struct InvocationIdentityRow {
        #[diesel(sql_type = Nullable<Text>)]
        invocation_path_json: Option<String>,
        #[diesel(sql_type = Nullable<BigInt>)]
        invocation_ordinal: Option<i64>,
    }

    fn schema_object_exists(conn: &mut SqliteConnection, name: &str) -> bool {
        sql_query("SELECT sql AS value FROM sqlite_master WHERE name = ?")
            .bind::<Text, _>(name)
            .get_result::<TextRow>(conn)
            .optional()
            .expect("query SQLite schema object")
            .is_some()
    }

    fn effect_capture_columns(conn: &mut SqliteConnection) -> Vec<String> {
        sql_query("SELECT name AS value FROM pragma_table_info('effect_captures') ORDER BY cid")
            .load::<TextRow>(conn)
            .expect("read effect capture columns")
            .into_iter()
            .map(|row| row.value)
            .collect()
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

    #[test]
    fn control_invocation_archive_migration_round_trips_without_backfill() {
        let mut conn = SqliteConnection::establish(":memory:").expect("open migration fixture");
        assert!(run_migrations(&mut conn).expect("apply all migrations"));
        assert!(schema_object_exists(
            &mut conn,
            "execution_invocation_ledger"
        ));
        assert!(schema_object_exists(&mut conn, "control_traces"));
        assert!(schema_object_exists(
            &mut conn,
            "idx_effect_captures_current_invocation"
        ));
        assert!(effect_capture_columns(&mut conn).contains(&"invocation_path_json".to_string()));

        let reverted = conn
            .revert_last_migration(MIGRATIONS)
            .expect("revert invocation archive migration");
        assert_eq!(reverted.to_string(), "0009");
        assert!(!schema_object_exists(
            &mut conn,
            "execution_invocation_ledger"
        ));
        assert!(!schema_object_exists(&mut conn, "control_traces"));
        assert!(schema_object_exists(
            &mut conn,
            "idx_effect_captures_replay"
        ));
        assert!(!effect_capture_columns(&mut conn).contains(&"invocation_path_json".to_string()));
        sql_query(
            "INSERT INTO effect_captures (execution_id, effect_id, node_id, effect_kind, fingerprint, output_hash, output_artifact_hash, global_seq) VALUES ('execution', 'effect', 'node', '\"http\"', 'fingerprint', 'output-hash', 'artifact-hash', 1)",
        )
        .execute(&mut conn)
        .expect("seed legacy effect capture");

        let applied = conn
            .run_pending_migrations(MIGRATIONS)
            .expect("reapply invocation archive migration");
        assert_eq!(
            applied.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["0009"]
        );
        assert!(schema_object_exists(
            &mut conn,
            "execution_invocation_ledger"
        ));
        assert!(schema_object_exists(&mut conn, "control_traces"));
        assert!(schema_object_exists(
            &mut conn,
            "idx_effect_captures_current_path"
        ));
        let legacy_identity = sql_query(
            "SELECT invocation_path_json, invocation_ordinal FROM effect_captures WHERE effect_id = 'effect'",
        )
        .get_result::<InvocationIdentityRow>(&mut conn)
        .expect("read migrated legacy identity");
        assert_eq!(legacy_identity.invocation_path_json, None);
        assert_eq!(legacy_identity.invocation_ordinal, None);
    }
}
