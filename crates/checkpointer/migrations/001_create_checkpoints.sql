CREATE TABLE checkpoints (
    id            UUID PRIMARY KEY,
    run_id        UUID REFERENCES runs,
    node_id       UUID REFERENCES nodes,
    tenant_id     UUID REFERENCES tenants,
    state_hash    TEXT NOT NULL,
    storage       TEXT NOT NULL,
    location      TEXT NOT NULL,
    created_at    TIMESTAMPTZ DEFAULT NOW(),
    expires_at    TIMESTAMPTZ
);

CREATE INDEX idx_checkpoints_run_id ON checkpoints(run_id);
CREATE INDEX idx_checkpoints_node_id ON checkpoints(node_id);
CREATE INDEX idx_checkpoints_expires_at ON checkpoints(expires_at);