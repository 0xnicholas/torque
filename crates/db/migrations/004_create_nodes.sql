-- 004_create_nodes.sql
CREATE TABLE nodes (
    id                UUID PRIMARY KEY,
    run_id            UUID REFERENCES runs,
    tenant_id         UUID REFERENCES tenants,
    agent_type        TEXT REFERENCES agent_types(name),
    fallback_agent_type TEXT REFERENCES agent_types(name),
    instruction       TEXT NOT NULL,
    tools             JSONB,
    failure_policy    TEXT,
    requires_approval BOOLEAN DEFAULT false,
    status            TEXT NOT NULL DEFAULT 'pending',
    layer             INTEGER,
    created_at        TIMESTAMPTZ DEFAULT NOW(),
    started_at        TIMESTAMPTZ,
    completed_at      TIMESTAMPTZ,
    retry_count       INTEGER DEFAULT 0,
    error             TEXT,
    executor_id       TEXT
);

CREATE INDEX idx_nodes_run_id ON nodes(run_id);
CREATE INDEX idx_nodes_tenant_id ON nodes(tenant_id);
CREATE INDEX idx_nodes_status ON nodes(status);
CREATE INDEX idx_nodes_layer ON nodes(layer);
