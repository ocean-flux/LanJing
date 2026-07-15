CREATE TABLE rules (
    id TEXT PRIMARY KEY NOT NULL,
    source_url TEXT NOT NULL,
    graph_json TEXT NOT NULL,
    import_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE media (
    id TEXT PRIMARY KEY NOT NULL,
    source_id TEXT NOT NULL,
    media_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_media_source ON media(source_id);

CREATE TABLE cookies (
    id TEXT PRIMARY KEY NOT NULL,
    domain TEXT NOT NULL,
    cookie_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
