use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;

use crate::{execute_unprepared, schema_metadata};

#[derive(DeriveMigrationName)]
pub struct Migration;

const METADATA_TRIGGER_PREDICATE: &str =
    "NEW.id <> 1 OR NEW.schema_name <> 'lanjing_current' OR NEW.schema_version NOT IN (1, 2, 3)";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        reset_metadata_triggers(connection, METADATA_TRIGGER_PREDICATE).await?;
        execute_unprepared(
            connection,
            "CREATE TABLE rule_document_effective_semantic_history (document_id TEXT NOT NULL REFERENCES rule_documents(document_id) ON DELETE CASCADE, revision BIGINT NOT NULL CHECK(revision > 0), definition_hash TEXT NOT NULL, definition_json TEXT NOT NULL, manifest_json TEXT NOT NULL, updated_at_ms BIGINT NOT NULL CHECK(updated_at_ms >= 0), PRIMARY KEY(document_id, revision))",
        )
        .await?;
        execute_unprepared(
            connection,
            "CREATE INDEX idx_rule_document_effective_history_order ON rule_document_effective_semantic_history(document_id, updated_at_ms DESC, revision DESC)",
        )
        .await?;
        create_history_triggers(connection).await?;
        execute_unprepared(
            connection,
            "INSERT INTO rule_document_effective_semantic_history (document_id, revision, definition_hash, definition_json, manifest_json, updated_at_ms) SELECT document_id, revision, definition_hash, definition_json, manifest_json, updated_at_ms FROM rule_document_effective_semantics",
        )
        .await?;
        schema_metadata::refresh_to_version(connection, 3).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        for trigger in [
            "ck_rule_document_effective_semantic_history_insert",
            "ck_rule_document_effective_semantic_history_update",
        ] {
            execute_unprepared(connection, &format!("DROP TRIGGER IF EXISTS {trigger}")).await?;
        }
        execute_unprepared(
            connection,
            "DROP INDEX IF EXISTS idx_rule_document_effective_history_order",
        )
        .await?;
        execute_unprepared(
            connection,
            "DROP TABLE IF EXISTS rule_document_effective_semantic_history",
        )
        .await?;
        reset_metadata_triggers(
            connection,
            "NEW.id <> 1 OR NEW.schema_name <> 'lanjing_current' OR NEW.schema_version NOT IN (1, 2)",
        )
        .await?;
        schema_metadata::refresh_to_version(connection, 2).await
    }
}

async fn reset_metadata_triggers<C>(connection: &C, predicate: &str) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    for trigger in [
        "ck_storage_schema_metadata_insert",
        "ck_storage_schema_metadata_update",
    ] {
        execute_unprepared(connection, &format!("DROP TRIGGER IF EXISTS {trigger}")).await?;
    }
    for operation in ["INSERT", "UPDATE"] {
        execute_unprepared(
            connection,
            &format!(
                "CREATE TRIGGER ck_storage_schema_metadata_{} BEFORE {} ON storage_schema_metadata WHEN {} BEGIN SELECT RAISE(ABORT, 'current schema check failed'); END",
                operation.to_ascii_lowercase(),
                operation,
                predicate
            ),
        )
        .await?;
    }
    Ok(())
}

async fn create_history_triggers<C>(connection: &C) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    for operation in ["INSERT", "UPDATE"] {
        let suffix = operation.to_ascii_lowercase();
        execute_unprepared(
            connection,
            &format!(
                "CREATE TRIGGER ck_rule_document_effective_semantic_history_{suffix} BEFORE {operation} ON rule_document_effective_semantic_history WHEN NEW.revision <= 0 OR NEW.updated_at_ms < 0 BEGIN SELECT RAISE(ABORT, 'current schema check failed'); END"
            ),
        )
        .await?;
    }
    Ok(())
}
