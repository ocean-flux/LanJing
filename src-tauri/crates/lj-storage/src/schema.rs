//! Diesel embedded migrations 的唯一入口。
//!
//! 运行时查询使用 storage owner 内的 Diesel `sql_query`，不把 schema 行模型暴露给
//! application 或 RuleSystem。migration 始终在 blocking lane 的初始化阶段执行。

use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::types::StorageError;

pub(crate) const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// 应用全部 pending migration。
pub(crate) fn run_migrations(conn: &mut SqliteConnection) -> Result<(), StorageError> {
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|error| StorageError::Database(error.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use diesel::prelude::*;
    use diesel::sql_query;
    use diesel::sql_types::{BigInt, Nullable, Text};
    use diesel::sqlite::SqliteConnection;
    use diesel_migrations::MigrationHarness;

    use super::{MIGRATIONS, run_migrations};

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
    fn control_invocation_archive_migration_round_trips_without_backfill() {
        let mut conn = SqliteConnection::establish(":memory:").expect("open migration fixture");
        run_migrations(&mut conn).expect("apply all migrations");
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
