-- 来源文档保险库、随机 secret identity、candidate-v2 与 revision-keyed execution pin。
-- 历史 deterministic secret 文件仍由启动期 Rust migrator执行 decrypt/copy/validate；本 migration
-- 只建立可回滚的新 schema 和迁移队列，不删除旧 artifact metadata/file。

CREATE TABLE vault_key_metadata (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    key_id TEXT NOT NULL UNIQUE,
    verifier TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE secret_artifact_projection (
    secret_id TEXT PRIMARY KEY NOT NULL,
    blob_locator TEXT NOT NULL UNIQUE,
    key_id TEXT NOT NULL,
    ciphertext_hash TEXT NOT NULL,
    stored_bytes INTEGER NOT NULL CHECK (stored_bytes >= 0),
    ref_count INTEGER NOT NULL CHECK (ref_count >= 0),
    schema_version INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_secret_artifact_ref_count
    ON secret_artifact_projection(ref_count, created_at_ms);

CREATE TABLE secret_artifact_owners (
    owner_kind TEXT NOT NULL,
    owner_id TEXT NOT NULL,
    secret_id TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (owner_kind, owner_id),
    FOREIGN KEY (secret_id) REFERENCES secret_artifact_projection(secret_id)
);
CREATE INDEX idx_secret_artifact_owner_secret
    ON secret_artifact_owners(secret_id);

CREATE TABLE source_document_projection (
    document_id TEXT PRIMARY KEY NOT NULL,
    format TEXT NOT NULL,
    title TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('draft', 'linked')),
    source_identity TEXT,
    installed_revision INTEGER,
    current_document_revision INTEGER NOT NULL CHECK (current_document_revision > 0),
    masked_secret_id TEXT NOT NULL,
    raw_secret_id TEXT NOT NULL,
    manifest_secret_id TEXT NOT NULL,
    masked_hash TEXT NOT NULL,
    credential_slot_count INTEGER NOT NULL CHECK (credential_slot_count >= 0),
    schema_version INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY (masked_secret_id) REFERENCES secret_artifact_projection(secret_id),
    FOREIGN KEY (raw_secret_id) REFERENCES secret_artifact_projection(secret_id),
    FOREIGN KEY (manifest_secret_id) REFERENCES secret_artifact_projection(secret_id)
);
CREATE UNIQUE INDEX idx_source_document_linked_source
    ON source_document_projection(source_identity)
    WHERE source_identity IS NOT NULL;
CREATE INDEX idx_source_document_updated
    ON source_document_projection(updated_at_ms DESC, document_id);

CREATE TABLE source_document_credential_slots (
    document_id TEXT NOT NULL,
    document_revision INTEGER NOT NULL CHECK (document_revision > 0),
    slot_id TEXT NOT NULL,
    path TEXT NOT NULL,
    name TEXT NOT NULL,
    secret_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (document_id, document_revision, slot_id),
    UNIQUE (document_id, document_revision, path),
    FOREIGN KEY (document_id) REFERENCES source_document_projection(document_id) ON DELETE CASCADE,
    FOREIGN KEY (secret_id) REFERENCES secret_artifact_projection(secret_id)
);
CREATE INDEX idx_source_document_slot_secret
    ON source_document_credential_slots(secret_id);

CREATE TABLE source_document_snapshots (
    document_id TEXT NOT NULL,
    document_revision INTEGER NOT NULL CHECK (document_revision > 0),
    source_identity TEXT NOT NULL,
    source_revision INTEGER NOT NULL CHECK (source_revision > 0),
    masked_secret_id TEXT NOT NULL,
    raw_secret_id TEXT NOT NULL,
    manifest_secret_id TEXT NOT NULL,
    masked_hash TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (document_id, document_revision, source_revision),
    UNIQUE (source_identity, source_revision),
    FOREIGN KEY (document_id) REFERENCES source_document_projection(document_id),
    FOREIGN KEY (masked_secret_id) REFERENCES secret_artifact_projection(secret_id),
    FOREIGN KEY (raw_secret_id) REFERENCES secret_artifact_projection(secret_id),
    FOREIGN KEY (manifest_secret_id) REFERENCES secret_artifact_projection(secret_id)
);
CREATE INDEX idx_source_document_snapshot_revision
    ON source_document_snapshots(document_id, document_revision);

CREATE TABLE candidate_projection (
    candidate_id TEXT PRIMARY KEY NOT NULL,
    candidate_schema_version INTEGER NOT NULL,
    document_schema_version INTEGER NOT NULL,
    source_format TEXT NOT NULL,
    document_id TEXT,
    document_revision INTEGER,
    transient INTEGER NOT NULL CHECK (transient IN (0, 1)),
    document_masked_hash TEXT NOT NULL,
    raw_secret_id TEXT NOT NULL,
    manifest_secret_id TEXT NOT NULL,
    runtime_credential_secret_id TEXT,
    target_source_identity TEXT NOT NULL,
    expected_installed_revision INTEGER NOT NULL CHECK (expected_installed_revision >= 0),
    package_artifact_hash TEXT NOT NULL,
    plan_artifact_hash TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    plan_hash TEXT NOT NULL,
    profile_json TEXT NOT NULL,
    required_grant_json TEXT NOT NULL,
    diagnostics_json TEXT NOT NULL,
    expires_at_ms INTEGER NOT NULL,
    status TEXT NOT NULL,
    stream_version INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    consumed_at_ms INTEGER,
    CHECK (
        (transient = 1 AND document_id IS NULL AND document_revision IS NULL)
        OR
        (transient = 0 AND document_id IS NOT NULL AND document_revision > 0)
    ),
    FOREIGN KEY (raw_secret_id) REFERENCES secret_artifact_projection(secret_id),
    FOREIGN KEY (manifest_secret_id) REFERENCES secret_artifact_projection(secret_id),
    FOREIGN KEY (runtime_credential_secret_id) REFERENCES secret_artifact_projection(secret_id)
);
CREATE INDEX idx_candidate_projection_expiry
    ON candidate_projection(status, expires_at_ms);
CREATE INDEX idx_candidate_projection_document
    ON candidate_projection(document_id, document_revision, status);

CREATE TABLE legacy_secret_artifact_migration (
    owner_kind TEXT NOT NULL,
    owner_id TEXT NOT NULL,
    legacy_hash TEXT NOT NULL,
    secret_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    PRIMARY KEY (owner_kind, owner_id)
);
CREATE INDEX idx_legacy_secret_migration_hash
    ON legacy_secret_artifact_migration(legacy_hash, status);

INSERT INTO legacy_secret_artifact_migration (owner_kind, owner_id, legacy_hash)
SELECT 'source_projection_runtime', source_identity, secret_artifact_hash
FROM source_projection
WHERE secret_artifact_hash IS NOT NULL;

INSERT INTO legacy_secret_artifact_migration (owner_kind, owner_id, legacy_hash)
SELECT 'source_version_runtime', source_identity || ':' || CAST(source_revision AS TEXT), secret_artifact_hash
FROM source_versions
WHERE secret_artifact_hash IS NOT NULL;

INSERT INTO legacy_secret_artifact_migration (owner_kind, owner_id, legacy_hash)
SELECT 'effect_response_headers', execution_id || ':' || effect_id, secret_artifact_hash
FROM effect_captures
WHERE secret_artifact_hash IS NOT NULL;

INSERT INTO legacy_secret_artifact_migration (owner_kind, owner_id, legacy_hash)
SELECT 'effect_request_body', execution_id || ':' || effect_id, request_body_artifact_hash
FROM effect_captures
WHERE request_body_artifact_hash IS NOT NULL;

CREATE TABLE source_projection_vault (
    source_identity TEXT PRIMARY KEY NOT NULL,
    version TEXT NOT NULL,
    profile_json TEXT NOT NULL,
    grant_json TEXT NOT NULL,
    package_artifact_hash TEXT NOT NULL,
    plan_artifact_hash TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    plan_hash TEXT NOT NULL,
    cookie_namespace TEXT NOT NULL DEFAULT '',
    runtime_credential_secret_id TEXT,
    document_id TEXT,
    document_revision INTEGER,
    revision INTEGER NOT NULL,
    updated_global_seq INTEGER NOT NULL,
    FOREIGN KEY (runtime_credential_secret_id) REFERENCES secret_artifact_projection(secret_id)
);
INSERT INTO source_projection_vault (
    source_identity, version, profile_json, grant_json, package_artifact_hash,
    plan_artifact_hash, definition_hash, plan_hash, cookie_namespace,
    runtime_credential_secret_id, document_id, document_revision, revision, updated_global_seq
)
SELECT
    source_identity, version, profile_json, grant_json, package_artifact_hash,
    plan_artifact_hash, definition_hash, plan_hash, cookie_namespace,
    NULL, NULL, NULL, revision, updated_global_seq
FROM source_projection;
DROP TABLE source_projection;
ALTER TABLE source_projection_vault RENAME TO source_projection;

CREATE TABLE source_versions_vault (
    source_identity TEXT NOT NULL,
    source_revision INTEGER NOT NULL CHECK (source_revision > 0),
    version TEXT NOT NULL,
    profile_json TEXT,
    grant_json TEXT,
    base_url TEXT,
    package_artifact_hash TEXT NOT NULL,
    plan_artifact_hash TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    plan_hash TEXT NOT NULL,
    cookie_namespace TEXT NOT NULL DEFAULT '',
    runtime_credential_secret_id TEXT,
    document_id TEXT,
    document_revision INTEGER,
    document_masked_hash TEXT,
    schema_version INTEGER NOT NULL DEFAULT 1,
    installed_at_ms INTEGER NOT NULL,
    PRIMARY KEY (source_identity, source_revision),
    FOREIGN KEY (runtime_credential_secret_id) REFERENCES secret_artifact_projection(secret_id)
);
INSERT INTO source_versions_vault (
    source_identity, source_revision, version, profile_json, grant_json, base_url,
    package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash,
    cookie_namespace, runtime_credential_secret_id, document_id, document_revision,
    document_masked_hash, schema_version, installed_at_ms
)
SELECT
    source_identity, source_revision, version, profile_json, grant_json, base_url,
    package_artifact_hash, plan_artifact_hash, definition_hash, plan_hash,
    cookie_namespace, NULL, NULL, NULL, NULL, 1, installed_at_ms
FROM source_versions;
DROP TABLE source_versions;
ALTER TABLE source_versions_vault RENAME TO source_versions;
CREATE INDEX idx_source_versions_definition
    ON source_versions(source_identity, version, definition_hash);
CREATE INDEX idx_source_versions_plan
    ON source_versions(source_identity, plan_hash);

CREATE TABLE legacy_execution_revision_evidence (
    execution_id TEXT PRIMARY KEY NOT NULL,
    source_revision INTEGER NOT NULL
);
INSERT INTO legacy_execution_revision_evidence (execution_id, source_revision)
SELECT execution.execution_id, versions.source_revision
FROM execution_projection AS execution
JOIN events AS started_event
  ON started_event.stream_id = 'execution/' || execution.execution_id
 AND started_event.stream_version = 1
JOIN source_versions AS versions
  ON versions.source_identity = execution.source_identity
 AND versions.version = execution.source_version
 AND versions.plan_hash = execution.plan_hash
 AND versions.plan_artifact_hash = execution.plan_artifact_hash
JOIN events AS source_event
  ON source_event.stream_id = 'source/' || execution.source_identity
 AND source_event.stream_version = versions.source_revision
 AND source_event.global_seq < started_event.global_seq
WHERE COALESCE(versions.profile_json, '') != ''
  AND COALESCE(versions.grant_json, '') != ''
  AND COALESCE(versions.base_url, '') != ''
  AND NOT EXISTS (
      SELECT 1
      FROM events AS later_source_event
      WHERE later_source_event.stream_id = source_event.stream_id
        AND later_source_event.stream_version > source_event.stream_version
        AND later_source_event.global_seq < started_event.global_seq
  )
  AND EXISTS (
      SELECT 1
      FROM event_artifact_refs AS source_ref
      WHERE source_ref.global_seq = source_event.global_seq
        AND source_ref.hash = versions.package_artifact_hash
        AND source_ref.artifact_kind = 'body'
  )
  AND EXISTS (
      SELECT 1
      FROM event_artifact_refs AS source_ref
      WHERE source_ref.global_seq = source_event.global_seq
        AND source_ref.hash = versions.plan_artifact_hash
        AND source_ref.artifact_kind = 'body'
  )
  AND (
      (
          NOT EXISTS (
              SELECT 1
              FROM legacy_secret_artifact_migration AS legacy
              WHERE legacy.owner_kind = 'source_version_runtime'
                AND legacy.owner_id = versions.source_identity || ':' || CAST(versions.source_revision AS TEXT)
          )
          AND NOT EXISTS (
              SELECT 1
              FROM event_artifact_refs AS source_ref
              WHERE source_ref.global_seq = source_event.global_seq
                AND source_ref.artifact_kind = 'secret'
          )
      )
      OR (
          EXISTS (
              SELECT 1
              FROM legacy_secret_artifact_migration AS legacy
              WHERE legacy.owner_kind = 'source_version_runtime'
                AND legacy.owner_id = versions.source_identity || ':' || CAST(versions.source_revision AS TEXT)
                AND EXISTS (
                    SELECT 1
                    FROM event_artifact_refs AS source_ref
                    WHERE source_ref.global_seq = source_event.global_seq
                      AND source_ref.artifact_kind = 'secret'
                      AND source_ref.hash = legacy.legacy_hash
                )
          )
          AND NOT EXISTS (
              SELECT 1
              FROM event_artifact_refs AS source_ref
              WHERE source_ref.global_seq = source_event.global_seq
                AND source_ref.artifact_kind = 'secret'
                AND source_ref.hash NOT IN (
                    SELECT legacy.legacy_hash
                    FROM legacy_secret_artifact_migration AS legacy
                    WHERE legacy.owner_kind = 'source_version_runtime'
                      AND legacy.owner_id = versions.source_identity || ':' || CAST(versions.source_revision AS TEXT)
                )
          )
      )
  );

CREATE TABLE execution_projection_vault (
    execution_id TEXT PRIMARY KEY NOT NULL,
    source_identity TEXT NOT NULL,
    source_revision INTEGER,
    source_version TEXT NOT NULL,
    plan_hash TEXT NOT NULL,
    plan_artifact_hash TEXT NOT NULL,
    status TEXT NOT NULL,
    replay_unavailable_reason TEXT,
    pinned INTEGER NOT NULL DEFAULT 0,
    archive_available INTEGER NOT NULL DEFAULT 1,
    gc_state TEXT NOT NULL DEFAULT 'active',
    started_at_ms INTEGER NOT NULL,
    finished_at_ms INTEGER,
    revision INTEGER NOT NULL,
    updated_global_seq INTEGER NOT NULL
);
INSERT INTO execution_projection_vault (
    execution_id, source_identity, source_revision, source_version, plan_hash,
    plan_artifact_hash, status, replay_unavailable_reason, pinned, archive_available,
    gc_state, started_at_ms, finished_at_ms, revision, updated_global_seq
)
SELECT
    execution.execution_id,
    execution.source_identity,
    evidence.source_revision,
    execution.source_version,
    execution.plan_hash,
    execution.plan_artifact_hash,
    execution.status,
    CASE
        WHEN evidence.source_revision IS NULL THEN 'legacy_evidence_missing'
        ELSE NULL
    END,
    execution.pinned,
    CASE
        WHEN evidence.source_revision IS NULL THEN 0
        ELSE execution.archive_available
    END,
    execution.gc_state,
    execution.started_at_ms,
    execution.finished_at_ms,
    execution.revision,
    execution.updated_global_seq
FROM execution_projection AS execution
LEFT JOIN legacy_execution_revision_evidence AS evidence
  ON evidence.execution_id = execution.execution_id;
DROP TABLE execution_projection;
ALTER TABLE execution_projection_vault RENAME TO execution_projection;
DROP TABLE legacy_execution_revision_evidence;
CREATE INDEX idx_execution_gc
    ON execution_projection(status, pinned, finished_at_ms, gc_state);
CREATE INDEX idx_execution_source
    ON execution_projection(source_identity, source_revision, started_at_ms);

CREATE TABLE effect_captures_vault (
    execution_id TEXT NOT NULL,
    effect_id TEXT NOT NULL,
    node_id TEXT NOT NULL,
    effect_kind TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    output_hash TEXT NOT NULL,
    witness_hash TEXT,
    output_artifact_hash TEXT NOT NULL,
    witness_artifact_hash TEXT,
    response_headers_secret_id TEXT,
    request_body_secret_id TEXT,
    global_seq INTEGER NOT NULL,
    PRIMARY KEY (execution_id, effect_id),
    FOREIGN KEY (response_headers_secret_id) REFERENCES secret_artifact_projection(secret_id),
    FOREIGN KEY (request_body_secret_id) REFERENCES secret_artifact_projection(secret_id)
);
INSERT INTO effect_captures_vault (
    execution_id, effect_id, node_id, effect_kind, fingerprint, output_hash,
    witness_hash, output_artifact_hash, witness_artifact_hash,
    response_headers_secret_id, request_body_secret_id, global_seq
)
SELECT
    execution_id, effect_id, node_id, effect_kind, fingerprint, output_hash,
    witness_hash, output_artifact_hash, witness_artifact_hash,
    NULL, NULL, global_seq
FROM effect_captures;
DROP TABLE effect_captures;
ALTER TABLE effect_captures_vault RENAME TO effect_captures;
CREATE INDEX idx_effect_captures_replay
    ON effect_captures(execution_id, node_id, effect_kind);

ALTER TABLE source_credential_staging RENAME TO source_credential_staging_legacy_0007;

-- Keep exact pre-0007 candidate status for an automatic down on vault-copy failure.
CREATE TABLE source_document_vault_legacy_candidates (
    candidate_id TEXT PRIMARY KEY NOT NULL
);
INSERT INTO source_document_vault_legacy_candidates (candidate_id)
SELECT candidate_id FROM candidates WHERE status = 'staged';
UPDATE candidates SET status = 'schema_invalid' WHERE status = 'staged';

-- Down is legal only before Rust secret migration marks this schema finalized and before any writer event.
CREATE TABLE source_document_vault_migration_baseline (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    event_max_global_seq INTEGER NOT NULL,
    finalized INTEGER NOT NULL DEFAULT 0 CHECK (finalized IN (0, 1))
);
INSERT INTO source_document_vault_migration_baseline (id, event_max_global_seq, finalized)
SELECT 1, COALESCE(MAX(global_seq), 0), 0 FROM events;
