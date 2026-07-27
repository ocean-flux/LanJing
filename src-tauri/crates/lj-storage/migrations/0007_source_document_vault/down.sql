CREATE TEMP TABLE source_document_vault_rollback_guard (
    ok INTEGER NOT NULL CHECK (ok = 1)
);
INSERT INTO source_document_vault_rollback_guard (ok)
SELECT CASE WHEN
    NOT EXISTS (
        SELECT 1 FROM source_document_vault_migration_baseline WHERE id = 1
    )
    OR (SELECT finalized FROM source_document_vault_migration_baseline WHERE id = 1) != 0
    OR (SELECT COALESCE(MAX(global_seq), 0) FROM events) != (
        SELECT event_max_global_seq
        FROM source_document_vault_migration_baseline
        WHERE id = 1
    )
    OR EXISTS (SELECT 1 FROM source_document_projection)
    OR EXISTS (SELECT 1 FROM source_document_snapshots)
    OR EXISTS (SELECT 1 FROM candidate_projection)
    OR EXISTS (SELECT 1 FROM secret_artifact_projection)
    THEN 0 ELSE 1 END;
DROP TABLE source_document_vault_rollback_guard;

DROP TABLE IF EXISTS source_credential_staging;
ALTER TABLE source_credential_staging_legacy_0007 RENAME TO source_credential_staging;

CREATE TABLE effect_captures_legacy (
    execution_id TEXT NOT NULL,
    effect_id TEXT NOT NULL,
    node_id TEXT NOT NULL,
    effect_kind TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    output_hash TEXT NOT NULL,
    output_artifact_hash TEXT NOT NULL,
    secret_artifact_hash TEXT,
    global_seq INTEGER NOT NULL,
    witness_hash TEXT,
    witness_artifact_hash TEXT,
    request_body_artifact_hash TEXT,
    PRIMARY KEY (execution_id, effect_id)
);
INSERT INTO effect_captures_legacy (
    execution_id, effect_id, node_id, effect_kind, fingerprint, output_hash,
    output_artifact_hash, secret_artifact_hash, global_seq, witness_hash,
    witness_artifact_hash, request_body_artifact_hash
)
SELECT
    execution_id, effect_id, node_id, effect_kind, fingerprint, output_hash,
    output_artifact_hash,
    (
        SELECT legacy_hash
        FROM legacy_secret_artifact_migration AS legacy
        WHERE legacy.owner_kind = 'effect_response_headers'
          AND legacy.owner_id = effect_captures.execution_id || ':' || effect_captures.effect_id
    ),
    global_seq, witness_hash, witness_artifact_hash,
    (
        SELECT legacy_hash
        FROM legacy_secret_artifact_migration AS legacy
        WHERE legacy.owner_kind = 'effect_request_body'
          AND legacy.owner_id = effect_captures.execution_id || ':' || effect_captures.effect_id
    )
FROM effect_captures;
DROP TABLE effect_captures;
ALTER TABLE effect_captures_legacy RENAME TO effect_captures;
CREATE INDEX idx_effect_captures_replay
    ON effect_captures(execution_id, node_id, effect_kind);

CREATE TABLE execution_projection_legacy (
    execution_id TEXT PRIMARY KEY NOT NULL,
    source_identity TEXT NOT NULL,
    source_version TEXT NOT NULL,
    plan_hash TEXT NOT NULL,
    plan_artifact_hash TEXT NOT NULL,
    status TEXT NOT NULL,
    pinned INTEGER NOT NULL DEFAULT 0,
    archive_available INTEGER NOT NULL DEFAULT 1,
    gc_state TEXT NOT NULL DEFAULT 'active',
    started_at_ms INTEGER NOT NULL,
    finished_at_ms INTEGER,
    revision INTEGER NOT NULL,
    updated_global_seq INTEGER NOT NULL
);
INSERT INTO execution_projection_legacy (
    execution_id, source_identity, source_version, plan_hash, plan_artifact_hash,
    status, pinned, archive_available, gc_state, started_at_ms, finished_at_ms,
    revision, updated_global_seq
)
SELECT
    execution_id, source_identity, source_version, plan_hash, plan_artifact_hash,
    status, pinned, archive_available, gc_state, started_at_ms, finished_at_ms,
    revision, updated_global_seq
FROM execution_projection;
DROP TABLE execution_projection;
ALTER TABLE execution_projection_legacy RENAME TO execution_projection;
CREATE INDEX idx_execution_gc
    ON execution_projection(status, pinned, finished_at_ms, gc_state);
CREATE INDEX idx_execution_source
    ON execution_projection(source_identity, started_at_ms);

CREATE TABLE source_versions_legacy (
    source_identity TEXT NOT NULL,
    version TEXT NOT NULL,
    package_artifact_hash TEXT NOT NULL,
    plan_artifact_hash TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    plan_hash TEXT NOT NULL,
    source_revision INTEGER NOT NULL,
    installed_at_ms INTEGER NOT NULL,
    profile_json TEXT,
    grant_json TEXT,
    base_url TEXT,
    cookie_namespace TEXT NOT NULL DEFAULT '',
    secret_artifact_hash TEXT,
    PRIMARY KEY (source_identity, version)
);
INSERT OR IGNORE INTO source_versions_legacy (
    source_identity, version, package_artifact_hash, plan_artifact_hash,
    definition_hash, plan_hash, source_revision, installed_at_ms, profile_json,
    grant_json, base_url, cookie_namespace, secret_artifact_hash
)
SELECT
    source_identity, version, package_artifact_hash, plan_artifact_hash,
    definition_hash, plan_hash, source_revision, installed_at_ms, profile_json,
    grant_json, base_url, cookie_namespace,
    (
        SELECT legacy_hash
        FROM legacy_secret_artifact_migration AS legacy
        WHERE legacy.owner_kind = 'source_version_runtime'
          AND legacy.owner_id = source_versions.source_identity || ':' || CAST(source_versions.source_revision AS TEXT)
    )
FROM source_versions
ORDER BY source_revision ASC;
DROP TABLE source_versions;
ALTER TABLE source_versions_legacy RENAME TO source_versions;
CREATE INDEX idx_source_versions_plan
    ON source_versions(source_identity, plan_hash);

CREATE TABLE source_projection_legacy (
    source_identity TEXT PRIMARY KEY NOT NULL,
    version TEXT NOT NULL,
    profile_json TEXT NOT NULL,
    grant_json TEXT NOT NULL,
    package_artifact_hash TEXT NOT NULL,
    plan_artifact_hash TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    plan_hash TEXT NOT NULL,
    cookie_namespace TEXT NOT NULL DEFAULT '',
    secret_artifact_hash TEXT,
    revision INTEGER NOT NULL,
    updated_global_seq INTEGER NOT NULL
);
INSERT INTO source_projection_legacy (
    source_identity, version, profile_json, grant_json, package_artifact_hash,
    plan_artifact_hash, definition_hash, plan_hash, cookie_namespace,
    secret_artifact_hash, revision, updated_global_seq
)
SELECT
    source_identity, version, profile_json, grant_json, package_artifact_hash,
    plan_artifact_hash, definition_hash, plan_hash, cookie_namespace,
    (
        SELECT legacy_hash
        FROM legacy_secret_artifact_migration AS legacy
        WHERE legacy.owner_kind = 'source_projection_runtime'
          AND legacy.owner_id = source_projection.source_identity
    ),
    revision, updated_global_seq
FROM source_projection;
DROP TABLE source_projection;
ALTER TABLE source_projection_legacy RENAME TO source_projection;

UPDATE candidates
SET status = 'staged'
WHERE candidate_id IN (
    SELECT candidate_id FROM source_document_vault_legacy_candidates
);
DROP TABLE IF EXISTS source_document_vault_legacy_candidates;
DROP TABLE IF EXISTS source_document_vault_migration_baseline;
DROP TABLE IF EXISTS legacy_secret_artifact_migration;
DROP TABLE IF EXISTS candidate_projection;
DROP TABLE IF EXISTS source_document_snapshots;
DROP TABLE IF EXISTS source_document_credential_slots;
DROP TABLE IF EXISTS source_document_projection;
DROP TABLE IF EXISTS secret_artifact_owners;
DROP TABLE IF EXISTS secret_artifact_projection;
DROP TABLE IF EXISTS vault_key_metadata;
