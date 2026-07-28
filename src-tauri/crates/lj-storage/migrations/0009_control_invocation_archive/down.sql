DROP TABLE control_traces;
DROP TABLE execution_invocation_ledger;
DROP INDEX idx_effect_captures_current_path;
DROP INDEX idx_effect_captures_current_invocation;

CREATE TABLE effect_captures_before_0009 (
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
INSERT INTO effect_captures_before_0009 (
    execution_id, effect_id, node_id, effect_kind, fingerprint, output_hash,
    witness_hash, output_artifact_hash, witness_artifact_hash,
    response_headers_secret_id, request_body_secret_id, global_seq
)
SELECT
    execution_id, effect_id, node_id, effect_kind, fingerprint, output_hash,
    witness_hash, output_artifact_hash, witness_artifact_hash,
    response_headers_secret_id, request_body_secret_id, global_seq
FROM effect_captures;
DROP TABLE effect_captures;
ALTER TABLE effect_captures_before_0009 RENAME TO effect_captures;
CREATE INDEX idx_effect_captures_replay
    ON effect_captures(execution_id, node_id, effect_kind);
