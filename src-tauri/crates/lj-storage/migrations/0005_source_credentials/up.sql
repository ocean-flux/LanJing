CREATE TABLE source_credential_staging (
    candidate_id TEXT PRIMARY KEY NOT NULL,
    source_identity TEXT NOT NULL,
    cookie_namespace TEXT NOT NULL,
    secret_artifact_hash TEXT NOT NULL,
    expires_at_ms INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_source_credential_staging_expiry
    ON source_credential_staging(expires_at_ms);

ALTER TABLE candidates ADD COLUMN source_credentials_required INTEGER NOT NULL DEFAULT 0;

ALTER TABLE source_projection ADD COLUMN cookie_namespace TEXT NOT NULL DEFAULT '';
ALTER TABLE source_projection ADD COLUMN secret_artifact_hash TEXT;
ALTER TABLE source_versions ADD COLUMN cookie_namespace TEXT NOT NULL DEFAULT '';
ALTER TABLE source_versions ADD COLUMN secret_artifact_hash TEXT;

UPDATE source_projection
SET cookie_namespace = 'source/' || source_identity
WHERE cookie_namespace = '';
UPDATE source_versions
SET cookie_namespace = 'source/' || source_identity
WHERE cookie_namespace = '';
