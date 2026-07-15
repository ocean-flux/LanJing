CREATE TABLE media_graph (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    delta_json TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE library_entries (
    resource_id TEXT PRIMARY KEY NOT NULL,
    favorite INTEGER NOT NULL DEFAULT 0,
    pinned INTEGER NOT NULL DEFAULT 0,
    last_opened_at TEXT,
    progress_json TEXT
);
