use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::DbErr;

use crate::execute_unprepared;

const INDEXES: &[&str] = &[
    "CREATE UNIQUE INDEX idx_events_stream_version ON events(stream_id, stream_version)",
    "CREATE UNIQUE INDEX idx_execution_invocation_ledger_payload ON execution_invocation_ledger(execution_id, payload_id)",
    "CREATE UNIQUE INDEX idx_effect_captures_current_path ON effect_captures(execution_id, invocation_path_json)",
    "CREATE INDEX idx_events_source_global_seq ON events(source_identity, global_seq)",
    "CREATE INDEX idx_artifact_ref_count ON artifact_metadata(ref_count)",
    "CREATE INDEX idx_event_artifact_refs_hash ON event_artifact_refs(hash, artifact_kind)",
    "CREATE INDEX idx_secret_artifact_ref_count ON secret_artifact_projection(ref_count, created_at_ms)",
    "CREATE INDEX idx_secret_artifact_owner_secret ON secret_artifact_owners(secret_id)",
    "CREATE INDEX idx_candidate_projection_expiry ON candidate_projection(status, expires_at_ms)",
    "CREATE INDEX idx_source_versions_definition ON source_versions(source_identity, version, definition_hash)",
    "CREATE INDEX idx_source_versions_plan ON source_versions(source_identity, plan_hash)",
    "CREATE INDEX idx_execution_gc ON execution_projection(status, pinned, finished_at_ms, gc_state)",
    "CREATE INDEX idx_execution_source ON execution_projection(source_identity, source_revision, started_at_ms)",
    "CREATE INDEX idx_projection_items_source ON projection_items(source_identity, id)",
    "CREATE INDEX idx_projection_collections_source ON projection_collections(source_identity, id)",
    "CREATE INDEX idx_projection_units_item ON projection_units(item_id, position, id)",
    "CREATE INDEX idx_projection_units_source ON projection_units(source_identity, id)",
    "CREATE INDEX idx_projection_assets_unit ON projection_assets(unit_id, id)",
    "CREATE INDEX idx_projection_assets_source ON projection_assets(source_identity, id)",
    "CREATE INDEX idx_projection_relations_from ON projection_relations(from_id, relation_kind)",
    "CREATE INDEX idx_projection_actions_source ON projection_actions(source_identity, id)",
    "CREATE INDEX idx_projection_hints_source ON projection_hints(source_identity, resource_id)",
    "CREATE INDEX idx_library_projection_owned ON library_projection(favorite, pinned, resource_id)",
    "CREATE INDEX idx_rule_documents_source_identity ON rule_documents(source_identity)",
];

const CHECKS: &[(&str, &str)] = &[
    (
        "storage_schema_metadata",
        "NEW.id <> 1 OR NEW.schema_name <> 'lanjing_current' OR NEW.schema_version <> 1",
    ),
    ("event_counters", "NEW.id <> 1 OR NEW.next_global_seq < 0"),
    (
        "events",
        "NEW.global_seq <= 0 OR NEW.stream_version <= 0 OR NEW.schema_version <= 0",
    ),
    (
        "artifact_metadata",
        "NEW.hash_algorithm <> 'blake3' OR NEW.stored_bytes < 0 OR NEW.ref_count < 0",
    ),
    (
        "vault_key_metadata",
        "NEW.id <> 1 OR NEW.schema_version <= 0",
    ),
    (
        "secret_artifact_projection",
        "NEW.stored_bytes < 0 OR NEW.ref_count < 0 OR NEW.schema_version <= 0",
    ),
    (
        "candidate_projection",
        "NEW.candidate_schema_version <= 0 OR NEW.expected_installed_revision < 0 OR NEW.status NOT IN ('staged','consumed','expired') OR NEW.stream_version <= 0",
    ),
    (
        "source_projection",
        "NEW.revision <= 0 OR NEW.updated_global_seq <= 0",
    ),
    (
        "source_versions",
        "NEW.source_revision <= 0 OR NEW.schema_version <= 0",
    ),
    (
        "execution_projection",
        "NEW.source_revision <= 0 OR NEW.status NOT IN ('running','completed','failed','cancelled','incomplete') OR NEW.pinned NOT IN (0,1) OR NEW.archive_available NOT IN (0,1) OR NEW.gc_state NOT IN ('active','marked','external_refs_removed','finalized') OR NEW.revision <= 0 OR NEW.updated_global_seq <= 0",
    ),
    (
        "execution_invocation_ledger",
        "NEW.invocation_ordinal <= 0 OR NEW.invocation_kind NOT IN ('effect','control')",
    ),
    ("effect_captures", "NEW.invocation_ordinal <= 0"),
    (
        "source_checkpoints",
        "NEW.source_revision <= 0 OR NEW.global_seq <= 0",
    ),
    ("library_checkpoints", "NEW.id <> 1 OR NEW.global_seq <= 0"),
    ("projection_sources", "NEW.updated_global_seq <= 0"),
    ("projection_items", "NEW.updated_global_seq <= 0"),
    ("projection_collections", "NEW.updated_global_seq <= 0"),
    ("projection_units", "NEW.updated_global_seq <= 0"),
    ("projection_assets", "NEW.updated_global_seq <= 0"),
    ("projection_relations", "NEW.updated_global_seq <= 0"),
    ("projection_actions", "NEW.updated_global_seq <= 0"),
    ("projection_hints", "NEW.updated_global_seq <= 0"),
    (
        "library_projection",
        "NEW.favorite NOT IN (0,1) OR NEW.pinned NOT IN (0,1) OR NEW.updated_global_seq <= 0",
    ),
    (
        "rule_documents",
        "NEW.state NOT IN ('draft','linked') OR NEW.semantic_revision < 0 OR NEW.layout_revision < 0 OR NEW.link_revision < 0 OR NEW.created_at_ms < 0 OR NEW.updated_at_ms < 0",
    ),
    (
        "rule_document_semantics",
        "NEW.revision < 0 OR NEW.updated_at_ms < 0",
    ),
    (
        "rule_document_layouts",
        "NEW.revision < 0 OR NEW.updated_at_ms < 0",
    ),
    ("rule_document_provenances", "NEW.imported_at_ms < 0"),
];

pub(crate) async fn apply<C>(connection: &C) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    for sql in INDEXES {
        execute_unprepared(connection, sql).await?;
    }
    for (table, predicate) in CHECKS {
        for operation in ["INSERT", "UPDATE"] {
            let suffix = operation.to_ascii_lowercase();
            let sql = format!(
                "CREATE TRIGGER ck_{table}_{suffix} BEFORE {operation} ON {table} WHEN {predicate} BEGIN SELECT RAISE(ABORT, 'current schema check failed'); END"
            );
            execute_unprepared(connection, &sql).await?;
        }
    }
    execute_unprepared(
        connection,
        "INSERT INTO event_counters (id, next_global_seq) VALUES (1, 0)",
    )
    .await
}

pub(crate) async fn drop_all<C>(connection: &C) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    for table in [
        "rule_document_provenances",
        "rule_document_semantics",
        "rule_document_layouts",
        "rule_documents",
        "library_projection",
        "projection_hints",
        "projection_actions",
        "projection_relations",
        "projection_assets",
        "projection_units",
        "projection_collections",
        "projection_items",
        "projection_sources",
        "library_checkpoints",
        "source_checkpoints",
        "control_traces",
        "effect_captures",
        "execution_invocation_ledger",
        "execution_projection",
        "source_versions",
        "source_projection",
        "candidate_projection",
        "secret_artifact_owners",
        "secret_artifact_projection",
        "vault_key_metadata",
        "event_artifact_refs",
        "artifact_metadata",
        "events",
        "event_streams",
        "event_counters",
        "storage_schema_metadata",
    ] {
        execute_unprepared(connection, &format!("DROP TABLE IF EXISTS {table}")).await?;
    }
    Ok(())
}
