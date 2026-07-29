-- 未发布 current-only schema：随机 secret、candidate/install 与 execution revision pin。
-- fresh database 是唯一支持状态，因此直接替换旧 staging/projection，不迁移开发期数据。

DROP TABLE source_credential_staging;
DROP TABLE candidates;
DROP TABLE source_projection;
DROP TABLE source_versions;
DROP TABLE execution_projection;
DROP TABLE effect_captures;

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

CREATE TABLE candidate_projection (
    candidate_id TEXT PRIMARY KEY NOT NULL,
    candidate_schema_version INTEGER NOT NULL,
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
    FOREIGN KEY (runtime_credential_secret_id) REFERENCES secret_artifact_projection(secret_id)
);
CREATE INDEX idx_candidate_projection_expiry
    ON candidate_projection(status, expires_at_ms);

CREATE TABLE source_projection (
    source_identity TEXT PRIMARY KEY NOT NULL,
    version TEXT NOT NULL,
    profile_json TEXT NOT NULL,
    grant_json TEXT NOT NULL,
    package_artifact_hash TEXT NOT NULL,
    plan_artifact_hash TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    plan_hash TEXT NOT NULL,
    cookie_namespace TEXT NOT NULL,
    runtime_credential_secret_id TEXT,
    revision INTEGER NOT NULL CHECK (revision > 0),
    updated_global_seq INTEGER NOT NULL,
    FOREIGN KEY (runtime_credential_secret_id) REFERENCES secret_artifact_projection(secret_id)
);

CREATE TABLE source_versions (
    source_identity TEXT NOT NULL,
    source_revision INTEGER NOT NULL CHECK (source_revision > 0),
    version TEXT NOT NULL,
    profile_json TEXT NOT NULL,
    grant_json TEXT NOT NULL,
    base_url TEXT NOT NULL,
    package_artifact_hash TEXT NOT NULL,
    plan_artifact_hash TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    plan_hash TEXT NOT NULL,
    cookie_namespace TEXT NOT NULL,
    runtime_credential_secret_id TEXT,
    schema_version INTEGER NOT NULL,
    installed_at_ms INTEGER NOT NULL,
    PRIMARY KEY (source_identity, source_revision),
    FOREIGN KEY (runtime_credential_secret_id) REFERENCES secret_artifact_projection(secret_id)
);
CREATE INDEX idx_source_versions_definition
    ON source_versions(source_identity, version, definition_hash);
CREATE INDEX idx_source_versions_plan
    ON source_versions(source_identity, plan_hash);

CREATE TABLE execution_projection (
    execution_id TEXT PRIMARY KEY NOT NULL,
    source_identity TEXT NOT NULL,
    source_revision INTEGER NOT NULL CHECK (source_revision > 0),
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
    updated_global_seq INTEGER NOT NULL,
    FOREIGN KEY (source_identity, source_revision)
        REFERENCES source_versions(source_identity, source_revision)
);
CREATE INDEX idx_execution_gc
    ON execution_projection(status, pinned, finished_at_ms, gc_state);
CREATE INDEX idx_execution_source
    ON execution_projection(source_identity, source_revision, started_at_ms);

-- 0009 在此 pre-0009 shape 上追加 invocation identity 和 control archive。
CREATE TABLE effect_captures (
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
CREATE INDEX idx_effect_captures_replay
    ON effect_captures(execution_id, node_id, effect_kind);
