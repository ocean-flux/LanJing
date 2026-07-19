-- C2 clean cutover：旧 Graph/单 JSON 媒体图不再是持久化真相。
DROP TABLE IF EXISTS library_entries;
DROP TABLE IF EXISTS media_graph;
DROP TABLE IF EXISTS cookies;
DROP TABLE IF EXISTS media;
DROP TABLE IF EXISTS rules;

CREATE TABLE event_counters (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    next_global_seq INTEGER NOT NULL
);
INSERT INTO event_counters (id, next_global_seq) VALUES (1, 0);

CREATE TABLE event_streams (
    stream_id TEXT PRIMARY KEY NOT NULL,
    version INTEGER NOT NULL
);

CREATE TABLE events (
    global_seq INTEGER PRIMARY KEY NOT NULL,
    stream_id TEXT NOT NULL,
    stream_version INTEGER NOT NULL,
    event_id TEXT NOT NULL UNIQUE,
    source_identity TEXT,
    event_type TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    correlation_id TEXT,
    causation_id TEXT,
    trace_id TEXT NOT NULL,
    occurred_at_ms INTEGER NOT NULL,
    payload_json TEXT NOT NULL,
    artifact_refs_json TEXT NOT NULL,
    secret_refs_json TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_events_stream_version ON events(stream_id, stream_version);
CREATE INDEX idx_events_source_global_seq ON events(source_identity, global_seq);

CREATE TABLE artifact_metadata (
    hash TEXT NOT NULL,
    artifact_kind TEXT NOT NULL,
    codec TEXT NOT NULL,
    hash_algorithm TEXT NOT NULL CHECK (hash_algorithm = 'blake3'),
    encryption TEXT,
    relative_path TEXT NOT NULL,
    stored_bytes INTEGER NOT NULL,
    ref_count INTEGER NOT NULL CHECK (ref_count >= 0),
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (hash, artifact_kind)
);
CREATE INDEX idx_artifact_ref_count ON artifact_metadata(ref_count);

CREATE TABLE event_artifact_refs (
    global_seq INTEGER NOT NULL,
    hash TEXT NOT NULL,
    artifact_kind TEXT NOT NULL,
    PRIMARY KEY (global_seq, hash, artifact_kind),
    FOREIGN KEY (global_seq) REFERENCES events(global_seq) ON DELETE CASCADE,
    FOREIGN KEY (hash, artifact_kind) REFERENCES artifact_metadata(hash, artifact_kind)
);
CREATE INDEX idx_event_artifact_refs_hash ON event_artifact_refs(hash, artifact_kind);

CREATE TABLE candidates (
    candidate_id TEXT PRIMARY KEY NOT NULL,
    source_identity TEXT NOT NULL,
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
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_candidates_expiry ON candidates(status, expires_at_ms);

CREATE TABLE source_projection (
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

CREATE TABLE source_versions (
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
CREATE INDEX idx_source_versions_plan ON source_versions(source_identity, plan_hash);

CREATE TABLE execution_projection (
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
CREATE INDEX idx_execution_gc ON execution_projection(status, pinned, finished_at_ms, gc_state);
CREATE INDEX idx_execution_source ON execution_projection(source_identity, started_at_ms);

CREATE TABLE source_checkpoints (
    source_identity TEXT PRIMARY KEY NOT NULL,
    source_revision INTEGER NOT NULL,
    global_seq INTEGER NOT NULL,
    artifact_hash TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE library_checkpoints (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    global_seq INTEGER NOT NULL,
    artifact_hash TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE projection_sources (
    id TEXT PRIMARY KEY NOT NULL,
    payload_json TEXT NOT NULL,
    updated_global_seq INTEGER NOT NULL
);

CREATE TABLE projection_items (
    id TEXT PRIMARY KEY NOT NULL,
    source_identity TEXT NOT NULL,
    media_kind TEXT NOT NULL,
    title TEXT NOT NULL,
    completeness TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    updated_global_seq INTEGER NOT NULL
);
CREATE INDEX idx_projection_items_source ON projection_items(source_identity, id);

CREATE TABLE projection_collections (
    id TEXT PRIMARY KEY NOT NULL,
    source_identity TEXT NOT NULL,
    collection_kind TEXT NOT NULL,
    title TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    updated_global_seq INTEGER NOT NULL
);
CREATE INDEX idx_projection_collections_source ON projection_collections(source_identity, id);

CREATE TABLE projection_units (
    id TEXT PRIMARY KEY NOT NULL,
    source_identity TEXT NOT NULL,
    item_id TEXT NOT NULL,
    position INTEGER,
    payload_json TEXT NOT NULL,
    updated_global_seq INTEGER NOT NULL
);
CREATE INDEX idx_projection_units_item ON projection_units(item_id, position, id);
CREATE INDEX idx_projection_units_source ON projection_units(source_identity, id);

CREATE TABLE projection_assets (
    id TEXT PRIMARY KEY NOT NULL,
    source_identity TEXT NOT NULL,
    unit_id TEXT,
    asset_kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    updated_global_seq INTEGER NOT NULL
);
CREATE INDEX idx_projection_assets_unit ON projection_assets(unit_id, id);
CREATE INDEX idx_projection_assets_source ON projection_assets(source_identity, id);

CREATE TABLE projection_relations (
    source_identity TEXT NOT NULL,
    from_id TEXT NOT NULL,
    to_id TEXT NOT NULL,
    relation_kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    updated_global_seq INTEGER NOT NULL,
    PRIMARY KEY (source_identity, from_id, to_id, relation_kind)
);
CREATE INDEX idx_projection_relations_from ON projection_relations(from_id, relation_kind);

CREATE TABLE projection_actions (
    id TEXT PRIMARY KEY NOT NULL,
    source_identity TEXT NOT NULL,
    intent TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    updated_global_seq INTEGER NOT NULL
);
CREATE INDEX idx_projection_actions_source ON projection_actions(source_identity, id);

CREATE TABLE projection_hints (
    resource_id TEXT PRIMARY KEY NOT NULL,
    source_identity TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    updated_global_seq INTEGER NOT NULL
);
CREATE INDEX idx_projection_hints_source ON projection_hints(source_identity, resource_id);

CREATE TABLE library_projection (
    resource_id TEXT PRIMARY KEY NOT NULL,
    favorite INTEGER NOT NULL,
    pinned INTEGER NOT NULL,
    last_opened_at TEXT,
    progress_json TEXT,
    updated_global_seq INTEGER NOT NULL
);
CREATE INDEX idx_library_projection_owned ON library_projection(favorite, pinned, resource_id);

CREATE TABLE effect_captures (
    execution_id TEXT NOT NULL,
    effect_id TEXT NOT NULL,
    node_id TEXT NOT NULL,
    effect_kind TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    output_hash TEXT NOT NULL,
    output_artifact_hash TEXT NOT NULL,
    secret_artifact_hash TEXT,
    global_seq INTEGER NOT NULL,
    PRIMARY KEY (execution_id, effect_id)
);
CREATE INDEX idx_effect_captures_replay ON effect_captures(execution_id, node_id, effect_kind);
