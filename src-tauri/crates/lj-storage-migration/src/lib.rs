//! Fresh-only current schema migration 与完整性校验。

mod custom_schema;
mod m20260729_000001_current_schema;
mod schema_metadata;

use sea_orm::{ConnectionTrait, DatabaseConnection};
use sea_orm_migration::prelude::*;
use thiserror::Error;

pub use schema_metadata::{CURRENT_SCHEMA_NAME, CURRENT_SCHEMA_VERSION};

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20260729_000001_current_schema::Migration)]
    }
}

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("current schema required")]
    CurrentSchemaRequired,
    #[error("schema corrupt: {0}")]
    SchemaCorrupt(String),
    #[error(transparent)]
    Database(#[from] DbErr),
}

/// 仅空数据库执行唯一 baseline；current 数据库只校验，不演进。
///
/// # Errors
///
/// 数据库不是 current schema、schema 被篡改或底层数据库操作失败时返回错误。
pub async fn bootstrap_current_schema(
    connection: &DatabaseConnection,
) -> Result<(), MigrationError> {
    let state = schema_metadata::classify(connection).await?;
    match state {
        schema_metadata::SchemaState::Empty => Migrator::up(connection, None).await?,
        schema_metadata::SchemaState::Current => {}
        schema_metadata::SchemaState::Unknown => {
            return Err(MigrationError::CurrentSchemaRequired);
        }
    }
    schema_metadata::validate(connection).await
}

pub(crate) async fn execute_unprepared<C>(connection: &C, sql: &str) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    connection.execute_unprepared(sql).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, FromQueryResult, Statement};
    use sea_orm_migration::MigratorTrait;

    use super::{MigrationError, Migrator, bootstrap_current_schema};

    #[derive(FromQueryResult)]
    struct CountRow {
        count: i64,
    }

    async fn memory_database() -> sea_orm::DatabaseConnection {
        Database::connect("sqlite::memory:")
            .await
            .expect("open migration fixture")
    }

    async fn application_table_count(connection: &sea_orm::DatabaseConnection) -> i64 {
        connection
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> 'seaql_migrations'",
            ))
            .await
            .expect("query current tables")
            .map(|row| CountRow::from_query_result(&row, "").expect("decode count").count)
            .expect("table count row")
    }

    #[tokio::test]
    async fn fresh_baseline_builds_31_tables_and_reopens() {
        let connection = memory_database().await;
        bootstrap_current_schema(&connection)
            .await
            .expect("bootstrap current schema");
        assert_eq!(application_table_count(&connection).await, 31);

        bootstrap_current_schema(&connection)
            .await
            .expect("reopen current schema");
        assert_eq!(application_table_count(&connection).await, 31);
    }

    #[tokio::test]
    async fn baseline_down_up_rebuilds_current_schema() {
        let connection = memory_database().await;
        bootstrap_current_schema(&connection)
            .await
            .expect("bootstrap current schema");
        Migrator::down(&connection, None)
            .await
            .expect("drop baseline");
        assert_eq!(application_table_count(&connection).await, 0);

        Migrator::up(&connection, None)
            .await
            .expect("reapply baseline");
        bootstrap_current_schema(&connection)
            .await
            .expect("validate rebuilt schema");
        assert_eq!(application_table_count(&connection).await, 31);
    }

    #[tokio::test]
    async fn unknown_or_tampered_schema_is_rejected() {
        let unknown = memory_database().await;
        unknown
            .execute_unprepared("CREATE TABLE legacy_rules (id TEXT PRIMARY KEY NOT NULL)")
            .await
            .expect("create unknown schema");
        assert!(matches!(
            bootstrap_current_schema(&unknown).await,
            Err(MigrationError::CurrentSchemaRequired)
        ));

        let tampered = memory_database().await;
        bootstrap_current_schema(&tampered)
            .await
            .expect("bootstrap current schema");
        tampered
            .execute_unprepared("DROP INDEX idx_projection_items_source")
            .await
            .expect("tamper current schema");
        assert!(matches!(
            bootstrap_current_schema(&tampered).await,
            Err(MigrationError::SchemaCorrupt(_))
        ));
    }

    #[tokio::test]
    async fn current_checks_reject_invalid_rows() {
        let connection = memory_database().await;
        bootstrap_current_schema(&connection)
            .await
            .expect("bootstrap current schema");
        let error = connection
            .execute_unprepared(
                "INSERT INTO events (global_seq, stream_id, stream_version, event_id, event_type, schema_version, trace_id, occurred_at_ms, payload_json, artifact_refs_json, secret_refs_json) VALUES (0, 'bad', 0, 'bad', 'bad', 0, 'bad', 0, '{}', '[]', '[]')",
            )
            .await
            .expect_err("current check must reject invalid event");
        assert!(error.to_string().contains("current schema check failed"));
    }

    #[tokio::test]
    async fn invocation_uniqueness_is_scoped_to_execution() {
        let connection = memory_database().await;
        bootstrap_current_schema(&connection)
            .await
            .expect("bootstrap current schema");
        connection
            .execute_unprepared(
                "INSERT INTO source_versions (source_identity, source_revision, version, profile_json, grant_json, base_url, package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash, cookie_namespace, schema_version, installed_at_ms) VALUES ('source', 1, '1', '{}', '{}', '', 'package', 'plan-artifact', 'definition', 'plan', 'cookies', 1, 1)",
            )
            .await
            .expect("seed source version");
        for execution_id in ["execution-a", "execution-b"] {
            connection
                .execute_unprepared(&format!(
                    "INSERT INTO execution_projection (execution_id, source_identity, source_revision, source_version, plan_hash, plan_artifact_hash, status, pinned, archive_available, gc_state, started_at_ms, revision, updated_global_seq) VALUES ('{execution_id}', 'source', 1, '1', 'plan', 'plan-artifact', 'running', 0, 1, 'active', 1, 1, 1)"
                ))
                .await
                .expect("seed execution");
        }
        for (global_seq, execution_id) in [(1, "execution-a"), (2, "execution-b")] {
            connection
                .execute_unprepared(&format!(
                    "INSERT INTO events (global_seq, stream_id, stream_version, event_id, event_type, schema_version, trace_id, occurred_at_ms, payload_json, artifact_refs_json, secret_refs_json) VALUES ({global_seq}, '{execution_id}', 1, 'event-{global_seq}', 'EffectCaptured', 1, 'trace', 1, '{{}}', '[]', '[]')"
                ))
                .await
                .expect("seed event");
            connection
                .execute_unprepared(&format!(
                    "INSERT INTO execution_invocation_ledger (execution_id, invocation_ordinal, invocation_kind, node_id, invocation_path_json, payload_id) VALUES ('{execution_id}', 1, 'effect', 'node', '[\"same-path\"]', 'same-payload')"
                ))
                .await
                .expect("same invocation identity must be valid in another execution");
            connection
                .execute_unprepared(&format!(
                    "INSERT INTO effect_captures (execution_id, effect_id, node_id, effect_kind, fingerprint, output_hash, output_artifact_hash, global_seq, invocation_path_json, invocation_ordinal) VALUES ('{execution_id}', 'effect-{global_seq}', 'node', 'http', 'fingerprint', 'output', 'artifact', {global_seq}, '[\"same-path\"]', 1)"
                ))
                .await
                .expect("same effect ordinal must be valid in another execution");
        }

        connection
            .execute_unprepared(
                "INSERT INTO effect_captures (execution_id, effect_id, node_id, effect_kind, fingerprint, output_hash, output_artifact_hash, global_seq, invocation_path_json, invocation_ordinal) VALUES ('execution-a', 'duplicate-effect', 'node', 'http', 'fingerprint', 'output', 'artifact', 1, '[\"other-path\"]', 1)",
            )
            .await
            .expect_err("effect ordinal must be unique within one execution");
        connection
            .execute_unprepared(
                "INSERT INTO execution_invocation_ledger (execution_id, invocation_ordinal, invocation_kind, node_id, invocation_path_json, payload_id) VALUES ('execution-a', 2, 'effect', 'node', '[\"same-path\"]', 'other-payload')",
            )
            .await
            .expect_err("invocation path must be unique within one execution");
        connection
            .execute_unprepared(
                "INSERT INTO execution_invocation_ledger (execution_id, invocation_ordinal, invocation_kind, node_id, invocation_path_json, payload_id) VALUES ('execution-a', 2, 'effect', 'node', '[\"other-path\"]', 'same-payload')",
            )
            .await
            .expect_err("payload must be unique within one execution");
    }
}
