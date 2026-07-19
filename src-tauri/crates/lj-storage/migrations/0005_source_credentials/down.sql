DROP TABLE source_credential_staging;
ALTER TABLE candidates DROP COLUMN source_credentials_required;

CREATE TABLE source_projection_without_source_credentials (
    source_identity TEXT PRIMARY KEY NOT NULL,
    version TEXT NOT NULL,
    profile_json TEXT NOT NULL,
    grant_json TEXT NOT NULL,
    package_artifact_hash TEXT NOT NULL,
    plan_artifact_hash TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    plan_hash TEXT NOT NULL,
    revision INTEGER NOT NULL,
    updated_global_seq INTEGER NOT NULL
);
INSERT INTO source_projection_without_source_credentials (
    source_identity,
    version,
    profile_json,
    grant_json,
    package_artifact_hash,
    plan_artifact_hash,
    definition_hash,
    plan_hash,
    revision,
    updated_global_seq
)
SELECT
    source_identity,
    version,
    profile_json,
    grant_json,
    package_artifact_hash,
    plan_artifact_hash,
    definition_hash,
    plan_hash,
    revision,
    updated_global_seq
FROM source_projection;
DROP TABLE source_projection;
ALTER TABLE source_projection_without_source_credentials RENAME TO source_projection;

CREATE TABLE source_versions_without_source_credentials (
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
    PRIMARY KEY (source_identity, version)
);
INSERT INTO source_versions_without_source_credentials (
    source_identity,
    version,
    package_artifact_hash,
    plan_artifact_hash,
    definition_hash,
    plan_hash,
    source_revision,
    installed_at_ms,
    profile_json,
    grant_json,
    base_url
)
SELECT
    source_identity,
    version,
    package_artifact_hash,
    plan_artifact_hash,
    definition_hash,
    plan_hash,
    source_revision,
    installed_at_ms,
    profile_json,
    grant_json,
    base_url
FROM source_versions;
DROP TABLE source_versions;
ALTER TABLE source_versions_without_source_credentials RENAME TO source_versions;
CREATE INDEX idx_source_versions_plan ON source_versions(source_identity, plan_hash);
