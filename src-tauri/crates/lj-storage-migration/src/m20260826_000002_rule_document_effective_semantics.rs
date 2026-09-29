use lj_compiler::validate;
use lj_rule_model::{DiagnosticSeverity, read_rule_definition};
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use sea_orm_migration::prelude::*;

use crate::{execute_unprepared, schema_metadata};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(FromQueryResult)]
struct LegacySemanticSnapshot {
    document_id: String,
    revision: i64,
    definition_hash: String,
    definition_json: String,
    manifest_json: String,
    updated_at_ms: i64,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        for trigger in [
            "ck_storage_schema_metadata_insert",
            "ck_storage_schema_metadata_update",
        ] {
            execute_unprepared(connection, &format!("DROP TRIGGER {trigger}")).await?;
        }
        for operation in ["INSERT", "UPDATE"] {
            execute_unprepared(
                connection,
                &format!(
                    "CREATE TRIGGER ck_storage_schema_metadata_{} BEFORE {} ON storage_schema_metadata WHEN NEW.id <> 1 OR NEW.schema_name <> 'lanjing_current' OR NEW.schema_version NOT IN (1, 2) BEGIN SELECT RAISE(ABORT, 'current schema check failed'); END",
                    operation.to_ascii_lowercase(),
                    operation,
                ),
            )
            .await?;
        }
        execute_unprepared(
            connection,
            "CREATE TABLE rule_document_effective_semantics (document_id TEXT PRIMARY KEY NOT NULL REFERENCES rule_documents(document_id) ON DELETE CASCADE, revision BIGINT NOT NULL CHECK(revision > 0), definition_hash TEXT NOT NULL, definition_json TEXT NOT NULL, manifest_json TEXT NOT NULL, updated_at_ms BIGINT NOT NULL CHECK(updated_at_ms >= 0))",
        )
        .await?;
        backfill_effective_semantics(connection).await?;
        schema_metadata::refresh_to_version(connection, 2).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        execute_unprepared(
            manager.get_connection(),
            "DROP TABLE rule_document_effective_semantics",
        )
        .await
    }
}

async fn backfill_effective_semantics<C>(connection: &C) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let snapshots = connection
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT document_id, revision, definition_hash, definition_json, manifest_json, updated_at_ms FROM rule_document_semantics WHERE revision > 0",
        ))
        .await?
        .iter()
        .map(|row| LegacySemanticSnapshot::from_query_result(row, ""))
        .collect::<Result<Vec<_>, _>>()?;

    for snapshot in snapshots {
        let Ok(definition) = read_rule_definition(snapshot.definition_json.as_bytes()) else {
            continue;
        };
        if validate(&definition)
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        {
            continue;
        }
        connection
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "INSERT INTO rule_document_effective_semantics (document_id, revision, definition_hash, definition_json, manifest_json, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?)",
                vec![
                    snapshot.document_id.into(),
                    snapshot.revision.into(),
                    snapshot.definition_hash.into(),
                    snapshot.definition_json.into(),
                    snapshot.manifest_json.into(),
                    snapshot.updated_at_ms.into(),
                ],
            ))
            .await?;
    }
    Ok(())
}
