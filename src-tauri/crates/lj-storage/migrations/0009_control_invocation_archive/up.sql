DROP INDEX idx_effect_captures_replay;

ALTER TABLE effect_captures ADD COLUMN invocation_path_json TEXT;
ALTER TABLE effect_captures ADD COLUMN invocation_ordinal INTEGER;

CREATE UNIQUE INDEX idx_effect_captures_current_invocation
    ON effect_captures(execution_id, invocation_ordinal)
    WHERE invocation_path_json IS NOT NULL AND invocation_ordinal IS NOT NULL;
CREATE UNIQUE INDEX idx_effect_captures_current_path
    ON effect_captures(execution_id, invocation_path_json)
    WHERE invocation_path_json IS NOT NULL;

CREATE TABLE execution_invocation_ledger (
    execution_id TEXT NOT NULL,
    invocation_ordinal INTEGER NOT NULL CHECK (invocation_ordinal > 0),
    invocation_kind TEXT NOT NULL CHECK (invocation_kind IN ('effect', 'control')),
    node_id TEXT NOT NULL,
    invocation_path_json TEXT NOT NULL,
    payload_id TEXT NOT NULL,
    PRIMARY KEY (execution_id, invocation_ordinal),
    UNIQUE (execution_id, invocation_path_json),
    UNIQUE (execution_id, payload_id)
);

CREATE TABLE control_traces (
    execution_id TEXT NOT NULL,
    invocation_ordinal INTEGER NOT NULL CHECK (invocation_ordinal > 0),
    invocation_path_json TEXT NOT NULL,
    trace_hash TEXT NOT NULL,
    trace_json TEXT NOT NULL,
    PRIMARY KEY (execution_id, invocation_ordinal),
    FOREIGN KEY (execution_id, invocation_ordinal)
        REFERENCES execution_invocation_ledger(execution_id, invocation_ordinal)
        ON DELETE CASCADE
);
CREATE UNIQUE INDEX idx_control_traces_current_path
    ON control_traces(execution_id, invocation_path_json);
