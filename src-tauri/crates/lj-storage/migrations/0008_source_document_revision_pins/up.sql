-- 编辑会话显式固定来源文档 revision；pin 只保存随机 secret 引用和安全 slot metadata。
CREATE TABLE source_document_revision_pins (
    pin_id TEXT PRIMARY KEY NOT NULL,
    document_id TEXT NOT NULL,
    document_revision INTEGER NOT NULL CHECK (document_revision > 0),
    format TEXT NOT NULL,
    masked_secret_id TEXT NOT NULL,
    raw_secret_id TEXT NOT NULL,
    manifest_secret_id TEXT NOT NULL,
    masked_hash TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    expires_at_ms INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    FOREIGN KEY (document_id) REFERENCES source_document_projection(document_id),
    FOREIGN KEY (masked_secret_id) REFERENCES secret_artifact_projection(secret_id),
    FOREIGN KEY (raw_secret_id) REFERENCES secret_artifact_projection(secret_id),
    FOREIGN KEY (manifest_secret_id) REFERENCES secret_artifact_projection(secret_id)
);
CREATE INDEX idx_source_document_revision_pin_expiry
    ON source_document_revision_pins(expires_at_ms, pin_id);
CREATE INDEX idx_source_document_revision_pin_owner
    ON source_document_revision_pins(document_id, document_revision);

CREATE TABLE source_document_revision_pin_slots (
    pin_id TEXT NOT NULL,
    slot_id TEXT NOT NULL,
    path TEXT NOT NULL,
    name TEXT NOT NULL,
    secret_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (pin_id, slot_id),
    UNIQUE (pin_id, path),
    FOREIGN KEY (pin_id) REFERENCES source_document_revision_pins(pin_id) ON DELETE CASCADE,
    FOREIGN KEY (secret_id) REFERENCES secret_artifact_projection(secret_id)
);
CREATE INDEX idx_source_document_revision_pin_slot_secret
    ON source_document_revision_pin_slots(secret_id);
