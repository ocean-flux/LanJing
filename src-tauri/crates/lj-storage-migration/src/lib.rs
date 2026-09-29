//! Fresh-only current schema migration 与完整性校验。

mod custom_schema;
mod m20260729_000001_current_schema;
mod m20260826_000002_rule_document_effective_semantics;
mod m20260830_000003_rule_document_effective_history;
mod schema_metadata;

use sea_orm::{ConnectionTrait, DatabaseConnection};
use sea_orm_migration::prelude::*;
use thiserror::Error;

pub use schema_metadata::{CURRENT_SCHEMA_NAME, CURRENT_SCHEMA_VERSION};

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260729_000001_current_schema::Migration),
            Box::new(m20260826_000002_rule_document_effective_semantics::Migration),
            Box::new(m20260830_000003_rule_document_effective_history::Migration),
        ]
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

/// 初始化 current schema；空数据库建立 baseline，受验证的旧版本执行加法升级。
///
/// # Errors
///
/// 数据库不是受支持 schema、schema 被篡改或底层数据库操作失败时返回错误。
pub async fn bootstrap_current_schema(
    connection: &DatabaseConnection,
) -> Result<(), MigrationError> {
    let state = schema_metadata::classify(connection).await?;
    match state {
        schema_metadata::SchemaState::Empty => Migrator::up(connection, None).await?,
        schema_metadata::SchemaState::Current => {
            schema_metadata::validate_upgrade_path(connection).await?;
            Migrator::up(connection, None).await?;
        }
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
    use std::collections::BTreeMap;

    use lj_capability::{IntentExport, StandardIntent};
    use lj_compiler::validate;
    use lj_rule_model::{
        CapabilityManifest, ControlledMapper, FlowEdge, FlowGraph, FlowNode, FlowNodeConfig,
        FlowPortRef, JsConfig, JsOutputKind, LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE,
        MapperOutputKind, PolicyCapabilities, RuleDefinition, SourceIdentity, SystemCapabilities,
    };
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, FromQueryResult, Statement};
    use sea_orm_migration::MigratorTrait;
    use uuid::Uuid;

    use super::{MigrationError, Migrator, bootstrap_current_schema, schema_metadata};

    #[derive(FromQueryResult)]
    struct CountRow {
        count: i64,
    }

    #[derive(FromQueryResult)]
    struct EffectiveSemanticRow {
        revision: i64,
        definition_hash: String,
        definition_json: String,
        manifest_json: String,
        updated_at_ms: i64,
    }

    fn valid_definition_json() -> String {
        let entry = Uuid::from_u128(1);
        let mapper = Uuid::from_u128(2);
        let definition = RuleDefinition::new(
            SourceIdentity {
                id: "native:source".to_string(),
            },
            "https://example.invalid",
            BTreeMap::from([(StandardIntent::Search, IntentExport::new(entry, mapper))]),
            FlowGraph {
                nodes: vec![
                    FlowNode::new(
                        entry,
                        FlowNodeConfig::Js(JsConfig {
                            code: "[]".to_string(),
                            output: JsOutputKind::Json,
                        }),
                    ),
                    FlowNode::new(
                        mapper,
                        FlowNodeConfig::Mapper(ControlledMapper {
                            output: MapperOutputKind::Items,
                            identity_fields: vec!["url".to_string()],
                        }),
                    ),
                ],
                edges: vec![FlowEdge::new(
                    FlowPortRef::new(entry, LINEAR_OUTPUT_HANDLE),
                    FlowPortRef::new(mapper, LINEAR_INPUT_HANDLE),
                )],
            },
            CapabilityManifest {
                required: PolicyCapabilities {
                    network: true,
                    system: SystemCapabilities::default(),
                },
            },
            vec!["url".to_string()],
        );
        let diagnostics = validate(&definition);
        assert!(
            diagnostics.is_empty(),
            "fixture must be valid: {diagnostics:?}"
        );
        serde_json::to_string(&definition).expect("serialize valid definition")
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
    async fn fresh_baseline_builds_33_tables_and_reopens() {
        let connection = memory_database().await;
        bootstrap_current_schema(&connection)
            .await
            .expect("bootstrap current schema");
        assert_eq!(application_table_count(&connection).await, 33);

        bootstrap_current_schema(&connection)
            .await
            .expect("reopen current schema");
        assert_eq!(application_table_count(&connection).await, 33);
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
            .expect("reapply current schema");
        assert_eq!(application_table_count(&connection).await, 33);
    }

    #[tokio::test]
    async fn v1_semantic_snapshot_migrates_to_effective_snapshot() {
        let connection = memory_database().await;
        bootstrap_current_schema(&connection)
            .await
            .expect("bootstrap current schema");
        connection
            .execute_unprepared("DROP TABLE rule_document_effective_semantics")
            .await
            .expect("remove v2 table");
        connection
            .execute_unprepared("DROP TABLE rule_document_effective_semantic_history")
            .await
            .expect("remove v3 table");
        connection
            .execute_unprepared(
                "DELETE FROM seaql_migrations WHERE version = 'm20260826_000002_rule_document_effective_semantics'",
            )
            .await
            .expect("remove v2 migration marker");
        connection
            .execute_unprepared(
                "DELETE FROM seaql_migrations WHERE version = 'm20260830_000003_rule_document_effective_history'",
            )
            .await
            .expect("remove v3 migration marker");
        schema_metadata::set_version_for_test(&connection, 1)
            .await
            .expect("mark v1 schema");
        let valid_definition = valid_definition_json().replace('\'', "''");
        connection
            .execute_unprepared(
                "INSERT INTO rule_documents (document_id, format, title, source_identity, state, semantic_revision, layout_revision, link_revision, created_at_ms, updated_at_ms) VALUES ('effective-document', 'native_rule', 'Title', 'native:source', 'draft', 7, 0, 0, 100, 200)",
            )
            .await
            .expect("seed v1 document");
        connection
            .execute_unprepared(&format!(
                "INSERT INTO rule_document_semantics (document_id, revision, definition_hash, definition_json, manifest_json, updated_at_ms) VALUES ('effective-document', 7, 'definition-hash', '{valid_definition}', '{{\"schema_version\":1,\"slots\":[]}}', 200)",
            ))
            .await
            .expect("seed v1 semantic snapshot");
        connection
            .execute_unprepared(
                "INSERT INTO rule_documents (document_id, format, title, source_identity, state, semantic_revision, layout_revision, link_revision, created_at_ms, updated_at_ms) VALUES ('draft-document', 'native_rule', 'Invalid', 'native:draft', 'draft', 3, 0, 0, 100, 200)",
            )
            .await
            .expect("seed invalid v1 document");
        connection
            .execute_unprepared(
                "INSERT INTO rule_document_semantics (document_id, revision, definition_hash, definition_json, manifest_json, updated_at_ms) VALUES ('draft-document', 3, 'invalid-hash', '{\"contract\":\"rule_definition\"}', '{\"schema_version\":1,\"slots\":[]}', 200)",
            )
            .await
            .expect("seed invalid v1 semantic snapshot");

        bootstrap_current_schema(&connection)
            .await
            .expect("upgrade v1 schema");

        let row = connection
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT revision, definition_hash, definition_json, manifest_json, updated_at_ms FROM rule_document_effective_semantics WHERE document_id = 'effective-document'",
            ))
            .await
            .expect("read effective snapshot")
            .map(|row| EffectiveSemanticRow::from_query_result(&row, "").expect("decode row"))
            .expect("effective snapshot");
        assert_eq!(row.revision, 7);
        assert_eq!(row.definition_hash, "definition-hash");
        assert_eq!(row.definition_json, valid_definition_json());
        assert_eq!(row.manifest_json, r#"{"schema_version":1,"slots":[]}"#);
        assert_eq!(row.updated_at_ms, 200);
        let invalid_count = connection
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM rule_document_effective_semantics WHERE document_id = 'draft-document'",
            ))
            .await
            .expect("count invalid effective snapshot")
            .map(|row| CountRow::from_query_result(&row, "").expect("decode count").count)
            .expect("invalid count row");
        assert_eq!(invalid_count, 0);
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
