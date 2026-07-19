ALTER TABLE source_versions ADD COLUMN profile_json TEXT;
ALTER TABLE source_versions ADD COLUMN grant_json TEXT;
ALTER TABLE source_versions ADD COLUMN base_url TEXT;

-- 仅回填仍是当前 source version 的配置；历史版本没有可信快照时由 replay pin 显式拒绝。
UPDATE source_versions
SET profile_json = (
        SELECT source_projection.profile_json
        FROM source_projection
        WHERE source_projection.source_identity = source_versions.source_identity
          AND source_projection.version = source_versions.version
    ),
    grant_json = (
        SELECT source_projection.grant_json
        FROM source_projection
        WHERE source_projection.source_identity = source_versions.source_identity
          AND source_projection.version = source_versions.version
    )
WHERE EXISTS (
    SELECT 1
    FROM source_projection
    WHERE source_projection.source_identity = source_versions.source_identity
      AND source_projection.version = source_versions.version
);
