CREATE TABLE source_versions_without_replay_pin_snapshot (
    source_identity TEXT NOT NULL,
    version TEXT NOT NULL,
    package_artifact_hash TEXT NOT NULL,
    plan_artifact_hash TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    plan_hash TEXT NOT NULL,
    source_revision INTEGER NOT NULL,
    installed_at_ms INTEGER NOT NULL,
    PRIMARY KEY (source_identity, version)
);

INSERT INTO source_versions_without_replay_pin_snapshot (
    source_identity,
    version,
    package_artifact_hash,
    plan_artifact_hash,
    definition_hash,
    plan_hash,
    source_revision,
    installed_at_ms
)
SELECT
    source_identity,
    version,
    package_artifact_hash,
    plan_artifact_hash,
    definition_hash,
    plan_hash,
    source_revision,
    installed_at_ms
FROM source_versions;

DROP TABLE source_versions;
ALTER TABLE source_versions_without_replay_pin_snapshot RENAME TO source_versions;
CREATE INDEX idx_source_versions_plan ON source_versions(source_identity, plan_hash);
